use crate::motion::spring::SpringDamper;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiClusterBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub interaction_weight: f32,
}

pub struct ClusterVerticalCropper {
    source_width: u32,
    #[allow(dead_code)]
    source_height: u32,
    crop_width: u32,
    crop_height: u32,
    pan_spring: SpringDamper,
}

impl ClusterVerticalCropper {
    pub fn new(source_width: u32, source_height: u32) -> Self {
        // 9:16 aspect ratio: crop_width = source_height * (9 / 16)
        let exact_w = (source_height as f32 * (9.0 / 16.0)).round() as u32;
        // Enforce even dimensions for video codecs (H.264 / HEVC)
        let crop_width = if exact_w % 2 != 0 { exact_w + 1 } else { exact_w };
        let crop_height = source_height;

        let initial_x = (source_width.saturating_sub(crop_width) as f32) * 0.5;

        Self {
            source_width,
            source_height,
            crop_width,
            crop_height,
            pan_spring: SpringDamper::new(initial_x, 120.0, 24.0),
        }
    }

    pub fn crop_dimensions(&self) -> (u32, u32) {
        (self.crop_width, self.crop_height)
    }

    /// Update crop target based on interaction clusters (accessibility bounding boxes + cursor)
    pub fn update_clusters(
        &mut self,
        clusters: &[UiClusterBox],
        cursor_x: f32,
        dt: f32,
    ) -> (u32, u32, u32, u32) {
        let mut total_weight = 1.0f32;
        let mut weighted_x_sum = cursor_x * 1.0;

        for c in clusters {
            let center_x = c.x + c.width * 0.5;
            weighted_x_sum += center_x * c.interaction_weight;
            total_weight += c.interaction_weight;
        }

        let centroid_x = weighted_x_sum / total_weight;

        // Ideal crop left: center the 9:16 window on cluster centroid
        let half_w = self.crop_width as f32 * 0.5;
        let max_left = (self.source_width.saturating_sub(self.crop_width)) as f32;
        let target_left = (centroid_x - half_w).clamp(0.0, max_left);

        self.pan_spring.set_target(target_left);
        self.pan_spring.update(dt);

        let smooth_left = self.pan_spring.position.clamp(0.0, max_left).round() as u32;

        (smooth_left, 0, self.crop_width, self.crop_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertical_crop_dimensions_9_16() {
        let cropper = ClusterVerticalCropper::new(1920, 1080);
        let (w, h) = cropper.crop_dimensions();

        assert_eq!(h, 1080);
        // 1080 * 9 / 16 = 607.5 -> rounded to 608 (even)
        assert_eq!(w, 608);
        assert_eq!(w % 2, 0);
    }

    #[test]
    fn test_cluster_centroid_centers_crop_window() {
        let mut cropper = ClusterVerticalCropper::new(1920, 1080);

        // An active modal dialog on the right side of the screen at x=1400..1800
        let cluster = UiClusterBox {
            x: 1400.0,
            y: 200.0,
            width: 400.0,
            height: 600.0,
            interaction_weight: 10.0, // High priority modal
        };

        // Advance simulation towards cluster
        for _ in 0..120 {
            cropper.update_clusters(&[cluster.clone()], 1600.0, 1.0 / 60.0);
        }

        let (left, top, w, h) = cropper.update_clusters(&[cluster], 1600.0, 1.0 / 60.0);
        assert_eq!(top, 0);
        assert_eq!(w, 608);
        assert_eq!(h, 1080);

        // Max left is 1920 - 608 = 1312
        assert!(left > 1200);
        assert!(left <= 1312);
    }
}
