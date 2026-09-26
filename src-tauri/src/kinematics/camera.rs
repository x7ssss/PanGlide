use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use crate::kinematics::physics::{SpringDamper, SpringDamper2D, SpringPreset, VelocityDeadZoneFilter};
use crate::telemetry::types::TelemetrySidecar;

pub type SessionTelemetry = TelemetrySidecar;

/// Represents the camera viewport transform for a single 60 FPS CFR video frame
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraFrame {
    pub timestamp_ms: u64,
    pub x: f32, // normalized [0.0..1.0]
    pub y: f32, // normalized [0.0..1.0]
    pub zoom: f32,
    #[serde(default)]
    pub frame_index: u64,
    #[serde(default)]
    pub center_x: f32,
    #[serde(default)]
    pub center_y: f32,
}

impl CameraFrame {
    /// Return the source cropping rectangle (x0, y0, width, height) in source pixel coordinates
    pub fn crop_rect(&self, source_w: u32, source_h: u32) -> (f32, f32, f32, f32) {
        let zoom = self.zoom.max(1.0);
        let view_w = source_w as f32 / zoom;
        let view_h = source_h as f32 / zoom;
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;

        let cx = if self.center_x > 0.0 { self.center_x } else { self.x * source_w as f32 };
        let cy = if self.center_y > 0.0 { self.center_y } else { self.y * source_h as f32 };

        let left = (cx - half_w).clamp(0.0, source_w as f32 - view_w);
        let top = (cy - half_h).clamp(0.0, source_h as f32 - view_h);
        (left, top, view_w, view_h)
    }

    /// Return normalized center coordinates [0.0..1.0]
    pub fn normalized_center(&self, source_w: u32, source_h: u32) -> (f32, f32) {
        if self.x > 0.0 || self.y > 0.0 {
            (self.x.clamp(0.0, 1.0), self.y.clamp(0.0, 1.0))
        } else {
            (
                (self.center_x / source_w as f32).clamp(0.0, 1.0),
                (self.center_y / source_h as f32).clamp(0.0, 1.0),
            )
        }
    }
}

/// Kinematic Camera Engine simulating smooth panning, physical spring acceleration, and zooming
pub struct KinematicCamera {
    pub screen_width: f32,
    pub screen_height: f32,
    pub spring_pos: SpringDamper2D,
    pub spring_zoom: SpringDamper,
    pub deadzone: VelocityDeadZoneFilter,
    pub click_zoom_scale: f32, // Default 1.5x (range: 1.4x - 1.8x)
    pub target_zoom: f32,
    pub last_activity_us: u64,
    pub current_frame_index: u64,
}

impl KinematicCamera {
    pub fn new(screen_width: f32, screen_height: f32, preset: SpringPreset, click_zoom_scale: f32) -> Self {
        let center_x = screen_width * 0.5;
        let center_y = screen_height * 0.5;
        let (k, c) = preset.params();

        Self {
            screen_width,
            screen_height,
            spring_pos: SpringDamper2D::new(center_x, center_y, k, c),
            spring_zoom: SpringDamper::new(1.0, k, c),
            deadzone: VelocityDeadZoneFilter::default_pan_glide(),
            click_zoom_scale: click_zoom_scale.clamp(1.1, 2.5),
            target_zoom: 1.0,
            last_activity_us: 0,
            current_frame_index: 0,
        }
    }

    pub fn default_pan_glide(screen_width: f32, screen_height: f32) -> Self {
        Self::new(screen_width, screen_height, SpringPreset::Cinematic, 1.5)
    }

    /// Handle mouse move event with velocity and dead-zone filtering
    pub fn on_mouse_move(&mut self, x: f32, y: f32, timestamp_us: u64) {
        let prev_target = self.deadzone.current_target();
        let filtered_target = self.deadzone.process_point(x, y, timestamp_us);

        // If dead-zone released and moved to new target, register activity
        if (filtered_target.0 - prev_target.0).abs() > 0.01 || (filtered_target.1 - prev_target.1).abs() > 0.01 {
            self.last_activity_us = timestamp_us;

            // Only track cursor target position if currently zoomed in or zooming
            if self.target_zoom > 1.05 || self.spring_zoom.position > 1.05 {
                let zoom = self.target_zoom.max(self.spring_zoom.position).max(1.0);
                let half_w = (self.screen_width / zoom) * 0.5;
                let half_h = (self.screen_height / zoom) * 0.5;

                let clamped_x = filtered_target.0.clamp(half_w, self.screen_width - half_w);
                let clamped_y = filtered_target.1.clamp(half_h, self.screen_height - half_h);

                self.spring_pos.set_target(clamped_x, clamped_y);
            }
        }
    }

    /// Handle mouse click / down event: triggers camera zoom focus towards target coordinates
    pub fn on_mouse_down(&mut self, x: f32, y: f32, timestamp_us: u64) {
        self.last_activity_us = timestamp_us;
        self.target_zoom = self.click_zoom_scale;
        self.spring_zoom.set_target(self.target_zoom);

        // Reset deadzone anchor to click point with accurate timestamp
        self.deadzone.reset(x, y, timestamp_us);

        // Compute viewport margins for target zoom
        let half_w = (self.screen_width / self.target_zoom) * 0.5;
        let half_h = (self.screen_height / self.target_zoom) * 0.5;

        // Clamp camera target center so screen edges are never cropped beyond bounds
        let clamped_x = x.clamp(half_w, self.screen_width - half_w);
        let clamped_y = y.clamp(half_h, self.screen_height - half_h);

        self.spring_pos.set_target(clamped_x, clamped_y);
    }

    /// Check if cursor has remained idle for > 2.0s; if so, smoothly return camera to center (zoom: 1.0x)
    pub fn check_inactivity(&mut self, timestamp_us: u64) {
        if self.last_activity_us > 0 && timestamp_us.saturating_sub(self.last_activity_us) > 2_000_000 {
            // Inactivity threshold reached (2.0s) -> glide back to center at 1.0x zoom
            if (self.target_zoom - 1.0).abs() > 0.001 {
                self.target_zoom = 1.0;
                self.spring_zoom.set_target(1.0);
                self.spring_pos.set_target(self.screen_width * 0.5, self.screen_height * 0.5);
            }
        }
    }

    /// Advance physics simulation by dt seconds and return the resulting CameraFrame
    pub fn update(&mut self, dt: f32) -> CameraFrame {
        self.spring_zoom.update(dt);
        self.spring_pos.update(dt);

        let cur_zoom = self.spring_zoom.position.clamp(1.0, 3.0);
        let half_w = (self.screen_width / cur_zoom) * 0.5;
        let half_h = (self.screen_height / cur_zoom) * 0.5;

        let (raw_x, raw_y) = self.spring_pos.position();

        // Enforce strict boundary clamping: viewport never crops beyond screen edges
        let center_x = raw_x.clamp(half_w, self.screen_width - half_w);
        let center_y = raw_y.clamp(half_h, self.screen_height - half_h);

        let sw = if self.screen_width > 0.0 { self.screen_width } else { 1920.0 };
        let sh = if self.screen_height > 0.0 { self.screen_height } else { 1080.0 };

        let frame = CameraFrame {
            timestamp_ms: (self.current_frame_index as f64 * (1000.0 / 60.0)).round() as u64,
            x: (center_x / sw).clamp(0.0, 1.0),
            y: (center_y / sh).clamp(0.0, 1.0),
            zoom: cur_zoom,
            frame_index: self.current_frame_index,
            center_x,
            center_y,
        };

        self.current_frame_index += 1;
        frame
    }
}

/// Global cache of latest solved camera keyframes for zero-latency retrieval
static CACHED_KEYFRAMES: Mutex<Option<(String, Vec<CameraFrame>)>> = Mutex::new(None);

pub fn cache_latest_keyframes(identifier: String, frames: Vec<CameraFrame>) {
    if let Ok(mut guard) = CACHED_KEYFRAMES.lock() {
        *guard = Some((identifier, frames));
    }
}

/// Deterministic click-zoom state solver and second-order spring-damper camera solver
pub fn solve_camera_keyframes(
    events: &[crate::telemetry::types::InputEvent],
    duration_ms: u64,
    source_w: u32,
    source_h: u32,
    zoom_scale: f32,
) -> Vec<CameraFrame> {
    #[derive(Clone, Copy, Debug)]
    struct ClickEvent {
        timestamp_ms: u64,
        cx: f32,
        cy: f32,
    }

    let mut clicks: Vec<ClickEvent> = Vec::new();
    for evt in events {
        // Capture left-clicks (WM_LBUTTONDOWN) with normalized (x, y) in [0.0, 1.0]
        if evt.event_type == crate::telemetry::types::InputEventType::MouseDown && (evt.button == 1 || evt.button == 0) {
            let t_ms = evt.get_timestamp_ms();
            clicks.push(ClickEvent {
                timestamp_ms: t_ms,
                cx: evt.x.clamp(0.0, 1.0),
                cy: evt.y.clamp(0.0, 1.0),
            });
        }
    }

    // Re-base timestamps if UNIX epoch was used in older recordings
    if let Some(first) = clicks.first() {
        if first.timestamp_ms > 10_000_000 {
            let base = first.timestamp_ms;
            for c in &mut clicks {
                c.timestamp_ms = c.timestamp_ms.saturating_sub(base);
            }
        }
    }

    let total_duration_ms = if duration_ms > 0 {
        duration_ms
    } else if let Some(last_evt) = events.last() {
        last_evt.get_timestamp_ms().max(1000)
    } else {
        clicks.last().map(|c| c.timestamp_ms + 2500).unwrap_or(3000)
    };

    let total_frames = ((total_duration_ms as f64 * 60.0) / 1000.0).ceil() as u64;
    let total_frames = total_frames.max(1);

    let tension = 170.0f32;
    let damping = 26.0f32;
    let dt = 1.0f32 / 60.0f32;
    let target_zoom_scale = if zoom_scale >= 1.1 { zoom_scale } else { 1.5f32 };
    let dwell_time_ms = 2000u64; // 2.0s dwell time

    let mut current_x = 0.5f32;
    let mut current_y = 0.5f32;
    let mut current_zoom = 1.0f32;
    let mut vel_x = 0.0f32;
    let mut vel_y = 0.0f32;
    let mut vel_zoom = 0.0f32;

    let sw = if source_w > 0 { source_w as f32 } else { 1920.0 };
    let sh = if source_h > 0 { source_h as f32 } else { 1080.0 };

    let mut keyframes = Vec::with_capacity(total_frames as usize);

    for frame_idx in 0..total_frames {
        let t_ms = (frame_idx as f64 * (1000.0 / 60.0)).round() as u64;

        // 1. Click Event Ingestion & Deterministic Zoom State Machine:
        // Default state: target_zoom = 1.0, target_center = (0.5, 0.5)
        // On click event at (cx, cy):
        //   Set target_zoom = 1.5 (or user inspector setting)
        //   Set target_center = (cx, cy)
        //   Hold this zoom target for a dwell time of 2.0 seconds after the click.
        // After 2.0 seconds of no clicks:
        //   Ease target_zoom back to 1.0
        //   Ease target_center back to (0.5, 0.5)
        let latest_click = clicks.iter().filter(|c| c.timestamp_ms <= t_ms).last();

        let (target_x, target_y, target_zoom) = match latest_click {
            Some(click) if t_ms.saturating_sub(click.timestamp_ms) <= dwell_time_ms => {
                (click.cx, click.cy, target_zoom_scale)
            }
            _ => (0.5f32, 0.5f32, 1.0f32),
        };

        // 2. Second-Order Spring-Damper Solver (sub-stepped for numerical stability):
        let sub_steps = 2;
        let sub_dt = dt / sub_steps as f32;

        for _ in 0..sub_steps {
            let accel_x = tension * (target_x - current_x) - damping * vel_x;
            vel_x += accel_x * sub_dt;
            current_x += vel_x * sub_dt;

            let accel_y = tension * (target_y - current_y) - damping * vel_y;
            vel_y += accel_y * sub_dt;
            current_y += vel_y * sub_dt;

            let accel_zoom = tension * (target_zoom - current_zoom) - damping * vel_zoom;
            vel_zoom += accel_zoom * sub_dt;
            current_zoom += vel_zoom * sub_dt;
        }

        current_zoom = current_zoom.max(1.0);

        // Clamp current_x and current_y so the zoomed viewport never extends outside the monitor bounds:
        // half_w = 0.5 / current_zoom; half_h = 0.5 / current_zoom;
        // current_x = current_x.clamp(half_w, 1.0 - half_w);
        // current_y = current_y.clamp(half_h, 1.0 - half_h);
        let half_w = 0.5f32 / current_zoom;
        let half_h = 0.5f32 / current_zoom;
        current_x = current_x.clamp(half_w, 1.0f32 - half_w);
        current_y = current_y.clamp(half_h, 1.0f32 - half_h);

        keyframes.push(CameraFrame {
            timestamp_ms: t_ms,
            x: current_x,
            y: current_y,
            zoom: current_zoom,
            frame_index: frame_idx,
            center_x: current_x * sw,
            center_y: current_y * sh,
        });
    }

    keyframes
}

/// Expose Tauri command get_solved_camera_keyframes(session_id: String) -> Vec<CameraFrame>
#[tauri::command]
pub fn get_solved_camera_keyframes(session_id: String) -> Vec<CameraFrame> {
    // 1. Check in-memory cache
    if let Ok(guard) = CACHED_KEYFRAMES.lock() {
        if let Some((ref id, ref frames)) = *guard {
            if session_id.is_empty() || session_id == "latest" || id == &session_id || session_id.contains(id) || id.contains(&session_id) {
                return frames.clone();
            }
        }
    }

    // 2. Direct path candidates
    let mut candidates = Vec::new();
    if !session_id.is_empty() && session_id != "latest" {
        candidates.push(PathBuf::from(&session_id));
        candidates.push(PathBuf::from(&session_id).with_extension("telemetry.json"));
        let mut p = PathBuf::from(&session_id);
        p.set_extension("telemetry.json");
        candidates.push(p);

        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:/ProgramData".to_string());
        let rec_dir = PathBuf::from(local_app_data).join("PanGlide").join("recordings");
        candidates.push(rec_dir.join(&session_id));
        candidates.push(rec_dir.join(format!("{}.telemetry.json", session_id)));
    }

    for cand in &candidates {
        if cand.exists() {
            if let Ok(content) = std::fs::read_to_string(cand) {
                if let Ok(sidecar) = serde_json::from_str::<crate::telemetry::types::TelemetrySidecar>(&content) {
                    let frames = solve_camera_keyframes(
                        &sidecar.events,
                        sidecar.metadata.duration_ms,
                        sidecar.metadata.display_width,
                        sidecar.metadata.display_height,
                        1.5,
                    );
                    cache_latest_keyframes(session_id.clone(), frames.clone());
                    return frames;
                }
            }
        }
    }

    // 3. Check recordings directory for latest telemetry sidecar
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:/ProgramData".to_string());
    let rec_dir = PathBuf::from(local_app_data).join("PanGlide").join("recordings");
    if rec_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&rec_dir) {
            let mut sidecars: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.ends_with(".telemetry.json")
                })
                .collect();
            sidecars.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH));
            if let Some(latest) = sidecars.last() {
                if let Ok(content) = std::fs::read_to_string(latest.path()) {
                    if let Ok(sidecar) = serde_json::from_str::<crate::telemetry::types::TelemetrySidecar>(&content) {
                        let frames = solve_camera_keyframes(
                            &sidecar.events,
                            sidecar.metadata.duration_ms,
                            sidecar.metadata.display_width,
                            sidecar.metadata.display_height,
                            1.5,
                        );
                        cache_latest_keyframes(session_id, frames.clone());
                        return frames;
                    }
                }
            }
        }
    }

    // 4. Return cached frames if any exist
    if let Ok(guard) = CACHED_KEYFRAMES.lock() {
        if let Some((_, ref frames)) = *guard {
            return frames.clone();
        }
    }

    Vec::new()
}

/// Generate a 60 FPS kinematic camera path for an entire recording session
pub fn generate_camera_path(telemetry: &SessionTelemetry, source_w: u32, source_h: u32) -> Vec<CameraFrame> {
    solve_camera_keyframes(&telemetry.events, telemetry.metadata.duration_ms, source_w, source_h, 1.5)
}

/// Generate a 60 FPS kinematic camera path with custom spring preset and zoom scale
pub fn generate_camera_path_with_preset(
    telemetry: &SessionTelemetry,
    source_w: u32,
    source_h: u32,
    _preset: SpringPreset,
    click_zoom: f32,
) -> Vec<CameraFrame> {
    solve_camera_keyframes(&telemetry.events, telemetry.metadata.duration_ms, source_w, source_h, click_zoom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::types::*;

    #[test]
    fn test_deadzone_filter_suppresses_micro_jitters() {
        let sw = 1920.0;
        let sh = 1080.0;
        let mut camera = KinematicCamera::default_pan_glide(sw, sh);

        // Click at (1000, 500) to activate zoom
        camera.on_mouse_down(1000.0, 500.0, 1_000_000);
        assert_eq!(camera.target_zoom, 1.5);

        // Micro-jitter: move 40px to (1040, 500) within 50ms (< 100px threshold)
        camera.on_mouse_move(1040.0, 500.0, 1_050_000);

        // Deadzone filter must keep target position stationary at 1000.0
        let (tx, _) = (camera.spring_pos.x.target, camera.spring_pos.y.target);
        assert_eq!(tx, 1000.0, "Sub-100px jitter must not shift spring target");

        // Intentional gesture: move 150px to (1150, 500) (> 100px threshold)
        camera.on_mouse_move(1150.0, 500.0, 1_100_000);
        let (tx_new, _) = (camera.spring_pos.x.target, camera.spring_pos.y.target);
        assert_eq!(tx_new, 1150.0, "Displacement >= 100px must release deadzone");
    }

    #[test]
    fn test_click_triggers_zoom_and_smooth_damped_acceleration() {
        let sw = 1920.0;
        let sh = 1080.0;
        let mut camera = KinematicCamera::default_pan_glide(sw, sh);

        // Initial state at center
        assert_eq!(camera.spring_pos.position(), (960.0, 540.0));
        assert_eq!(camera.spring_zoom.position, 1.0);

        // Click at (1200, 650) at t = 1.0s (within 1.5x zoom bounds [640..1280] x [360..720])
        camera.on_mouse_down(1200.0, 650.0, 1_000_000);
        assert_eq!(camera.target_zoom, 1.5);

        // Step simulation 1 frame (1/60s)
        let frame1 = camera.update(1.0 / 60.0);
        // Camera must NOT teleport instantly: smooth physical motion
        assert!(frame1.center_x > 960.0 && frame1.center_x < 1200.0);
        assert!(frame1.zoom > 1.0 && frame1.zoom < 1.5);

        // Verify velocity is smoothly bounded
        let (vx, vy) = camera.spring_pos.velocity();
        assert!(vx > 0.0 && vx < 5000.0, "Velocity must be finite and damped");
        assert!(vy > 0.0 && vy < 5000.0);

        // Step 120 frames (2 seconds)
        for _ in 0..120 {
            camera.update(1.0 / 60.0);
        }

        // Camera must smoothly settle at target coordinates
        assert!((camera.spring_pos.position().0 - 1200.0).abs() < 1.0);
        assert!((camera.spring_pos.position().1 - 650.0).abs() < 1.0);
        assert!((camera.spring_zoom.position - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_inactivity_returns_to_center() {
        let sw = 1920.0;
        let sh = 1080.0;
        let mut camera = KinematicCamera::default_pan_glide(sw, sh);

        // User clicks at (1200, 300) at t = 1.0s
        camera.on_mouse_down(1200.0, 300.0, 1_000_000);
        for _ in 0..60 {
            camera.update(1.0 / 60.0);
        }

        // Idle for 1.5s (t = 2.5s) -> Under 2.0s threshold, stay zoomed
        camera.check_inactivity(2_500_000);
        assert_eq!(camera.target_zoom, 1.5);

        // Idle reaches 2.1s (t = 3.1s) -> Inactivity threshold crossed!
        camera.check_inactivity(3_100_000);
        assert_eq!(camera.target_zoom, 1.0, "Inactivity must reset target zoom to 1.0x");
        assert_eq!(
            (camera.spring_pos.x.target, camera.spring_pos.y.target),
            (960.0, 540.0),
            "Inactivity must reset spring target to screen center"
        );

        // Simulate 2 seconds of easing back
        for _ in 0..120 {
            camera.update(1.0 / 60.0);
        }

        assert!((camera.spring_pos.position().0 - 960.0).abs() < 1.0);
        assert!((camera.spring_pos.position().1 - 540.0).abs() < 1.0);
        assert!((camera.spring_zoom.position - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_boundary_clamping_never_exceeds_screen() {
        let sw = 1920.0;
        let sh = 1080.0;
        let mut camera = KinematicCamera::default_pan_glide(sw, sh);

        // Click at top-left corner (0, 0)
        camera.on_mouse_down(0.0, 0.0, 1_000_000);

        // At zoom 1.5x:
        // view_w = 1920 / 1.5 = 1280, half_w = 640
        // view_h = 1080 / 1.5 = 720, half_h = 360
        // Center MUST be clamped to at least (640, 360) so left/top crop is >= 0!
        assert_eq!(camera.spring_pos.x.target, 640.0);
        assert_eq!(camera.spring_pos.y.target, 360.0);

        // Simulate to settle
        for _ in 0..120 {
            camera.update(1.0 / 60.0);
        }

        let (left, top, vw, vh) = camera.update(1.0 / 60.0).crop_rect(1920, 1080);
        assert!(left >= 0.0, "Left crop must never be negative");
        assert!(top >= 0.0, "Top crop must never be negative");
        assert!(left + vw <= 1920.01, "Right crop must never exceed source width");
        assert!(top + vh <= 1080.01, "Bottom crop must never exceed source height");

        // Now test bottom-right corner (1920, 1080)
        camera.on_mouse_down(1920.0, 1080.0, 5_000_000);
        assert_eq!(camera.spring_pos.x.target, 1920.0 - 640.0);
        assert_eq!(camera.spring_pos.y.target, 1080.0 - 360.0);

        for _ in 0..120 {
            camera.update(1.0 / 60.0);
        }

        let (left2, top2, vw2, vh2) = camera.update(1.0 / 60.0).crop_rect(1920, 1080);
        assert!(left2 >= 0.0);
        assert!(top2 >= 0.0);
        assert!(left2 + vw2 <= 1920.01);
        assert!(top2 + vh2 <= 1080.01);
    }

    #[test]
    fn test_generate_camera_path_full_session() {
        let telemetry = TelemetrySidecar {
            metadata: TelemetryMetadata {
                frame_count: 180, // 3 seconds at 60 FPS
                duration_ms: 3000,
                sample_rate: 48000,
                dpi_scale: 1.0,
                display_width: 1920,
                display_height: 1080,
            },
            rejected_takes: vec![],
            events: vec![
                InputEvent {
                    timestamp_us: 100_000,
                    timestamp_ms: 100,
                    event_type: InputEventType::MouseDown,
                    x: 0.7,
                    y: 0.7,
                    button: 1,
                    key_code: 0,
                },
                InputEvent {
                    timestamp_us: 200_000,
                    timestamp_ms: 200,
                    event_type: InputEventType::Move,
                    x: 0.72,
                    y: 0.72,
                    button: 0,
                    key_code: 0,
                },
            ],
        };

        let frames = generate_camera_path(&telemetry, 1920, 1080);
        assert_eq!(frames.len(), 180, "Must produce exactly 180 frames");

        // First frame starts at center with 1.0 zoom
        assert_eq!(frames[0].frame_index, 0);
        assert_eq!(frames[0].zoom, 1.0);
        assert_eq!(frames[0].center_x, 960.0);
        assert_eq!(frames[0].center_y, 540.0);

        // Later frames must have zoom > 1.0 and move towards (0.7*1920 = 1344, 0.7*1080 = 756)
        let mid_frame = &frames[90];
        assert!(mid_frame.zoom > 1.2);
        assert!(mid_frame.center_x > 1100.0);
        assert!(mid_frame.center_y > 600.0);
    }

    #[test]
    fn test_camera_kinematics_comprehensive() {
        let sw = 1920.0;
        let sh = 1080.0;
        let mut camera = KinematicCamera::default_pan_glide(sw, sh);

        // 1. Verify micro-jitter under 100px within 200ms is suppressed
        camera.on_mouse_down(1000.0, 500.0, 1_000_000);
        let target_before = camera.spring_pos.x.target;
        // Jitter by 50px at t = 100ms
        camera.on_mouse_move(1050.0, 500.0, 1_100_000);
        assert_eq!(
            camera.spring_pos.x.target, target_before,
            "Micro-jitter < 100px within 200ms must be suppressed"
        );

        // 2. Sudden cursor jump produces smooth, damped acceleration without overshoot spikes
        // Jump from 1000.0 to 1250.0 (within 1.5x zoom boundaries [640..1280])
        camera.on_mouse_move(1250.0, 500.0, 1_350_000);
        assert_eq!(camera.spring_pos.x.target, 1250.0, "Large motion >= 100px updates target");

        let mut prev_pos_x = camera.spring_pos.position().0;
        let mut max_vel_x = 0.0f32;
        let mut overshoot_observed = false;

        for _ in 0..120 {
            let f = camera.update(1.0 / 60.0);
            let cur_pos_x = f.center_x;
            let vel = (cur_pos_x - prev_pos_x).abs() * 60.0;
            if vel > max_vel_x {
                max_vel_x = vel;
            }
            if cur_pos_x > 1251.0 {
                overshoot_observed = true;
            }
            prev_pos_x = cur_pos_x;

            // 3. Camera bounds stay clamped within 0.0..1.0
            let (norm_x, norm_y) = f.normalized_center(1920, 1080);
            assert!(
                norm_x >= 0.0 && norm_x <= 1.0,
                "Normalized center X must stay clamped in 0.0..1.0 (got {})",
                norm_x
            );
            assert!(
                norm_y >= 0.0 && norm_y <= 1.0,
                "Normalized center Y must stay clamped in 0.0..1.0 (got {})",
                norm_y
            );
        }

        // Acceleration and velocity must be smoothly bounded
        assert!(max_vel_x > 50.0 && max_vel_x < 3000.0, "Velocity must be finite and damped");
        assert!(!overshoot_observed, "Damped second-order spring must not produce wild overshoot spikes");
        assert!((camera.spring_pos.position().0 - 1250.0).abs() < 1.0, "Camera smoothly settles at target");
    }

    #[test]
    fn test_automated_pan_interpolation_across_generated_mp4() {
        // Move cursor from top-left (0.1, 0.1) to bottom-right (0.9, 0.9) over 2.0 seconds (120 frames)
        let mut events = Vec::new();
        // Initial click at center (0.5, 0.5) to trigger zoom focus
        events.push(InputEvent {
            timestamp_us: 0,
            timestamp_ms: 0,
            event_type: InputEventType::MouseDown,
            x: 0.5,
            y: 0.5,
            button: 1,
            key_code: 0,
        });

        // 120 interpolation steps smoothly panning towards bottom-right (0.9, 0.9)
        for i in 1..=120 {
            let t_us = (i as u64) * (1_000_000 / 60);
            let factor = i as f32 / 120.0;
            events.push(InputEvent {
                timestamp_us: t_us,
                timestamp_ms: (t_us / 1000),
                event_type: InputEventType::MouseDown,
                x: 0.5 + factor * 0.4,
                y: 0.5 + factor * 0.4,
                button: 1,
                key_code: 0,
            });
        }

        let telemetry = TelemetrySidecar {
            metadata: TelemetryMetadata {
                frame_count: 120,
                duration_ms: 2000,
                sample_rate: 48000,
                dpi_scale: 1.0,
                display_width: 1920,
                display_height: 1080,
            },
            rejected_takes: vec![],
            events,
        };

        let frames = generate_camera_path(&telemetry, 1920, 1080);
        assert_eq!(frames.len(), 120);

        // Verify camera pan smoothly interpolates from top-left to bottom-right
        let first = &frames[0];
        let mid = &frames[60];
        let last = &frames[119];

        assert!(mid.center_x > first.center_x, "Center X must advance smoothly across the pan");
        assert!(mid.center_y > first.center_y, "Center Y must advance smoothly across the pan");
        assert!(last.center_x > mid.center_x, "Pan must continue smoothly towards bottom-right");
        assert!(last.center_y > mid.center_y);

        // All frames must stay within valid source boundaries
        for f in &frames {
            let (left, top, vw, vh) = f.crop_rect(1920, 1080);
            assert!(left >= 0.0);
            assert!(top >= 0.0);
            assert!(left + vw <= 1920.01);
            assert!(top + vh <= 1080.01);
        }
    }

    #[test]
    fn test_solve_camera_keyframes_deterministic_gliding() {
        // Create 5-second test scenario with click at (0.2, 0.3) at t = 1000ms
        let events = vec![
            InputEvent {
                timestamp_us: 1_000_000,
                timestamp_ms: 1000,
                event_type: InputEventType::MouseDown,
                x: 0.2,
                y: 0.3,
                button: 1,
                key_code: 0,
            },
        ];

        let frames = solve_camera_keyframes(&events, 5000, 1920, 1080, 1.5);
        assert_eq!(frames.len(), 300); // 5 sec * 60 fps = 300 frames

        // Frame 0 (t = 0): exactly centered at 1.0x
        assert_eq!(frames[0].zoom, 1.0);
        assert!((frames[0].x - 0.5).abs() < 1e-4);
        assert!((frames[0].y - 0.5).abs() < 1e-4);

        // Frame 60 (t = 1000ms, click moment): starts zooming in
        let f_click = &frames[60];
        assert_eq!(f_click.timestamp_ms, 1000);

        // Frame 120 (t = 2000ms, 1.0s after click): camera is zoomed in (~1.5x) and panned toward click
        let f_zoomed = &frames[120];
        assert!(f_zoomed.zoom > 1.45, "Zoom must reach target 1.5x (got {})", f_zoomed.zoom);
        assert!(f_zoomed.x < 0.45, "Camera X must glide towards click 0.2 (got {})", f_zoomed.x);
        assert!(f_zoomed.y < 0.45, "Camera Y must glide towards click 0.3 (got {})", f_zoomed.y);

        // Frame 270 (t = 4500ms, 3.5s after click, > 2.0s dwell): camera eases back to 1.0x and (0.5, 0.5)
        let f_idle = &frames[270];
        assert!(f_idle.zoom < 1.05, "Zoom must return to 1.0x idle (got {})", f_idle.zoom);
        assert!((f_idle.x - 0.5).abs() < 0.05, "Center X must return to 0.5 idle (got {})", f_idle.x);
        assert!((f_idle.y - 0.5).abs() < 0.05, "Center Y must return to 0.5 idle (got {})", f_idle.y);

        // Clamping check across all frames
        for f in &frames {
            let half_w = 0.5 / f.zoom;
            let half_h = 0.5 / f.zoom;
            assert!(f.x >= half_w - 1e-4 && f.x <= 1.0 - half_w + 1e-4, "Viewport X extends outside bounds: {} not in [{}, {}]", f.x, half_w, 1.0 - half_w);
            assert!(f.y >= half_h - 1e-4 && f.y <= 1.0 - half_h + 1e-4, "Viewport Y extends outside bounds: {} not in [{}, {}]", f.y, half_h, 1.0 - half_h);
        }
    }
}
