use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RedactionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: String,
    pub time_ms: u64,
}

/// Map redaction bounding box from source coordinates through active camera kinematics (center_x, center_y, zoom)
/// to destination canvas pixel coordinates [x0, y0, x1, y1]
pub fn transform_redaction_to_canvas(
    rect: &RedactionRect,
    center_x: f64,
    center_y: f64,
    zoom: f64,
    in_width: u32,
    in_height: u32,
    out_width: u32,
    out_height: u32,
) -> Option<[usize; 4]> {
    let scale_x = out_width as f64 / in_width as f64;
    let scale_y = out_height as f64 / in_height as f64;
    let base_fit_scale = scale_x.min(scale_y);
    let scale = base_fit_scale * zoom.max(0.1);

    // Support both normalized [0.0, 1.0] coordinates and absolute source pixels
    let (src_x0, src_x1, src_y0, src_y1) = if rect.width <= 1.0 && rect.height <= 1.0 && (rect.x + rect.width) <= 1.05 && (rect.y + rect.height) <= 1.05 {
        (
            rect.x as f64 * in_width as f64,
            (rect.x + rect.width) as f64 * in_width as f64,
            rect.y as f64 * in_height as f64,
            (rect.y + rect.height) as f64 * in_height as f64,
        )
    } else {
        (
            rect.x as f64,
            (rect.x + rect.width) as f64,
            rect.y as f64,
            (rect.y + rect.height) as f64,
        )
    };

    let dst_x0 = out_width as f64 * 0.5 + (src_x0 - center_x) * scale;
    let dst_x1 = out_width as f64 * 0.5 + (src_x1 - center_x) * scale;
    let dst_y0 = out_height as f64 * 0.5 + (src_y0 - center_y) * scale;
    let dst_y1 = out_height as f64 * 0.5 + (src_y1 - center_y) * scale;

    // Check if quad intersects canvas boundaries
    if dst_x1 <= 0.0 || dst_x0 >= out_width as f64 || dst_y1 <= 0.0 || dst_y0 >= out_height as f64 {
        return None;
    }

    let x0 = (dst_x0.max(0.0).min(out_width as f64)).floor() as usize;
    let x1 = (dst_x1.max(0.0).min(out_width as f64)).ceil() as usize;
    let y0 = (dst_y0.max(0.0).min(out_height as f64)).floor() as usize;
    let y1 = (dst_y1.max(0.0).min(out_height as f64)).ceil() as usize;

    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    Some([x0, y0, x1, y1])
}

/// Apply frosted-glass redaction blur (multi-pass blur + amber tint #F59E0B at 35% opacity)
/// directly to frame buffer in BGRA format pre-encode.
pub fn apply_frosted_glass_redaction(
    frame_bgra: &mut [u8],
    width: u32,
    height: u32,
    quad: [usize; 4],
) {
    let [x0, y0, x1, y1] = quad;
    let qw = x1 - x0;
    let qh = y1 - y0;

    if qw < 2 || qh < 2 {
        return;
    }

    let w = width as usize;
    let h = height as usize;

    let x0 = x0.min(w);
    let x1 = x1.min(w);
    let y0 = y0.min(h);
    let y1 = y1.min(h);

    let qw = x1 - x0;
    let qh = y1 - y0;

    // 1. Extract patch into temporary buffers for separable box blur
    // We store [B, G, R] channels
    let mut patch = vec![0u8; qw * qh * 4];
    for dy in 0..qh {
        let src_row_start = ((y0 + dy) * w + x0) * 4;
        let dst_row_start = dy * qw * 4;
        patch[dst_row_start..dst_row_start + qw * 4]
            .copy_from_slice(&frame_bgra[src_row_start..src_row_start + qw * 4]);
    }

    let mut temp = vec![0u8; qw * qh * 4];

    // Blur radius (radius 8 gives a 17-pixel convolution window, completely eliminating legibility)
    let radius = (qw.min(qh) / 4).clamp(4, 12) as i32;

    // Horizontal pass: patch -> temp
    for y in 0..qh {
        for x in 0..qw {
            let mut sum_b = 0u32;
            let mut sum_g = 0u32;
            let mut sum_r = 0u32;
            let mut count = 0u32;

            let start_kx = (x as i32 - radius).max(0);
            let end_kx = (x as i32 + radius).min(qw as i32 - 1);

            for kx in start_kx..=end_kx {
                let idx = (y * qw + kx as usize) * 4;
                sum_b += patch[idx] as u32;
                sum_g += patch[idx + 1] as u32;
                sum_r += patch[idx + 2] as u32;
                count += 1;
            }

            let dst_idx = (y * qw + x) * 4;
            temp[dst_idx] = (sum_b / count) as u8;
            temp[dst_idx + 1] = (sum_g / count) as u8;
            temp[dst_idx + 2] = (sum_r / count) as u8;
            temp[dst_idx + 3] = patch[dst_idx + 3];
        }
    }

    // Vertical pass: temp -> patch
    for y in 0..qh {
        for x in 0..qw {
            let mut sum_b = 0u32;
            let mut sum_g = 0u32;
            let mut sum_r = 0u32;
            let mut count = 0u32;

            let start_ky = (y as i32 - radius).max(0);
            let end_ky = (y as i32 + radius).min(qh as i32 - 1);

            for ky in start_ky..=end_ky {
                let idx = (ky as usize * qw + x) * 4;
                sum_b += temp[idx] as u32;
                sum_g += temp[idx + 1] as u32;
                sum_r += temp[idx + 2] as u32;
                count += 1;
            }

            let dst_idx = (y * qw + x) * 4;
            patch[dst_idx] = (sum_b / count) as u8;
            patch[dst_idx + 1] = (sum_g / count) as u8;
            patch[dst_idx + 2] = (sum_r / count) as u8;
        }
    }

    // 2. Blend with Amber Tint #F59E0B (RGB: 245, 158, 11) at 35% opacity
    // Formula: (blurred * 0.65) + (amber * 0.35)
    let amber_b = 11.0f32;
    let amber_g = 158.0f32;
    let amber_r = 245.0f32;
    let alpha = 0.35f32;
    let inv_alpha = 0.65f32;

    for dy in 0..qh {
        let frame_row_start = ((y0 + dy) * w + x0) * 4;
        let patch_row_start = dy * qw * 4;

        for dx in 0..qw {
            let p_idx = patch_row_start + dx * 4;
            let f_idx = frame_row_start + dx * 4;

            let blurred_b = patch[p_idx] as f32;
            let blurred_g = patch[p_idx + 1] as f32;
            let blurred_r = patch[p_idx + 2] as f32;

            // Check if boundary pixel for amber frosted-glass border accent
            let is_border = dy == 0 || dy == qh - 1 || dx == 0 || dx == qw - 1;

            if is_border {
                // 80% amber accent border
                frame_bgra[f_idx] = (blurred_b * 0.2 + amber_b * 0.8).round() as u8;
                frame_bgra[f_idx + 1] = (blurred_g * 0.2 + amber_g * 0.8).round() as u8;
                frame_bgra[f_idx + 2] = (blurred_r * 0.2 + amber_r * 0.8).round() as u8;
            } else {
                // 35% amber frosted-glass interior
                frame_bgra[f_idx] = (blurred_b * inv_alpha + amber_b * alpha).round() as u8;
                frame_bgra[f_idx + 1] = (blurred_g * inv_alpha + amber_g * alpha).round() as u8;
                frame_bgra[f_idx + 2] = (blurred_r * inv_alpha + amber_r * alpha).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_redaction_to_canvas_at_center() {
        let rect = RedactionRect {
            x: 900.0,
            y: 500.0,
            width: 120.0,
            height: 30.0,
            label: "API Key".into(),
            time_ms: 0,
        };

        // Source 1920x1080 -> 16:9 1080p target (1920x1080) at 1.0x centered at (960, 540)
        let quad = transform_redaction_to_canvas(
            &rect,
            960.0,
            540.0,
            1.0,
            1920,
            1080,
            1920,
            1080,
        );

        assert!(quad.is_some());
        let [x0, y0, x1, y1] = quad.unwrap();
        assert_eq!(x0, 900);
        assert_eq!(y0, 500);
        assert_eq!(x1, 1020);
        assert_eq!(y1, 530);
    }

    #[test]
    fn test_transform_redaction_to_canvas_with_zoom_and_pan() {
        let rect = RedactionRect {
            x: 800.0,
            y: 400.0,
            width: 100.0,
            height: 40.0,
            label: "Secret".into(),
            time_ms: 0,
        };

        // Zoom 1.5x focused on (800, 400)
        let quad = transform_redaction_to_canvas(
            &rect,
            800.0,
            400.0,
            1.5,
            1920,
            1080,
            1920,
            1080,
        );

        assert!(quad.is_some());
        let [x0, y0, x1, y1] = quad.unwrap();
        // Since focal center is 800, 400: x0 maps to screen center (960)
        assert_eq!(x0, 960);
        assert_eq!(y0, 540);
        // Width scaled by 1.5 -> 150px
        assert_eq!(x1, 960 + 150);
        // Height scaled by 1.5 -> 60px
        assert_eq!(y1, 540 + 60);
    }

    #[test]
    fn test_frosted_glass_redaction_obscures_high_contrast_pixels() {
        let w = 100u32;
        let h = 100u32;
        let mut buffer = vec![0u8; (w * h * 4) as usize];

        // Create sharp high-contrast pattern (simulating text: alternating black and white lines)
        for y in 20..80 {
            for x in 20..80 {
                let idx = ((y * w + x) * 4) as usize;
                let val = if (x / 4) % 2 == 0 { 255u8 } else { 0u8 };
                buffer[idx] = val;     // B
                buffer[idx + 1] = val; // G
                buffer[idx + 2] = val; // R
                buffer[idx + 3] = 255; // A
            }
        }

        // Apply frosted glass redaction over [20, 20, 80, 80]
        apply_frosted_glass_redaction(&mut buffer, w, h, [20, 20, 80, 80]);

        // Verify that high-contrast frequency is obliterated (no 0 or 255 extremes remaining in interior)
        let mut has_amber_tint = false;
        for y in 30..70 {
            for x in 30..70 {
                let idx = ((y * w + x) * 4) as usize;
                let b = buffer[idx];
                let g = buffer[idx + 1];
                let r = buffer[idx + 2];

                // Contrast variation must be smooth and heavily damped
                assert!(r > 80 && r < 250, "Red channel must be blended and tinted");
                // Verify amber tint presence (R > G > B)
                if r > g && g > b {
                    has_amber_tint = true;
                }
            }
        }

        assert!(has_amber_tint, "Redacted region must possess amber frosted glass color characteristic");
    }
}
