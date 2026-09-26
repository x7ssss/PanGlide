/// 1D Kalman Filter for position & velocity smoothing
#[derive(Clone, Debug)]
pub struct KalmanFilter1D {
    // State: [position, velocity]
    pub pos: f32,
    pub vel: f32,

    // Error covariance
    p00: f32,
    p01: f32,
    p10: f32,
    p11: f32,

    // Process noise
    pub q_pos: f32,
    pub q_vel: f32,

    // Measurement noise
    pub r_measure: f32,
}

impl KalmanFilter1D {
    pub fn new(initial_pos: f32, process_noise: f32, measurement_noise: f32) -> Self {
        Self {
            pos: initial_pos,
            vel: 0.0,
            p00: 1.0,
            p01: 0.0,
            p10: 0.0,
            p11: 1.0,
            q_pos: process_noise,
            q_vel: process_noise * 2.0,
            r_measure: measurement_noise,
        }
    }

    pub fn update(&mut self, measurement: f32, dt: f32) -> f32 {
        // 1. Predict
        let new_pos = self.pos + self.vel * dt;
        let new_vel = self.vel;

        let p00 = self.p00 + dt * (self.p10 + self.p01) + dt * dt * self.p11 + self.q_pos;
        let p01 = self.p01 + dt * self.p11;
        let p10 = self.p10 + dt * self.p11;
        let p11 = self.p11 + self.q_vel;

        // 2. Innovation
        let y = measurement - new_pos;
        let s = p00 + self.r_measure;

        // 3. Kalman Gain
        let k0 = p00 / s;
        let k1 = p10 / s;

        // 4. Update state
        self.pos = new_pos + k0 * y;
        self.vel = new_vel + k1 * y;

        // 5. Update covariance
        self.p00 = p00 * (1.0 - k0);
        self.p01 = p01 * (1.0 - k0);
        self.p10 = -k1 * p00 + p10;
        self.p11 = -k1 * p01 + p11;

        self.pos
    }
}

/// 2D Kalman filter for cursor coordinates (X, Y)
#[derive(Clone, Debug)]
pub struct CursorKalmanFilter2D {
    pub kx: KalmanFilter1D,
    pub ky: KalmanFilter1D,
}

impl CursorKalmanFilter2D {
    pub fn new(initial_x: f32, initial_y: f32) -> Self {
        Self {
            kx: KalmanFilter1D::new(initial_x, 0.05, 0.8),
            ky: KalmanFilter1D::new(initial_y, 0.05, 0.8),
        }
    }

    pub fn filter(&mut self, raw_x: f32, raw_y: f32, dt: f32) -> (f32, f32) {
        let x = self.kx.update(raw_x, dt);
        let y = self.ky.update(raw_y, dt);
        (x, y)
    }

    pub fn velocity(&self) -> (f32, f32) {
        (self.kx.vel, self.ky.vel)
    }

    pub fn speed(&self) -> f32 {
        (self.kx.vel * self.kx.vel + self.ky.vel * self.ky.vel).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kalman_filters_sensor_jitter() {
        let mut kf = CursorKalmanFilter2D::new(100.0, 100.0);
        let dt = 1.0 / 60.0;

        // Feed jittery noise around 100.0
        let noise = [98.0, 102.5, 99.0, 101.8, 100.2, 98.7, 101.1];
        let mut filtered_x = 100.0;

        for &n in &noise {
            let (fx, _) = kf.filter(n, 100.0, dt);
            filtered_x = fx;
        }

        // Filtered value should stay close to true center 100.0
        assert!((filtered_x - 100.0).abs() < 1.5);
    }
}
