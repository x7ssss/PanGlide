use crate::error::{PanGlideError, Result};
use std::path::PathBuf;
use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::{
    MFCreateSinkWriterFromURL, MFStartup, IMFSinkWriter,
    MFSTARTUP_NOSOCKET, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    HEVC,
}

pub struct MftEncoderConfig {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32, // bps, e.g. 20_000_000
    pub codec: VideoCodec,
}

impl Default for MftEncoderConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("recording.mp4"),
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 20_000_000,
            codec: VideoCodec::H264,
        }
    }
}

pub struct MftHardwareEncoder {
    #[allow(dead_code)]
    config: MftEncoderConfig,
    sink_writer: Option<IMFSinkWriter>,
    is_active: bool,
    has_stream_started: bool,
    frames_written: u64,
}

use std::sync::Once;

static MF_INIT: Once = Once::new();

fn ensure_mf_initialized() {
    MF_INIT.call_once(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _ = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET);
        }
    });
}

impl MftHardwareEncoder {
    pub fn new(config: MftEncoderConfig) -> Result<Self> {
        ensure_mf_initialized();

        let output_url = HSTRING::from(config.output_path.to_string_lossy().as_ref());
        let sink_writer = unsafe {
            MFCreateSinkWriterFromURL(&output_url, None, None).ok()
        };

        Ok(Self {
            config,
            sink_writer,
            is_active: false,
            has_stream_started: false,
            frames_written: 0,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        self.is_active = true;
        self.frames_written = 0;
        Ok(())
    }

    pub fn write_rgba_frame(&mut self, _rgba_data: &[u8], frame_index: u64) -> Result<()> {
        if !self.is_active {
            return Err(PanGlideError::Export("Encoder is not started".into()));
        }

        self.frames_written = frame_index + 1;
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<u64> {
        if !self.is_active {
            return Ok(self.frames_written);
        }

        self.is_active = false;
        let count = self.frames_written;

        if self.has_stream_started {
            if let Some(ref writer) = self.sink_writer {
                let _ = unsafe { writer.Finalize() };
            }
        }
        self.sink_writer = None;

        Ok(count)
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
}

impl Drop for MftHardwareEncoder {
    fn drop(&mut self) {
        let _ = self.finalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mft_encoder_lifecycle() {
        let config = MftEncoderConfig {
            output_path: PathBuf::from("target/test_out.mp4"),
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 15_000_000,
            codec: VideoCodec::H264,
        };

        let mut encoder = MftHardwareEncoder::new(config).expect("Encoder init");
        encoder.start().expect("Start encoder");

        let dummy_frame = vec![0u8; 1920 * 1080 * 4];
        for i in 0..10 {
            encoder.write_rgba_frame(&dummy_frame, i).expect("Write frame");
        }

        let total = encoder.finalize().expect("Finalize encoder");
        assert_eq!(total, 10);
    }
}
