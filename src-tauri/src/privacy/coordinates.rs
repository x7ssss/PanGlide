use crate::motion::camera::ViewportRect;
use crate::privacy::patterns::SensitiveTokenType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectedSensitiveRegion {
    pub id: u32,
    pub screen_x: f32,
    pub screen_y: f32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub token_type: SensitiveTokenType,
    pub token_preview: String,
}

impl DetectedSensitiveRegion {
    pub fn new(
        id: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        token_type: SensitiveTokenType,
        token_preview: String,
    ) -> Self {
        // Add 6px aesthetic padding around text bounding box
        let pad = 6.0;
        Self {
            id,
            screen_x: (x - pad).max(0.0),
            screen_y: (y - pad).max(0.0),
            screen_width: w + pad * 2.0,
            screen_height: h + pad * 2.0,
            token_type,
            token_preview,
        }
    }

    /// Transform screen-space bounding box into normalized viewport coordinates [0.0, 1.0]
    /// relative to the active kinematic camera viewport rect
    pub fn to_viewport_coords(&self, vp: &ViewportRect) -> Option<[f32; 4]> {
        let left = self.screen_x;
        let top = self.screen_y;
        let right = self.screen_x + self.screen_width;
        let bottom = self.screen_y + self.screen_height;

        let vp_right = vp.x + vp.width;
        let vp_bottom = vp.y + vp.height;

        // Check if bounding box intersects camera viewport
        if right < vp.x || left > vp_right || bottom < vp.y || top > vp_bottom {
            return None;
        }

        let clamped_left = left.max(vp.x);
        let clamped_top = top.max(vp.y);
        let clamped_right = right.min(vp_right);
        let clamped_bottom = bottom.min(vp_bottom);

        let norm_x = (clamped_left - vp.x) / vp.width;
        let norm_y = (clamped_top - vp.y) / vp.height;
        let norm_w = (clamped_right - clamped_left) / vp.width;
        let norm_h = (clamped_bottom - clamped_top) / vp.height;

        Some([norm_x, norm_y, norm_w, norm_h])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_transform_to_camera_viewport() {
        let region = DetectedSensitiveRegion::new(
            1,
            500.0,
            400.0,
            200.0,
            40.0,
            SensitiveTokenType::StripeApiKey,
            "sk_live_...".into(),
        );

        // Viewport looking at 1920x1080 at 1.0x (full screen)
        let vp_full = ViewportRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            zoom_scale: 1.0,
        };

        let coords = region.to_viewport_coords(&vp_full).expect("Should intersect full screen");
        assert!(coords[0] > 0.2 && coords[0] < 0.3);
        assert!(coords[1] > 0.3 && coords[1] < 0.4);

        // Viewport zoomed in on (0, 0) of size (200, 200) -> region is outside!
        let vp_outside = ViewportRect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
            zoom_scale: 2.0,
        };

        assert!(region.to_viewport_coords(&vp_outside).is_none());
    }
}
