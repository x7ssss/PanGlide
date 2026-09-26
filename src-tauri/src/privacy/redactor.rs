use crate::capture::d3d11::D3D11Context;
use crate::motion::camera::ViewportRect;
use crate::privacy::coordinates::DetectedSensitiveRegion;
use crate::shader::FrostedGlassPipeline;
use std::sync::{Arc, Mutex};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;

pub struct PrivacyRedactionEngine {
    pipeline: Option<FrostedGlassPipeline>,
    active_regions: Arc<Mutex<Vec<DetectedSensitiveRegion>>>,
    auto_redact_enabled: Arc<Mutex<bool>>,
}

impl PrivacyRedactionEngine {
    pub fn new(d3d: Option<D3D11Context>) -> Self {
        let pipeline = d3d.and_then(|ctx| FrostedGlassPipeline::new(ctx).ok());
        Self {
            pipeline,
            active_regions: Arc::new(Mutex::new(Vec::new())),
            auto_redact_enabled: Arc::new(Mutex::new(true)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        *self.auto_redact_enabled.lock().unwrap() = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        *self.auto_redact_enabled.lock().unwrap()
    }

    pub fn update_detected_regions(&self, regions: Vec<DetectedSensitiveRegion>) {
        *self.active_regions.lock().unwrap() = regions;
    }

    pub fn active_regions(&self) -> Vec<DetectedSensitiveRegion> {
        self.active_regions.lock().unwrap().clone()
    }

    /// Flatten privacy blur masks onto Direct3D 11 texture surface pre-encode
    pub fn apply_pre_encode_redaction(
        &self,
        src: &ID3D11Texture2D,
        dst: &ID3D11Texture2D,
        camera_viewport: &ViewportRect,
        time: f32,
        width: u32,
        height: u32,
    ) {
        if !self.is_enabled() {
            if let Some(ref p) = self.pipeline {
                p.render_redaction_mask(src, dst, &[], time, width, height);
            }
            return;
        }

        let regions = self.active_regions.lock().unwrap().clone();
        let mut viewport_rects = Vec::new();

        for r in &regions {
            if let Some(rect) = r.to_viewport_coords(camera_viewport) {
                viewport_rects.push(rect);
            }
        }

        if let Some(ref p) = self.pipeline {
            p.render_redaction_mask(src, dst, &viewport_rects, time, width, height);
        }
    }
}
