use crate::motion::deadzone::VelocityDeadZoneFilter;
use crate::motion::kalman::CursorKalmanFilter2D;
use crate::motion::spring::{SpringDamper, SpringDamper2D};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub zoom_scale: f32,
}

pub struct KinematicCamera {
    screen_width: f32,
    screen_height: f32,
    spring_pos: SpringDamper2D,
    spring_zoom: SpringDamper,
    kalman_filter: CursorKalmanFilter2D,
    deadzone_filter: VelocityDeadZoneFilter,
    min_zoom: f32,
    max_zoom: f32,
    target_zoom: f32,
}

impl KinematicCamera {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        let center_x = screen_width * 0.5;
        let center_y = screen_height * 0.5;

        Self {
            screen_width,
            screen_height,
            spring_pos: SpringDamper2D::default_pan_glide(center_x, center_y),
            spring_zoom: SpringDamper::new(1.0, 170.0, 26.0),
            kalman_filter: CursorKalmanFilter2D::new(center_x, center_y),
            deadzone_filter: VelocityDeadZoneFilter::default_pan_glide(),
            min_zoom: 1.0,
            max_zoom: 2.2,
            target_zoom: 1.0,
        }
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        let clamped = zoom.clamp(self.min_zoom, self.max_zoom);
        self.target_zoom = clamped;
        self.spring_zoom.set_target(clamped);
    }

    pub fn target_zoom(&self) -> f32 {
        self.target_zoom
    }

    /// Feed incoming cursor telemetry event into camera kinematics
    pub fn on_cursor_move(&mut self, raw_x: f32, raw_y: f32, timestamp_us: u64, dt: f32) {
        // 1. Kalman filter removes hardware sensor micro-jitter
        let (filtered_x, filtered_y) = self.kalman_filter.filter(raw_x, raw_y, dt);

        // 2. Dead-zone ignores micro-motion < 100px within 200ms
        let (target_x, target_y) = self.deadzone_filter.process_point(filtered_x, filtered_y, timestamp_us);

        // 3. Update physical spring target with clamped margins
        let zoom = self.spring_zoom.position.max(1.0);
        let half_w = (self.screen_width / zoom) * 0.5;
        let half_h = (self.screen_height / zoom) * 0.5;

        // Clamp camera center so viewport never shows empty void outside screen
        let clamped_target_x = target_x.clamp(half_w, self.screen_width - half_w);
        let clamped_target_y = target_y.clamp(half_h, self.screen_height - half_h);

        self.spring_pos.set_target(clamped_target_x, clamped_target_y);
    }

    /// Advance physics simulation by dt seconds
    pub fn update(&mut self, dt: f32) -> ViewportRect {
        self.spring_zoom.update(dt);
        self.spring_pos.update(dt);

        let zoom = self.spring_zoom.position.clamp(self.min_zoom, self.max_zoom);
        let view_w = self.screen_width / zoom;
        let view_h = self.screen_height / zoom;

        let (mut center_x, mut center_y) = self.spring_pos.position();

        // Safe boundaries
        center_x = center_x.clamp(view_w * 0.5, self.screen_width - view_w * 0.5);
        center_y = center_y.clamp(view_h * 0.5, self.screen_height - view_h * 0.5);

        ViewportRect {
            x: center_x - view_w * 0.5,
            y: center_y - view_h * 0.5,
            width: view_w,
            height: view_h,
            zoom_scale: zoom,
        }
    }
}
