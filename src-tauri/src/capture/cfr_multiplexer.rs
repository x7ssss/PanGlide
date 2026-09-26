use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;

#[derive(Clone)]
pub struct SynchronizedVideoFrame {
    pub frame_index: u64,
    pub presentation_time_100ns: i64,
    pub is_duplicate: bool,
    pub texture: Option<ID3D11Texture2D>,
    pub width: u32,
    pub height: u32,
}

pub struct CfrFrameMultiplexer {
    fps: u32,
    frame_interval_100ns: i64,
    next_expected_frame_index: AtomicU64,
    start_audio_time_100ns: Mutex<Option<i64>>,
    last_valid_frame: Mutex<Option<SynchronizedVideoFrame>>,
}

impl CfrFrameMultiplexer {
    pub fn new(fps: u32) -> Self {
        let frame_interval_100ns = (10_000_000i64) / (fps as i64);
        Self {
            fps,
            frame_interval_100ns,
            next_expected_frame_index: AtomicU64::new(0),
            start_audio_time_100ns: Mutex::new(None),
            last_valid_frame: Mutex::new(None),
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn frame_interval_100ns(&self) -> i64 {
        self.frame_interval_100ns
    }

    /// Process a new incoming GPU capture frame locked to the audio hardware master clock.
    /// Returns a vector of frames: the current frame, plus any duplicate frames needed to fill GPU drop gaps.
    pub fn process_frame(
        &self,
        texture: ID3D11Texture2D,
        raw_video_time_100ns: i64,
        audio_master_clock_100ns: i64,
        width: u32,
        height: u32,
    ) -> Vec<SynchronizedVideoFrame> {
        let mut start_lock = self.start_audio_time_100ns.lock().unwrap();
        let start_time = match *start_lock {
            Some(t) => t,
            None => {
                let init = if audio_master_clock_100ns > 0 {
                    audio_master_clock_100ns
                } else {
                    raw_video_time_100ns
                };
                *start_lock = Some(init);
                init
            }
        };

        let current_ref_time = if audio_master_clock_100ns > 0 {
            audio_master_clock_100ns
        } else {
            raw_video_time_100ns
        };

        let elapsed = (current_ref_time - start_time).max(0);
        let target_frame_index = (elapsed / self.frame_interval_100ns) as u64;

        let mut output_frames = Vec::new();
        let current_expected = self.next_expected_frame_index.load(Ordering::Relaxed);
        let mut last_frame_lock = self.last_valid_frame.lock().unwrap();

        // Check if GPU dropped cycles between last emitted frame and target frame
        if target_frame_index > current_expected && last_frame_lock.is_some() {
            let last = last_frame_lock.as_ref().unwrap();
            let missed_count = target_frame_index - current_expected;

            // Cap padding to prevent runaway memory if pause/sleep occurs
            let fill_count = missed_count.min(120);

            for i in 0..fill_count {
                let dup_idx = current_expected + i;
                let dup_pts = (dup_idx as i64) * self.frame_interval_100ns;
                output_frames.push(SynchronizedVideoFrame {
                    frame_index: dup_idx,
                    presentation_time_100ns: dup_pts,
                    is_duplicate: true,
                    texture: last.texture.clone(),
                    width: last.width,
                    height: last.height,
                });
            }
        }

        let actual_index = if target_frame_index >= current_expected {
            target_frame_index
        } else {
            current_expected
        };

        let actual_pts = (actual_index as i64) * self.frame_interval_100ns;
        let new_frame = SynchronizedVideoFrame {
            frame_index: actual_index,
            presentation_time_100ns: actual_pts,
            is_duplicate: false,
            texture: Some(texture),
            width,
            height,
        };

        output_frames.push(new_frame.clone());
        *last_frame_lock = Some(new_frame);
        self.next_expected_frame_index.store(actual_index + 1, Ordering::Relaxed);

        output_frames
    }

    pub fn reset(&self) {
        self.next_expected_frame_index.store(0, Ordering::Relaxed);
        *self.start_audio_time_100ns.lock().unwrap() = None;
        *self.last_valid_frame.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfr_nominal_60fps_pacing() {
        let mux = CfrFrameMultiplexer::new(60);
        let interval = mux.frame_interval_100ns();
        assert_eq!(interval, 166_666);

        // Frame 0 at audio time 0
        let frames0 = mux.process_frame_dummy(0, 0);
        assert_eq!(frames0.len(), 1);
        assert_eq!(frames0[0].frame_index, 0);
        assert!(!frames0[0].is_duplicate);

        // Frame 1 at audio time 166_666
        let frames1 = mux.process_frame_dummy(interval, interval);
        assert_eq!(frames1.len(), 1);
        assert_eq!(frames1[0].frame_index, 1);
        assert!(!frames1[0].is_duplicate);
    }

    #[test]
    fn test_cfr_drops_gpu_cycle_pads_duplicate() {
        let mux = CfrFrameMultiplexer::new(60);
        let interval = mux.frame_interval_100ns();

        let _ = mux.process_frame_dummy(0, 0);

        // GPU dropped 2 cycles, next frame arrives at 3 * interval
        let frames = mux.process_frame_dummy(interval * 3, interval * 3);
        // Expect 2 duplicates (index 1, 2) + 1 real frame (index 3) = 3 total frames
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].frame_index, 1);
        assert!(frames[0].is_duplicate);
        assert_eq!(frames[1].frame_index, 2);
        assert!(frames[1].is_duplicate);
        assert_eq!(frames[2].frame_index, 3);
        assert!(!frames[2].is_duplicate);
    }

    impl CfrFrameMultiplexer {
        pub fn process_frame_dummy(&self, _raw_time: i64, audio_time: i64) -> Vec<SynchronizedVideoFrame> {
            let mut start_lock = self.start_audio_time_100ns.lock().unwrap();
            let start_time = match *start_lock {
                Some(t) => t,
                None => {
                    let init = audio_time;
                    *start_lock = Some(init);
                    init
                }
            };

            let elapsed = (audio_time - start_time).max(0);
            let target_frame_index = (elapsed / self.frame_interval_100ns) as u64;

            let mut output_frames = Vec::new();
            let current_expected = self.next_expected_frame_index.load(Ordering::Relaxed);
            let mut last_frame_lock = self.last_valid_frame.lock().unwrap();

            if target_frame_index > current_expected && last_frame_lock.is_some() {
                let last = last_frame_lock.as_ref().unwrap();
                let missed_count = target_frame_index - current_expected;
                for i in 0..missed_count {
                    let dup_idx = current_expected + i;
                    let dup_pts = (dup_idx as i64) * self.frame_interval_100ns;
                    output_frames.push(SynchronizedVideoFrame {
                        frame_index: dup_idx,
                        presentation_time_100ns: dup_pts,
                        is_duplicate: true,
                        texture: None,
                        width: last.width,
                        height: last.height,
                    });
                }
            }

            let actual_index = if target_frame_index >= current_expected {
                target_frame_index
            } else {
                current_expected
            };

            let actual_pts = (actual_index as i64) * self.frame_interval_100ns;
            let new_frame = SynchronizedVideoFrame {
                frame_index: actual_index,
                presentation_time_100ns: actual_pts,
                is_duplicate: false,
                texture: None,
                width: 1920,
                height: 1080,
            };

            output_frames.push(new_frame.clone());
            *last_frame_lock = Some(new_frame);
            self.next_expected_frame_index.store(actual_index + 1, Ordering::Relaxed);

            output_frames
        }
    }
}
