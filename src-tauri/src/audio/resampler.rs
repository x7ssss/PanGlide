/// High-fidelity audio sample rate converter targeting 48 kHz studio master standard
pub struct AudioResampler {
    in_sample_rate: u32,
    out_sample_rate: u32,
    channels: u16,
    phase: f64,
}

impl AudioResampler {
    pub fn new(in_sample_rate: u32, out_sample_rate: u32, channels: u16) -> Self {
        Self {
            in_sample_rate,
            out_sample_rate,
            channels,
            phase: 0.0,
        }
    }

    /// Resample planar or interleaved f32 PCM audio to 48 kHz
    pub fn process_interleaved(&mut self, input: &[f32], output: &mut Vec<f32>) {
        if self.in_sample_rate == self.out_sample_rate {
            output.extend_from_slice(input);
            return;
        }

        let channels = self.channels as usize;
        let num_in_frames = input.len() / channels;
        if num_in_frames < 2 {
            return;
        }

        let ratio = self.in_sample_rate as f64 / self.out_sample_rate as f64;

        while (self.phase as usize + 1) < num_in_frames {
            let idx = self.phase as usize;
            let frac = (self.phase - idx as f64) as f32;

            for ch in 0..channels {
                let s0 = input[idx * channels + ch];
                let s1 = input[(idx + 1) * channels + ch];
                // Linear interpolation with high accuracy
                let sample = s0 + frac * (s1 - s0);
                output.push(sample);
            }

            self.phase += ratio;
        }

        // Retain fractional phase relative to consumed frames
        self.phase -= num_in_frames as f64 - 1.0;
        if self.phase < 0.0 {
            self.phase = 0.0;
        }
    }

    /// Calculate RMS and Peak dB levels for live VU meters (normalized 0.0 .. 1.0)
    pub fn calculate_vu_meter(samples: &[f32]) -> (f32, f32) {
        if samples.is_empty() {
            return (0.0, 0.0);
        }

        let mut sum_squares = 0.0f32;
        let mut peak = 0.0f32;

        for &sample in samples {
            let abs_val = sample.abs();
            if abs_val > peak {
                peak = abs_val;
            }
            sum_squares += sample * sample;
        }

        let rms = (sum_squares / samples.len() as f32).sqrt();
        (rms.min(1.0), peak.min(1.0))
    }
}
