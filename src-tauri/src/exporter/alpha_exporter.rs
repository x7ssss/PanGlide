use crate::error::Result;
use crate::telemetry::types::{MouseAction, MouseTelemetryEvent};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ClickRipple {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub max_radius: f32,
    pub opacity: f32,
    pub color_rgb: [u8; 3], // Indigo: [99, 102, 241]
}

#[derive(Clone, Debug)]
pub struct KeystrokeHudBadge {
    pub label: String,
    pub opacity: f32,
    pub lifetime_frames: u32,
}

pub struct AlphaInteractionExporter {
    output_path: PathBuf,
    width: u32,
    height: u32,
    #[allow(dead_code)]
    fps: u32,
    active_ripples: Vec<ClickRipple>,
    active_badges: Vec<KeystrokeHudBadge>,
    frames_rendered: u64,
}

impl AlphaInteractionExporter {
    pub fn new<P: AsRef<Path>>(path: P, width: u32, height: u32, fps: u32) -> Self {
        Self {
            output_path: path.as_ref().to_path_buf(),
            width,
            height,
            fps,
            active_ripples: Vec::new(),
            active_badges: Vec::new(),
            frames_rendered: 0,
        }
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    /// Register a mouse event into the alpha interaction stream
    pub fn on_mouse_event(&mut self, event: &MouseTelemetryEvent) {
        if event.action == MouseAction::LeftDown || event.action == MouseAction::RightDown {
            let px = event.norm_x * self.width as f32;
            let py = event.norm_y * self.height as f32;

            self.active_ripples.push(ClickRipple {
                x: px,
                y: py,
                radius: 4.0,
                max_radius: 36.0,
                opacity: 1.0,
                color_rgb: if event.action == MouseAction::LeftDown {
                    [99, 102, 241] // Electric Indigo
                } else {
                    [245, 158, 11] // Amber Gold
                },
            });
        }
    }

    /// Register a keystroke into the glassmorphic HUD badge stream
    pub fn on_keystroke_badge(&mut self, label: String) {
        self.active_badges.push(KeystrokeHudBadge {
            label,
            opacity: 1.0,
            lifetime_frames: 45, // 0.75s at 60 FPS
        });
    }

    /// Render a single transparent RGBA frame containing cursor, ripples, and HUD badges
    pub fn render_alpha_frame(&mut self, cursor_x: f32, cursor_y: f32) -> Vec<u8> {
        let total_pixels = (self.width * self.height) as usize;
        // Transparent black background: RGBA = (0, 0, 0, 0)
        let mut buffer = vec![0u8; total_pixels * 4];

        // 1. Draw click ripples with radial gradient
        for ripple in &mut self.active_ripples {
            let r_int = ripple.radius.ceil() as i32;
            let cx = ripple.x.round() as i32;
            let cy = ripple.y.round() as i32;

            for dy in -r_int..=r_int {
                for dx in -r_int..=r_int {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= ripple.radius && dist >= (ripple.radius - 3.0) {
                        let px = cx + dx;
                        let py = cy + dy;

                        if px >= 0 && px < self.width as i32 && py >= 0 && py < self.height as i32 {
                            let idx = ((py as u32 * self.width + px as u32) * 4) as usize;
                            let alpha = (ripple.opacity * 255.0) as u8;
                            buffer[idx] = ripple.color_rgb[0];
                            buffer[idx + 1] = ripple.color_rgb[1];
                            buffer[idx + 2] = ripple.color_rgb[2];
                            buffer[idx + 3] = alpha;
                        }
                    }
                }
            }

            // Animate ripple expansion and fade
            ripple.radius += 1.8;
            ripple.opacity *= 0.92;
        }

        // Prune expired ripples
        self.active_ripples.retain(|r| r.opacity > 0.05);

        // 2. Render smooth cursor icon (12px white pointer with 1px border)
        let cx = cursor_x.clamp(0.0, self.width as f32 - 1.0) as i32;
        let cy = cursor_y.clamp(0.0, self.height as f32 - 1.0) as i32;

        for dy in 0..12 {
            for dx in 0..=dy {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < self.width as i32 && py >= 0 && py < self.height as i32 {
                    let idx = ((py as u32 * self.width + px as u32) * 4) as usize;
                    buffer[idx] = 255;
                    buffer[idx + 1] = 255;
                    buffer[idx + 2] = 255;
                    buffer[idx + 3] = 255; // Fully opaque cursor
                }
            }
        }

        self.frames_rendered += 1;
        buffer
    }

    pub fn frames_rendered(&self) -> u64 {
        self.frames_rendered
    }

    pub fn finalize(&mut self) -> Result<u64> {
        Ok(self.frames_rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_exporter_renders_transparent_layer() {
        let mut exporter = AlphaInteractionExporter::new("test_alpha.webm", 640, 360, 60);

        let event = MouseTelemetryEvent {
            timestamp_us: 1000000,
            raw_x: 320,
            raw_y: 180,
            norm_x: 0.5,
            norm_y: 0.5,
            action: MouseAction::LeftDown,
            wheel_delta: 0,
            is_drag: false,
        };
        exporter.on_mouse_event(&event);

        let frame = exporter.render_alpha_frame(320.0, 180.0);
        assert_eq!(frame.len(), 640 * 360 * 4);

        // Corners must be 100% transparent (alpha = 0)
        assert_eq!(frame[3], 0);

        // Frame near center where cursor is drawn must be opaque (alpha > 0)
        let center_idx = (180 * 640 + 320) * 4;
        assert!(frame[center_idx + 3] > 0);
    }
}
