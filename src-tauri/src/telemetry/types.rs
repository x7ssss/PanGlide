use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEventType {
    Move = 0,
    MouseDown = 1,
    MouseUp = 2,
    KeyDown = 3,
    KeyUp = 4,
    Wheel = 5,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InputEvent {
    pub timestamp_us: u64,
    pub event_type: InputEventType, // Move, MouseDown, MouseUp, KeyDown, KeyUp, Wheel
    pub x: f32, // Normalized display coordinates (0.0 to 1.0)
    pub y: f32,
    pub button: u8,
    pub key_code: u32,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseAction {
    Move = 0,
    LeftDown = 1,
    LeftUp = 2,
    RightDown = 3,
    RightUp = 4,
    MiddleDown = 5,
    MiddleUp = 6,
    Wheel = 7,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseTelemetryEvent {
    pub timestamp_us: u64,
    pub raw_x: i32,
    pub raw_y: i32,
    pub norm_x: f32,
    pub norm_y: f32,
    pub action: MouseAction,
    pub wheel_delta: i16,
    pub is_drag: bool,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAction {
    Down = 0,
    Up = 1,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierState {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool, // Windows key
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardTelemetryEvent {
    pub timestamp_us: u64,
    pub vk_code: u32,
    pub scan_code: u32,
    pub action: KeyAction,
    pub modifiers: ModifierState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedTakeMarker {
    pub marker_id: u32,
    pub start_timestamp_us: u64,
    pub end_timestamp_us: u64,
    pub duration_seconds: f32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TelemetryRecord {
    Mouse(MouseTelemetryEvent),
    Keyboard(KeyboardTelemetryEvent),
    LiveSnip(RejectedTakeMarker),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryMetadata {
    pub frame_count: u64,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub dpi_scale: f32,
    pub display_width: u32,
    pub display_height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySidecar {
    pub metadata: TelemetryMetadata,
    pub rejected_takes: Vec<RejectedTakeMarker>,
    pub events: Vec<InputEvent>,
}
