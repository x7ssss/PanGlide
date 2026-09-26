use thiserror::Error;

#[derive(Error, Debug)]
pub enum PanGlideError {
    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("Direct3D 11 initialization failed: {0}")]
    D3D11Init(String),

    #[error("Graphics capture error: {0}")]
    Capture(String),

    #[error("WASAPI audio error: {0}")]
    Audio(String),

    #[error("Input hook error: {0}")]
    Hook(String),

    #[error("Telemetry error: {0}")]
    Telemetry(String),

    #[error("Privacy engine error: {0}")]
    Privacy(String),

    #[error("Exporter error: {0}")]
    Export(String),

    #[error("License error: {0}")]
    License(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, PanGlideError>;
