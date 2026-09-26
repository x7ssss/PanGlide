/// Second-order spring-damper physical simulation for cinematic camera gliding
#[derive(Clone, Debug)]
pub struct SpringDamper {
    pub tension: f32, // Spring stiffness (k) - Default: 170.0
    pub damping: f32, // Friction damping (c) - Default: 26.0
    pub mass: f32,    // Effective camera mass - Default: 1.0
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

    /// PanGlide default: tension = 170.0, damping = 26.0
    pub fn default_pan_glide(initial_value: f32) -> Self {
        Self::new(initial_value, 170.0, 26.0)
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
        // Clamp dt to avoid explosion on hitch
        dt = dt.clamp(0.0001, 0.05);

        // Sub-step if dt > 1/60s for high physical accuracy
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

        // Snap if extremely close and practically at rest
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

    pub fn default_pan_glide(x: f32, y: f32) -> Self {
        Self {
            x: SpringDamper::default_pan_glide(x),
            y: SpringDamper::default_pan_glide(y),
        }
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

    pub fn is_settled(&self) -> bool {
        self.x.is_settled() && self.y.is_settled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spring_converges_to_target() {
        let mut spring = SpringDamper::default_pan_glide(0.0);
        spring.set_target(100.0);

        // Simulate 1 second at 60 FPS
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            spring.update(dt);
        }

        // Should settle comfortably near 100.0 without runaway oscillation
        assert!((spring.position - 100.0).abs() < 0.1);
        assert!(spring.velocity.abs() < 0.5);
    }
}
