pub mod audio_mixer;
pub mod cfr_multiplexer;
pub mod d3d11;
pub mod dxgi_fallback;
pub mod session;
pub mod wasapi_mic;
pub mod wgc;

pub use audio_mixer::AudioMixer;
pub use cfr_multiplexer::{CfrFrameMultiplexer, SynchronizedVideoFrame};
pub use d3d11::D3D11Context;
pub use dxgi_fallback::DxgiDesktopDuplication;
pub use session::{
    get_available_sources, get_recording_status, start_recording, stop_recording,
    trigger_live_snip, AutoBlurMarkerDto, CaptureSourceDto, RecordingResultDto,
    RecordingStatusDto, RejectedTakeDto, ZoomKeyframeDto,
};
pub use wasapi_mic::WasapiMicRecorder;
pub use wgc::{WgcCaptureSession, WgcFrame};
