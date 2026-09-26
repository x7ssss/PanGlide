use std::collections::VecDeque;

/// Real-time audio mixer that combines 48 kHz stereo 16-bit PCM audio streams
/// (WASAPI system loopback and WASAPI microphone input) using saturation clamping.
pub struct AudioMixer {
    mic_fifo: VecDeque<i16>,
    max_fifo_samples: usize,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMixer {
    /// Create a new AudioMixer.
    /// By default, caps FIFO to 19,200 samples (200ms of 48 kHz stereo audio)
    /// to prevent latency drift under clock skew.
    pub fn new() -> Self {
        Self {
            mic_fifo: VecDeque::with_capacity(19200),
            max_fifo_samples: 19200,
        }
    }

    pub fn with_capacity(max_samples: usize) -> Self {
        Self {
            mic_fifo: VecDeque::with_capacity(max_samples),
            max_fifo_samples: max_samples,
        }
    }

    /// Push microphone 16-bit stereo PCM samples into the mixer's FIFO buffer.
    pub fn push_mic_samples(&mut self, samples: &[i16]) {
        self.mic_fifo.extend(samples.iter().copied());
        if self.mic_fifo.len() > self.max_fifo_samples {
            let excess = self.mic_fifo.len() - self.max_fifo_samples;
            self.mic_fifo.drain(0..excess);
        }
    }

    /// Mix incoming loopback 16-bit stereo PCM samples with buffered microphone samples
    /// using saturation clamping to avoid integer overflow wraparound.
    pub fn mix_loopback_samples(&mut self, loopback: &[i16]) -> Vec<i16> {
        let mut mixed = Vec::with_capacity(loopback.len());
        for &s_loop in loopback {
            let s_mic = self.mic_fifo.pop_front().unwrap_or(0);
            let sum = (s_loop as i32) + (s_mic as i32);
            let s_out = sum.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            mixed.push(s_out);
        }
        mixed
    }

    /// Mix incoming loopback raw 16-bit little-endian PCM bytes with buffered microphone samples.
    pub fn mix_loopback_bytes(&mut self, loopback_bytes: &[u8]) -> Vec<u8> {
        if loopback_bytes.is_empty() {
            return Vec::new();
        }

        let num_samples = loopback_bytes.len() / 2;
        let mut out_bytes = Vec::with_capacity(loopback_bytes.len());

        for i in 0..num_samples {
            let s_loop = i16::from_le_bytes([loopback_bytes[i * 2], loopback_bytes[i * 2 + 1]]);
            let s_mic = self.mic_fifo.pop_front().unwrap_or(0);
            let sum = (s_loop as i32) + (s_mic as i32);
            let s_out = sum.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            out_bytes.extend_from_slice(&s_out.to_le_bytes());
        }

        out_bytes
    }

    /// Drain remaining buffered microphone samples as little-endian bytes (e.g. during final flush)
    pub fn drain_remaining_mic_bytes(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.mic_fifo.len() * 2);
        while let Some(s) = self.mic_fifo.pop_front() {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    /// Saturation clamp mixer for individual samples:
    /// Clamps sum into [-32768, 32767] to avoid wrap-around clipping distortion.
    #[inline(always)]
    pub fn clamp_mix(s1: i16, s2: i16) -> i16 {
        let sum = (s1 as i32) + (s2 as i32);
        sum.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    pub fn mic_buffer_len(&self) -> usize {
        self.mic_fifo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saturation_clamping() {
        // Positive saturation
        assert_eq!(AudioMixer::clamp_mix(30000, 10000), 32767);
        assert_eq!(AudioMixer::clamp_mix(32767, 1), 32767);

        // Negative saturation
        assert_eq!(AudioMixer::clamp_mix(-30000, -10000), -32768);
        assert_eq!(AudioMixer::clamp_mix(-32768, -1), -32768);

        // Normal linear addition
        assert_eq!(AudioMixer::clamp_mix(1000, 2000), 3000);
        assert_eq!(AudioMixer::clamp_mix(5000, -2000), 3000);
        assert_eq!(AudioMixer::clamp_mix(0, 0), 0);
    }

    #[test]
    fn test_mixer_with_empty_mic() {
        let mut mixer = AudioMixer::new();
        let loopback = vec![1000i16, -2000, 3000, -4000];
        let mixed = mixer.mix_loopback_samples(&loopback);
        assert_eq!(mixed, loopback);
    }

    #[test]
    fn test_mixer_with_buffered_mic() {
        let mut mixer = AudioMixer::new();
        mixer.push_mic_samples(&[500, 500, 500, 500]);
        let loopback = vec![1000i16, 2000, 3000, 4000];
        let mixed = mixer.mix_loopback_samples(&loopback);
        assert_eq!(mixed, vec![1500i16, 2500, 3500, 4500]);
    }

    #[test]
    fn test_mixer_bytes() {
        let mut mixer = AudioMixer::new();
        mixer.push_mic_samples(&[20000, -20000]);

        let loopback_samples = vec![20000i16, -20000i16];
        let mut loopback_bytes = Vec::new();
        for &s in &loopback_samples {
            loopback_bytes.extend_from_slice(&s.to_le_bytes());
        }

        let mixed_bytes = mixer.mix_loopback_bytes(&loopback_bytes);
        assert_eq!(mixed_bytes.len(), 4);

        let out_s0 = i16::from_le_bytes([mixed_bytes[0], mixed_bytes[1]]);
        let out_s1 = i16::from_le_bytes([mixed_bytes[2], mixed_bytes[3]]);

        assert_eq!(out_s0, 32767); // Clamped from 40000
        assert_eq!(out_s1, -32768); // Clamped from -40000
    }
}
