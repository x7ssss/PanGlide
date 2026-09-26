use std::collections::VecDeque;

/// Presets for second-order spring-damper physical simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringPreset {
    /// Tension: 240.0, Damping: 30.0 - Fast, responsive tracking
    Snappy,
    /// Tension: 170.0, Damping: 26.0 - Smooth, fluid gliding (PanGlide Default)
    Cinematic,
    /// Tension: 100.0, Damping: 20.0 - Subtle, gentle easing
    Gentle,
}

impl SpringPreset {
    pub fn params(&self) -> (f32, f32) {
        match self {
            SpringPreset::Snappy => (240.0, 30.0),
            SpringPreset::Cinematic => (170.0, 26.0),
            SpringPreset::Gentle => (100.0, 20.0),
        }
    }
}

/// Second-order spring-damper physical simulation for 1D scalar (zoom or coordinate)
#[derive(Clone, Debug)]
pub struct SpringDamper {
    pub tension: f32, // Spring stiffness (k)
    pub damping: f32, // Friction damping (c)
    pub mass: f32,    // Effective camera mass (default 1.0)
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
}

impl SpringDamper {
    pub fn new(initial_value: f32, tension: f32, damping: f32) -> Self {
        Self {
            tension,
            damping,
            mass: 1.0,
            position: initial_value,
            velocity: 0.0,
            target: initial_value,
        }
    }

    pub fn with_preset(initial_value: f32, preset: SpringPreset) -> Self {
        let (k, c) = preset.params();
        Self::new(initial_value, k, c)
    }

    /// PanGlide default: tension = 170.0, damping = 26.0
    pub fn default_pan_glide(initial_value: f32) -> Self {
        Self::with_preset(initial_value, SpringPreset::Cinematic)
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn reset(&mut self, value: f32) {
        self.position = value;
        self.velocity = 0.0;
        self.target = value;
    }

    /// Semi-implicit Euler integration step with sub-stepping for numerical stability
    pub fn update(&mut self, mut dt: f32) {
        // Clamp dt to avoid explosion on large hitch
        dt = dt.clamp(0.0001, 0.05);

        // Sub-step if dt > ~8ms (e.g. 60 FPS frame interval ~16.6ms -> 2 sub-steps)
        let sub_steps = (dt / 0.008).ceil() as usize;
        let sub_dt = dt / sub_steps as f32;

        for _ in 0..sub_steps {
            let displacement = self.position - self.target;
            let spring_force = -self.tension * displacement;
            let damping_force = -self.damping * self.velocity;
            let acceleration = (spring_force + damping_force) / self.mass;

            self.velocity += acceleration * sub_dt;
            self.position += self.velocity * sub_dt;
        }

        // Snap to target if extremely close and practically at rest
        if (self.position - self.target).abs() < 1e-4 && self.velocity.abs() < 1e-4 {
            self.position = self.target;
            self.velocity = 0.0;
        }
    }

    pub fn is_settled(&self) -> bool {
        (self.position - self.target).abs() < 1e-3 && self.velocity.abs() < 1e-3
    }
}

/// 2D Spring damper for (X, Y) camera center
#[derive(Clone, Debug)]
pub struct SpringDamper2D {
    pub x: SpringDamper,
    pub y: SpringDamper,
}

impl SpringDamper2D {
    pub fn new(x: f32, y: f32, tension: f32, damping: f32) -> Self {
        Self {
            x: SpringDamper::new(x, tension, damping),
            y: SpringDamper::new(y, tension, damping),
        }
    }

    pub fn with_preset(x: f32, y: f32, preset: SpringPreset) -> Self {
        let (k, c) = preset.params();
        Self::new(x, y, k, c)
    }

    pub fn default_pan_glide(x: f32, y: f32) -> Self {
        Self::with_preset(x, y, SpringPreset::Cinematic)
    }

    pub fn set_target(&mut self, target_x: f32, target_y: f32) {
        self.x.set_target(target_x);
        self.y.set_target(target_y);
    }

    pub fn update(&mut self, dt: f32) {
        self.x.update(dt);
        self.y.update(dt);
    }

    pub fn position(&self) -> (f32, f32) {
        (self.x.position, self.y.position)
    }

    pub fn velocity(&self) -> (f32, f32) {
        (self.x.velocity, self.y.velocity)
    }

    pub fn is_settled(&self) -> bool {
        self.x.is_settled() && self.y.is_settled()
    }

    pub fn reset(&mut self, x: f32, y: f32) {
        self.x.reset(x);
        self.y.reset(y);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TimedCursorSample {
    pub x: f32,
    pub y: f32,
    pub timestamp_us: u64,
}

/// Velocity & Dead-Zone Filter: ignores cursor movements under 100px within a 200ms window
#[derive(Clone, Debug)]
pub struct VelocityDeadZoneFilter {
    pub spatial_threshold_px: f32, // Default: 100.0 px
    pub temporal_window_us: u64,   // Default: 200,000 us (200ms)
    history: VecDeque<TimedCursorSample>,
    anchor_pos: Option<(f32, f32)>,
    last_active_time_us: u64,
    current_target: (f32, f32),
}

impl VelocityDeadZoneFilter {
    pub fn new(spatial_threshold_px: f32, temporal_window_ms: u64) -> Self {
        Self {
            spatial_threshold_px,
            temporal_window_us: temporal_window_ms * 1000,
            history: VecDeque::new(),
            anchor_pos: None,
            last_active_time_us: 0,
            current_target: (0.0, 0.0),
        }
    }

    /// PanGlide standard: 100px threshold within 200ms window
    pub fn default_pan_glide() -> Self {
        Self::new(100.0, 200)
    }

    /// Process a new raw cursor position and return the effective stabilized camera target
    pub fn process_point(&mut self, x: f32, y: f32, timestamp_us: u64) -> (f32, f32) {
        if self.anchor_pos.is_none() {
            self.anchor_pos = Some((x, y));
            self.current_target = (x, y);
            self.last_active_time_us = timestamp_us;
            return (x, y);
        }

        let anchor = self.anchor_pos.unwrap();

        // Prune history samples older than 200ms
        let cutoff = timestamp_us.saturating_sub(self.temporal_window_us);
        while let Some(front) = self.history.front() {
            if front.timestamp_us < cutoff {
                self.history.pop_front();
            } else {
                break;
            }
        }

        self.history.push_back(TimedCursorSample {
            x,
            y,
            timestamp_us,
        });

        let dx = x - anchor.0;
        let dy = y - anchor.1;
        let displacement = (dx * dx + dy * dy).sqrt();

        // Check if movement exceeds 100px spatial dead-zone threshold
        if displacement >= self.spatial_threshold_px {
            // Intentional motion confirmed: advance target and update anchor
            self.current_target = (x, y);
            self.anchor_pos = Some((x, y));
            self.last_active_time_us = timestamp_us;
        } else {
            // Check if cursor has rested at new spot for > 300ms, slowly updating anchor
            let idle_duration_us = timestamp_us.saturating_sub(self.last_active_time_us);
            if idle_duration_us > 300_000 {
                self.anchor_pos = Some((x, y));
                self.current_target = (x, y);
                self.last_active_time_us = timestamp_us;
            }
        }

        self.current_target
    }

    pub fn current_target(&self) -> (f32, f32) {
        self.current_target
    }

    pub fn reset(&mut self, x: f32, y: f32, timestamp_us: u64) {
        self.history.clear();
        self.anchor_pos = Some((x, y));
        self.current_target = (x, y);
        self.last_active_time_us = timestamp_us;
    }
}
