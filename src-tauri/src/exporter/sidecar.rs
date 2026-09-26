use crate::motion::camera::ViewportRect;
use crate::privacy::coordinates::DetectedSensitiveRegion;
use crate::telemetry::types::{MouseAction, MouseTelemetryEvent, RejectedTakeMarker};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelemetryFrameEntry {
    pub frame_index: u64,
    pub timestamp_us: u64,
    pub norm_x: f32,
    pub norm_y: f32,
    pub action: MouseAction,
    pub is_drag: bool,
    pub zoom_scale: f32,
    pub viewport: ViewportRect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanGlideTelemetrySidecar {
    pub version: String,
    pub resolution: [u32; 2],
    pub fps: u32,
    pub total_frames: u64,
    pub duration_seconds: f64,
    pub frames: Vec<TelemetryFrameEntry>,
    pub rejected_takes: Vec<RejectedTakeMarker>,
    pub redacted_regions: Vec<DetectedSensitiveRegion>,
}

impl PanGlideTelemetrySidecar {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self {
            version: "1.0.0".to_string(),
            resolution: [width, height],
            fps,
            total_frames: 0,
            duration_seconds: 0.0,
            frames: Vec::new(),
            rejected_takes: Vec::new(),
            redacted_regions: Vec::new(),
        }
    }

    pub fn add_frame(
        &mut self,
        frame_index: u64,
        mouse: &MouseTelemetryEvent,
        viewport: ViewportRect,
    ) {
        self.frames.push(TelemetryFrameEntry {
            frame_index,
            timestamp_us: mouse.timestamp_us,
            norm_x: mouse.norm_x,
            norm_y: mouse.norm_y,
            action: mouse.action,
            is_drag: mouse.is_drag,
            zoom_scale: viewport.zoom_scale,
            viewport,
        });
        self.total_frames = frame_index + 1;
        self.duration_seconds = (self.total_frames as f64) / (self.fps as f64);
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_generation_and_serialization() {
        let mut sidecar = PanGlideTelemetrySidecar::new(1920, 1080, 60);

        let mouse = MouseTelemetryEvent {
            timestamp_us: 1000000,
            raw_x: 960,
            raw_y: 540,
            norm_x: 0.5,
            norm_y: 0.5,
            action: MouseAction::Move,
            wheel_delta: 0,
            is_drag: false,
        };

        let vp = ViewportRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            zoom_scale: 1.0,
        };

        sidecar.add_frame(0, &mouse, vp);

        assert_eq!(sidecar.total_frames, 1);
        let json = serde_json::to_string(&sidecar).expect("Serialization failed");
        assert!(json.contains("\"fps\":60"));
        assert!(json.contains("\"norm_x\":0.5"));
    }
}
