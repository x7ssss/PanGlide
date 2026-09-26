use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct TimedCursorSample {
    pub x: f32,
    pub y: f32,
    pub timestamp_us: u64,
}

/// Spatial and velocity dead-zone filter: ignores cursor motion under 100px within 200ms
pub struct VelocityDeadZoneFilter {
    spatial_threshold_px: f32, // Default: 100.0 px
    temporal_window_us: u64,   // Default: 200,000 us (200ms)
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
            // Check if cursor has rested for > 300ms, slowly drifting anchor if needed
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

    pub fn reset(&mut self, x: f32, y: f32) {
        self.history.clear();
        self.anchor_pos = Some((x, y));
        self.current_target = (x, y);
        self.last_active_time_us = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dead_zone_ignores_sub_100px_jitter_in_200ms() {
        let mut filter = VelocityDeadZoneFilter::default_pan_glide();

        // Initial point at (500, 500)
        let t0 = 1_000_000u64;
        let target0 = filter.process_point(500.0, 500.0, t0);
        assert_eq!(target0, (500.0, 500.0));

        // Sub-100px jitter: moves 40px to (540, 500) at t = t0 + 50ms
        let target1 = filter.process_point(540.0, 500.0, t0 + 50_000);
        // Target remains pinned at initial anchor to prevent whip-pan!
        assert_eq!(target1, (500.0, 500.0));

        // Sub-100px jitter: moves 70px to (500, 570) at t = t0 + 150ms
        let target2 = filter.process_point(500.0, 570.0, t0 + 150_000);
        assert_eq!(target2, (500.0, 500.0));

        // Intentional gesture: moves 160px to (660, 500) at t = t0 + 180ms (>= 100px)
        let target3 = filter.process_point(660.0, 500.0, t0 + 180_000);
        // Exceeds 100px -> Deadzone releases and camera target updates!
        assert_eq!(target3, (660.0, 500.0));
    }
}
