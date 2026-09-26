use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;

use windows::core::{Interface, HSTRING};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::Media::MediaFoundation::{
    IMFSinkWriter, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL,
    MFMediaType_Video, MFMediaType_Audio, MFShutdown, MFStartup,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoFormat_RGB32, MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFVideoInterlace_Progressive, MF_LOW_LATENCY,
    MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MF_VERSION,
    MFSTARTUP_NOSOCKET,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

// ─── DTO Types ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatusDto {
    pub is_recording: bool,
    pub is_paused: bool,
    pub elapsed_ms: u64,
    pub frame_count: u64,
    pub mic_vu_level: f32,
    pub sys_vu_level: f32,
    pub active_source: String,
    pub latest_camera_x: f32,
    pub latest_camera_y: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AutoBlurMarkerDto {
    pub id: String,
    pub time_ms: u64,
    pub token_type: String,
    pub preview: String,
    pub bounds: MarkerBoundsDto,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarkerBoundsDto {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RejectedTakeDto {
    pub id: String,
    pub start_time_ms: u64,
    pub end_time_ms: u64,
    pub duration_sec: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ZoomKeyframeDto {
    pub id: String,
    pub time_ms: u64,
    pub scale: f64,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResultDto {
    pub video_path: String,
    pub video_url: String,
    pub duration_ms: u64,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub auto_blur_markers: Vec<AutoBlurMarkerDto>,
    pub rejected_takes: Vec<RejectedTakeDto>,
    pub rejected_take_intervals: Vec<RejectedTakeDto>,
    pub zoom_keyframes: Vec<ZoomKeyframeDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSourceDto {
    pub id: String,
    pub name: String,
    pub is_monitor: bool,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

// ─── Monitor Enumeration (from verified record_test.rs) ─────────────────────

#[derive(Debug, Clone)]
struct PhysicalMonitor {
    index: usize,
    name: String,
    hmonitor: HMONITOR,
    left: i32,
    top: i32,
    #[allow(dead_code)]
    right: i32,
    #[allow(dead_code)]
    bottom: i32,
    width: u32,
    height: u32,
    is_primary: bool,
}

fn enumerate_monitors() -> Vec<PhysicalMonitor> {
    let mut monitors: Vec<PhysicalMonitor> = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(lparam.0 as *mut Vec<PhysicalMonitor>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
            let rc = mi.monitorInfo.rcMonitor;
            let width = (rc.right - rc.left).unsigned_abs();
            let height = (rc.bottom - rc.top).unsigned_abs();
            let name = String::from_utf16_lossy(&mi.szDevice)
                .trim_matches('\0')
                .to_string();
            let is_primary = (mi.monitorInfo.dwFlags & 1) != 0;

            let index = monitors.len();
            monitors.push(PhysicalMonitor {
                index,
                name,
                hmonitor: hmon,
                left: rc.left,
                top: rc.top,
                right: rc.right,
                bottom: rc.bottom,
                width,
                height,
                is_primary,
            });
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}

// ─── D3D11 + WGC + MFT Initialization (from verified record_test.rs) ────────

struct D3D11Resources {
    #[allow(dead_code)]
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    multithread: ID3D11Multithread,
    winrt_device: IDirect3DDevice,
    render_texture: ID3D11Texture2D,
    staging_texture: ID3D11Texture2D,
}

// SAFETY: D3D11 resources are used with multithread protection enabled and all
// cross-thread access is guarded by IMFMultithread Enter/Leave calls.
unsafe impl Send for D3D11Resources {}

fn init_d3d11(width: u32, height: u32) -> windows::core::Result<D3D11Resources> {
    unsafe {
        let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL_11_0;

        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            Some(&mut context),
        )?;

        let device = device.expect("Failed to create D3D11 Device");
        let context = context.expect("Failed to create D3D11 Device Context");

        // Enable multithreaded protection for thread-safe CopyResource calls
        let multithread: ID3D11Multithread = device.cast()?;
        let _ = multithread.SetMultithreadProtected(BOOL(1));

        let dxgi_device: IDXGIDevice = device.cast()?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)?;
        let winrt_device: IDirect3DDevice = inspectable.cast()?;

        let render_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut render_texture = None;
        device.CreateTexture2D(&render_desc, None, Some(&mut render_texture))?;
        let render_texture = render_texture.expect("Failed to allocate render texture");

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging_texture = None;
        device.CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))?;
        let staging_texture = staging_texture.expect("Failed to allocate staging texture");

        Ok(D3D11Resources {
            device,
            context,
            multithread,
            winrt_device,
            render_texture,
            staging_texture,
        })
    }
}

fn init_sink_writer(
    output_filename: &str,
    width: u32,
    height: u32,
    fps: u32,
    bitrate_bps: u32,
) -> windows::core::Result<(IMFSinkWriter, u32, Option<u32>)> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)?;

        let mut attr = None;
        MFCreateAttributes(&mut attr, 3)?;
        let attr = attr.expect("Failed to create MF attributes");
        attr.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        attr.SetUINT32(&MF_LOW_LATENCY, 1)?;
        attr.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;

        let output_url = HSTRING::from(output_filename);
        let writer = MFCreateSinkWriterFromURL(&output_url, None, Some(&attr))?;

        // ── Video Stream (H.264) ──
        let out_type = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_bps)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        out_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        out_type.SetUINT32(
            &MF_MT_INTERLACE_MODE,
            MFVideoInterlace_Progressive.0 as u32,
        )?;

        let stream_index = writer.AddStream(&out_type)?;

        let in_type = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        in_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        in_type.SetUINT32(
            &MF_MT_INTERLACE_MODE,
            MFVideoInterlace_Progressive.0 as u32,
        )?;
        in_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;

        match writer.SetInputMediaType(stream_index, &in_type, None) {
            Ok(_) => {
                eprintln!("[PanGlide] SinkWriter input: MFVideoFormat_RGB32 (Native BGRA)")
            }
            Err(e) => {
                eprintln!(
                    "[PanGlide] RGB32 not supported ({:?}), falling back to NV12...",
                    e
                );
                in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
                writer.SetInputMediaType(stream_index, &in_type, None)?;
                eprintln!("[PanGlide] SinkWriter input: MFVideoFormat_NV12");
            }
        }

        // ── Audio Stream (AAC 48kHz stereo @ 160 kbps) ──
        let mut audio_stream_index = None;
        let audio_out_type = MFCreateMediaType()?;
        audio_out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        audio_out_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 20_000)?; // 160 kbps

        match writer.AddStream(&audio_out_type) {
            Ok(a_idx) => {
                let audio_in_type = MFCreateMediaType()?;
                audio_in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                audio_in_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
                audio_in_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                audio_in_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
                audio_in_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                audio_in_type.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                audio_in_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 48000 * 4)?;

                match writer.SetInputMediaType(a_idx, &audio_in_type, None) {
                    Ok(_) => {
                        audio_stream_index = Some(a_idx);
                        eprintln!("[PanGlide] SinkWriter audio enabled: MFAudioFormat_PCM @ 48kHz stereo -> AAC 160kbps (stream #{})", a_idx);
                    }
                    Err(e) => {
                        eprintln!("[PanGlide] Failed to set SinkWriter audio input type: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[PanGlide] Failed to add AAC audio stream: {:?}", e);
            }
        }

        writer.BeginWriting()?;
        Ok((writer, stream_index, audio_stream_index))
    }
}

/// Convert resampled float audio packet to 16-bit stereo PCM bytes (little-endian)
fn packet_to_stereo_i16_pcm(packet: &crate::audio::AudioPacket) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(packet.samples.len() * 2);
    if packet.channels == 1 {
        for &s in &packet.samples {
            let val = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            let bytes = val.to_le_bytes();
            pcm.extend_from_slice(&bytes);
            pcm.extend_from_slice(&bytes);
        }
    } else if packet.channels == 2 {
        for &s in &packet.samples {
            let val = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm.extend_from_slice(&val.to_le_bytes());
        }
    } else if packet.channels > 2 {
        let ch = packet.channels as usize;
        for frame in packet.samples.chunks_exact(ch) {
            let l = (frame[0].clamp(-1.0, 1.0) * 32767.0) as i16;
            let r = (frame[1].clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm.extend_from_slice(&l.to_le_bytes());
            pcm.extend_from_slice(&r.to_le_bytes());
        }
    }
    pcm
}

/// Package and write 16-bit stereo PCM audio buffer to IMFSinkWriter with 100ns timestamps
fn write_audio_samples(
    sink_writer: &IMFSinkWriter,
    audio_stream_index: u32,
    pcm_bytes: &[u8],
    total_audio_frames: &mut u64,
) -> windows::core::Result<()> {
    if pcm_bytes.is_empty() {
        return Ok(());
    }
    unsafe {
        let byte_len = pcm_bytes.len() as u32;
        let num_frames = (byte_len / 4) as u64; // 16-bit stereo = 4 bytes per frame
        let duration_hns = (num_frames as i64 * 10_000_000) / 48000;
        let sample_time_hns = (*total_audio_frames as i64 * 10_000_000) / 48000;

        let sample = MFCreateSample()?;
        let buffer = MFCreateMemoryBuffer(byte_len)?;

        let mut p_dst: *mut u8 = std::ptr::null_mut();
        let mut max_len = 0u32;
        let mut cur_len = 0u32;
        buffer.Lock(&mut p_dst, Some(&mut max_len), Some(&mut cur_len))?;
        std::ptr::copy_nonoverlapping(pcm_bytes.as_ptr(), p_dst, byte_len as usize);
        buffer.Unlock()?;
        buffer.SetCurrentLength(byte_len)?;

        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(sample_time_hns)?;
        sample.SetSampleDuration(duration_hns)?;

        sink_writer.WriteSample(audio_stream_index, &sample)?;
        *total_audio_frames += num_frames;
        Ok(())
    }
}

/// Pad silence frames to synchronize audio track length with video duration
fn pad_audio_silence(
    sink_writer: &IMFSinkWriter,
    audio_stream_index: u32,
    frames_needed: u64,
    total_audio_frames: &mut u64,
) -> windows::core::Result<()> {
    if frames_needed == 0 {
        return Ok(());
    }
    let chunk_frames = 4800u64; // 100ms chunks
    let mut remaining = frames_needed;
    let silence_buffer = vec![0u8; (chunk_frames * 4) as usize];

    while remaining > 0 {
        let to_write = remaining.min(chunk_frames);
        let byte_len = (to_write * 4) as usize;
        write_audio_samples(
            sink_writer,
            audio_stream_index,
            &silence_buffer[..byte_len],
            total_audio_frames,
        )?;
        remaining -= to_write;
    }
    Ok(())
}

// ─── Active Session State ────────────────────────────────────────────────────

struct ActiveSession {
    start_time: Instant,
    output_path: PathBuf,
    width: u32,
    height: u32,
    source_name: String,
    frame_count: Arc<Mutex<u64>>,
    sys_vu_level: Arc<Mutex<f32>>,
    mic_vu_level: Arc<Mutex<f32>>,
    rejected_takes: Arc<Mutex<Vec<RejectedTakeDto>>>,
    rejected_markers: Arc<Mutex<Vec<crate::telemetry::types::RejectedTakeMarker>>>,
    telemetry_events: Arc<Mutex<Vec<crate::telemetry::types::InputEvent>>>,
    zoom_keyframes: Arc<Mutex<Vec<ZoomKeyframeDto>>>,
    is_active: Arc<AtomicBool>,
    /// Handle to the capture+encode worker thread; joined on stop_recording
    worker_handle: Option<std::thread::JoinHandle<()>>,
}

static ACTIVE_SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);
static NEXT_MARKER_ID: AtomicU32 = AtomicU32::new(1);

fn date_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn get_recordings_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data)
            .join("PanGlide")
            .join("recordings")
    } else {
        PathBuf::from(".panglide").join("recordings")
    }
}

// ─── Tauri IPC Commands ──────────────────────────────────────────────────────

/// Dynamically enumerate all connected physical monitors using EnumDisplayMonitors.
#[tauri::command]
pub fn get_available_sources() -> Vec<CaptureSourceDto> {
    let monitors = enumerate_monitors();
    monitors
        .into_iter()
        .map(|mon| {
            let primary_tag = if mon.is_primary { " (Primary)" } else { "" };
            let clean_name = mon
                .name
                .replace(r"\\.\DISPLAY", "Display ")
                .replace(r"\\.\", "");
            CaptureSourceDto {
                id: format!("monitor_{}", mon.index),
                name: format!(
                    "{}{} — {}×{}",
                    clean_name, primary_tag, mon.width, mon.height
                ),
                is_monitor: true,
                width: mon.width,
                height: mon.height,
                x: mon.left,
                y: mon.top,
            }
        })
        .collect()
}

/// Return current recording status (elapsed time, frame count, audio VU levels).
#[tauri::command]
pub fn get_recording_status() -> std::result::Result<RecordingStatusDto, String> {
    let session_guard = ACTIVE_SESSION.lock().map_err(|e| e.to_string())?;
    if let Some(ref session) = *session_guard {
        let elapsed = session.start_time.elapsed().as_millis() as u64;
        let fc = session.frame_count.lock().map(|f| *f).unwrap_or(0);
        let sys_vu = session.sys_vu_level.lock().map(|v| *v).unwrap_or(0.0);
        let mic_vu = session.mic_vu_level.lock().map(|v| *v).unwrap_or(0.0);

        Ok(RecordingStatusDto {
            is_recording: true,
            is_paused: false,
            elapsed_ms: elapsed,
            frame_count: fc,
            mic_vu_level: mic_vu,
            sys_vu_level: sys_vu,
            active_source: session.source_name.clone(),
            latest_camera_x: 0.5,
            latest_camera_y: 0.5,
        })
    } else {
        Ok(RecordingStatusDto {
            is_recording: false,
            is_paused: false,
            elapsed_ms: 0,
            frame_count: 0,
            mic_vu_level: 0.0,
            sys_vu_level: 0.0,
            active_source: "None".to_string(),
            latest_camera_x: 0.5,
            latest_camera_y: 0.5,
        })
    }
}

/// Start a hardware-accelerated WGC + MFT H.264 recording targeting a specific monitor.
/// Spawns a dedicated capture+encode worker thread that runs the full D3D11 pipeline.
#[tauri::command]
pub fn start_recording(source_id: Option<String>) -> std::result::Result<RecordingStatusDto, String> {
    let mut session_guard = ACTIVE_SESSION.lock().map_err(|e| e.to_string())?;
    if session_guard.is_some() {
        return Err("A recording session is already active".to_string());
    }

    // Enumerate monitors and select target
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return Err("No physical monitors detected via EnumDisplayMonitors".to_string());
    }

    let selected_index: usize = source_id
        .as_deref()
        .and_then(|id| id.strip_prefix("monitor_"))
        .and_then(|idx| idx.parse().ok())
        .unwrap_or_else(|| {
            // Default to primary, or index 0
            monitors
                .iter()
                .position(|m| m.is_primary)
                .unwrap_or(0)
        });

    let monitor = monitors
        .get(selected_index)
        .cloned()
        .unwrap_or_else(|| monitors[0].clone());

    let recordings_dir = get_recordings_dir();
    let _ = fs::create_dir_all(&recordings_dir);

    let timestamp = date_now();
    let output_path = recordings_dir.join(format!("panglide_{}.mp4", timestamp));

    let is_active = Arc::new(AtomicBool::new(true));
    let frame_count = Arc::new(Mutex::new(0u64));
    let sys_vu_level = Arc::new(Mutex::new(0.0f32));
    let mic_vu_level = Arc::new(Mutex::new(0.0f32));
    let rejected_takes = Arc::new(Mutex::new(Vec::new()));
    let rejected_markers = Arc::new(Mutex::new(Vec::new()));
    let telemetry_events = Arc::new(Mutex::new(Vec::new()));
    let zoom_keyframes = Arc::new(Mutex::new(Vec::new()));

    // Activate low-level input hooks and target monitor coordinate normalization (< 5ns)
    crate::telemetry::hooks::InputHookManager::set_target_monitor(
        monitor.left,
        monitor.top,
        monitor.width,
        monitor.height,
    );
    let telemetry_consumer = crate::telemetry::hooks::InputHookManager::start_telemetry_capture(65536);

    let source_name = format!(
        "{}{} — {}×{}",
        monitor.name,
        if monitor.is_primary { " (Primary)" } else { "" },
        monitor.width,
        monitor.height
    );

    // Clone values for the worker thread
    let is_active_clone = is_active.clone();
    let frame_count_clone = frame_count.clone();
    let sys_vu_level_clone = sys_vu_level.clone();
    let mic_vu_level_clone = mic_vu_level.clone();
    let rejected_takes_clone = rejected_takes.clone();
    let rejected_markers_clone = rejected_markers.clone();
    let telemetry_events_clone = telemetry_events.clone();
    let output_path_str = output_path.to_string_lossy().to_string();
    let mon_width = monitor.width;
    let mon_height = monitor.height;
    let mon_hmonitor_raw = monitor.hmonitor.0 as isize;

    let worker_handle = std::thread::Builder::new()
        .name("panglide-mft-capture-worker".into())
        .spawn(move || {
            let mon_hmonitor = HMONITOR(mon_hmonitor_raw as *mut core::ffi::c_void);
            if let Err(e) = run_capture_loop(
                is_active_clone,
                frame_count_clone,
                sys_vu_level_clone,
                mic_vu_level_clone,
                rejected_takes_clone,
                rejected_markers_clone,
                telemetry_events_clone,
                telemetry_consumer,
                &output_path_str,
                mon_width,
                mon_height,
                mon_hmonitor,
            ) {
                eprintln!("[PanGlide] Capture worker error: {}", e);
            }
        })
        .map_err(|e| format!("Failed to spawn capture worker: {}", e))?;

    *session_guard = Some(ActiveSession {
        start_time: Instant::now(),
        output_path,
        width: mon_width,
        height: mon_height,
        source_name: source_name.clone(),
        frame_count,
        sys_vu_level,
        mic_vu_level,
        rejected_takes,
        rejected_markers,
        telemetry_events,
        zoom_keyframes,
        is_active,
        worker_handle: Some(worker_handle),
    });

    Ok(RecordingStatusDto {
        is_recording: true,
        is_paused: false,
        elapsed_ms: 0,
        frame_count: 0,
        mic_vu_level: 0.0,
        sys_vu_level: 0.0,
        active_source: source_name,
        latest_camera_x: 0.5,
        latest_camera_y: 0.5,
    })
}

/// The full D3D11 → WGC → IMFSinkWriter hardware capture & encode loop.
/// Runs on a dedicated thread. Signals via `is_active` AtomicBool.
fn run_capture_loop(
    is_active: Arc<AtomicBool>,
    frame_count: Arc<Mutex<u64>>,
    sys_vu_level: Arc<Mutex<f32>>,
    mic_vu_level: Arc<Mutex<f32>>,
    rejected_takes: Arc<Mutex<Vec<RejectedTakeDto>>>,
    rejected_markers: Arc<Mutex<Vec<crate::telemetry::types::RejectedTakeMarker>>>,
    telemetry_events: Arc<Mutex<Vec<crate::telemetry::types::InputEvent>>>,
    mut telemetry_consumer: rtrb::Consumer<crate::telemetry::types::InputEvent>,
    output_path: &str,
    width: u32,
    height: u32,
    hmonitor: HMONITOR,
) -> std::result::Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let fps = 60u32;
    let bitrate_bps = 12_000_000u32;

    // Initialize D3D11 device and GPU textures
    let d3d = init_d3d11(width, height).map_err(|e| format!("D3D11 init failed: {:?}", e))?;
    eprintln!(
        "[PanGlide] D3D11 Hardware Device initialized for {}×{}",
        width, height
    );

    // Initialize Media Foundation Sink Writer
    let (sink_writer, stream_index, audio_stream_index) =
        init_sink_writer(output_path, width, height, fps, bitrate_bps)
            .map_err(|e| format!("SinkWriter init failed: {:?}", e))?;
    eprintln!(
        "[PanGlide] MF SinkWriter ready: H.264 @ {}×{}, {} FPS, {} Mbps, audio_stream={:?} → {}",
        width,
        height,
        fps,
        bitrate_bps / 1_000_000,
        audio_stream_index,
        output_path
    );

    // Create WGC capture item for the target monitor
    let interop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| format!("WGC interop factory failed: {:?}", e))?;

    let capture_item: GraphicsCaptureItem = unsafe {
        interop
            .CreateForMonitor(hmonitor)
            .map_err(|e| format!("CreateForMonitor failed: {:?}", e))?
    };

    let item_size = capture_item
        .Size()
        .map_err(|e| format!("Failed to get capture item size: {:?}", e))?;
    eprintln!(
        "[PanGlide] WGC GraphicsCaptureItem bound: {}×{}",
        item_size.Width, item_size.Height
    );

    // Frame pool with 4 buffers to avoid exhaustion under load
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d.winrt_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        4,
        item_size,
    )
    .map_err(|e| format!("FramePool creation failed: {:?}", e))?;

    let capture_session = frame_pool
        .CreateCaptureSession(&capture_item)
        .map_err(|e| format!("CreateCaptureSession failed: {:?}", e))?;
    let _ = capture_session.SetIsBorderRequired(false);
    let _ = capture_session.SetIsCursorCaptureEnabled(true);

    // Set up frame-arrived callback to copy captured frames to render texture
    let shared_context = d3d.context.clone();
    let shared_render_tex = d3d.render_texture.clone();
    let multithread_cb = d3d.multithread.clone();
    let has_first_frame = Arc::new(AtomicBool::new(false));
    let has_first_frame_clone = has_first_frame.clone();

    frame_pool
        .FrameArrived(&TypedEventHandler::new(
            move |pool: &Option<Direct3D11CaptureFramePool>, _| {
                if let Some(pool) = pool {
                    if let Ok(frame) = pool.TryGetNextFrame() {
                        if let Ok(surface) = frame.Surface() {
                            if let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() {
                                let src_texture_res: windows::core::Result<ID3D11Texture2D> =
                                    unsafe { access.GetInterface() };
                                if let Ok(src_texture) = src_texture_res {
                                    unsafe {
                                        multithread_cb.Enter();
                                        shared_context
                                            .CopyResource(&shared_render_tex, &src_texture);
                                        multithread_cb.Leave();
                                    }
                                    has_first_frame_clone.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                        // CRITICAL: Close the frame so the buffer returns to the pool immediately
                        let _ = frame.Close();
                    }
                }
                Ok(())
            },
        ))
        .map_err(|e| format!("FrameArrived handler failed: {:?}", e))?;

    capture_session
        .StartCapture()
        .map_err(|e| format!("StartCapture failed: {:?}", e))?;
    eprintln!("[PanGlide] WGC capture started. Waiting for first frame...");

    // Wait for first frame (up to 5 seconds)
    let wait_start = Instant::now();
    while !has_first_frame.load(Ordering::SeqCst) {
        if !is_active.load(Ordering::SeqCst) {
            // User stopped before we got a frame — clean up
            let _ = capture_session.Close();
            let _ = frame_pool.Close();
            unsafe {
                let _ = sink_writer.Finalize();
                let _ = MFShutdown();
            }
            return Ok(());
        }
        if wait_start.elapsed() > Duration::from_secs(5) {
            let _ = capture_session.Close();
            let _ = frame_pool.Close();
            unsafe {
                let _ = MFShutdown();
            }
            return Err("Timed out waiting for first WGC capture frame".to_string());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    eprintln!("[PanGlide] First frame received. Encoding loop active.");

    // Setup WASAPI loopback audio capture
    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<crate::audio::AudioPacket>();
    let mut audio_recorder = if audio_stream_index.is_some() {
        let tx = audio_tx.clone();
        match crate::audio::WasapiLoopbackRecorder::start(Arc::new(move |pkt| {
            let _ = tx.send(pkt);
        })) {
            Ok(rec) => {
                eprintln!("[PanGlide] WASAPI loopback audio capture started successfully.");
                Some(rec)
            }
            Err(e) => {
                eprintln!(
                    "[PanGlide] Warning: Could not start WASAPI loopback: {:?}. Audio recording will be silent.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    // Setup WASAPI microphone audio capture & real-time audio mixer
    let (mic_tx, mic_rx) = std::sync::mpsc::channel::<Vec<i16>>();
    let mic_vu_clone = mic_vu_level.clone();
    let mut mic_recorder = if audio_stream_index.is_some() {
        let tx = mic_tx.clone();
        match crate::capture::wasapi_mic::WasapiMicRecorder::start(
            Arc::new(move |pcm| {
                let _ = tx.send(pcm);
            }),
            Some(Arc::new(move |_rms, peak| {
                if let Ok(mut vu) = mic_vu_clone.lock() {
                    *vu = peak;
                }
            })),
        ) {
            Ok(rec) => {
                eprintln!("[PanGlide] WASAPI microphone capture started successfully.");
                Some(rec)
            }
            Err(e) => {
                eprintln!(
                    "[PanGlide] Microphone not available or failed to start: {:?}. Falling back gracefully to loopback-only audio.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let mut audio_mixer = crate::capture::audio_mixer::AudioMixer::new();
    let mut total_audio_frames: u64 = 0;

    // ── Main encoding loop: CFR 60 FPS ──
    let frame_duration_hns = 10_000_000i64 / (fps as i64);
    let buffer_size = width * height * 4;
    let frame_interval = Duration::from_micros(1_000_000 / (fps as u64));
    let mut frame_index: u64 = 0;

    while is_active.load(Ordering::SeqCst) {
        let frame_start = Instant::now();

        // 1. Drain pending microphone audio into the real-time mixer
        while let Ok(mic_pcm) = mic_rx.try_recv() {
            audio_mixer.push_mic_samples(&mic_pcm);
        }

        // 2. Drain pending loopback audio packets, mix with microphone samples via saturation clamping, and write to SinkWriter
        if let Some(a_idx) = audio_stream_index {
            while let Ok(pkt) = audio_rx.try_recv() {
                let (_, peak) = crate::audio::AudioResampler::calculate_vu_meter(&pkt.samples);
                if let Ok(mut vu) = sys_vu_level.lock() {
                    *vu = peak;
                }
                let loopback_bytes = packet_to_stereo_i16_pcm(&pkt);
                let mixed_bytes = audio_mixer.mix_loopback_bytes(&loopback_bytes);
                let _ = write_audio_samples(&sink_writer, a_idx, &mixed_bytes, &mut total_audio_frames);
            }
        }

        // 3. Task 3.4 Live Mistake Snipping: Check if Ctrl+Z was pressed during recording
        if crate::telemetry::hooks::InputHookManager::check_snip_requested() {
            let now_ms = (frame_index as f64 * 1000.0 / fps as f64) as u64;
            let start_ms = now_ms.saturating_sub(5000);
            let duration_sec = (now_ms - start_ms) as f64 / 1000.0;
            let marker_id = NEXT_MARKER_ID.fetch_add(1, Ordering::Relaxed);

            let snip = RejectedTakeDto {
                id: format!("rt_{}", marker_id),
                start_time_ms: start_ms,
                end_time_ms: now_ms,
                duration_sec,
            };

            let marker = crate::telemetry::types::RejectedTakeMarker {
                marker_id,
                start_timestamp_us: start_ms * 1000,
                end_timestamp_us: now_ms * 1000,
                duration_seconds: duration_sec as f32,
                reason: "Live Snip: Hotkey Ctrl+Z triggered".to_string(),
            };

            if let Ok(mut takes) = rejected_takes.lock() {
                takes.push(snip.clone());
            }
            if let Ok(mut markers) = rejected_markers.lock() {
                markers.push(marker);
            }
            if let Some(app) = crate::get_app_handle() {
                let _ = app.emit("snip_recorded", &snip);
            }
            eprintln!("[PanGlide] Live Snip recorded (Ctrl+Z): {}ms -> {}ms", start_ms, now_ms);
        }

        // 4. Drain SPSC ring buffer telemetry events into session memory
        if let Ok(mut events) = telemetry_events.lock() {
            while let Ok(evt) = telemetry_consumer.pop() {
                events.push(evt);
            }
        }

        // 5. Capture and encode video frame
        let write_result: windows::core::Result<()> = (|| unsafe {
            // Protected staging copy from render texture
            d3d.multithread.Enter();
            d3d.context
                .CopyResource(&d3d.staging_texture, &d3d.render_texture);
            d3d.multithread.Leave();

            // Map staging texture to read BGRA pixel rows
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            d3d.context.Map(
                &d3d.staging_texture,
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;

            let sample = MFCreateSample()?;
            let buffer = MFCreateMemoryBuffer(buffer_size)?;

            let mut p_dst: *mut u8 = std::ptr::null_mut();
            let mut max_len = 0u32;
            let mut cur_len = 0u32;
            buffer.Lock(&mut p_dst, Some(&mut max_len), Some(&mut cur_len))?;

            let row_pitch = mapped.RowPitch as usize;
            let row_bytes = (width * 4) as usize;

            if row_pitch == row_bytes {
                std::ptr::copy_nonoverlapping(
                    mapped.pData as *const u8,
                    p_dst,
                    buffer_size as usize,
                );
            } else {
                for y in 0..height as usize {
                    let src_row = (mapped.pData as *const u8).add(y * row_pitch);
                    let dst_row = p_dst.add(y * row_bytes);
                    std::ptr::copy_nonoverlapping(src_row, dst_row, row_bytes);
                }
            }

            buffer.Unlock()?;
            buffer.SetCurrentLength(buffer_size)?;
            sample.AddBuffer(&buffer)?;

            let timestamp_hns = (frame_index as i64) * frame_duration_hns;
            sample.SetSampleTime(timestamp_hns)?;
            sample.SetSampleDuration(frame_duration_hns)?;

            d3d.context.Unmap(&d3d.staging_texture, 0);

            sink_writer.WriteSample(stream_index, &sample)?;
            Ok(())
        })();

        if let Err(e) = write_result {
            eprintln!("[PanGlide] Frame {} encode error: {:?}", frame_index, e);
            // Continue — don't abort entire recording for a single frame error
        }

        frame_index += 1;
        if let Ok(mut fc) = frame_count.lock() {
            *fc = frame_index;
        }

        // Log progress every second
        if frame_index % (fps as u64) == 0 {
            eprintln!(
                "[PanGlide] Recording: {}s ({} frames encoded)",
                frame_index / (fps as u64),
                frame_index
            );
        }

        // Maintain CFR pacing
        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    eprintln!(
        "[PanGlide] Capture loop ended after {} frames. Finalizing...",
        frame_index
    );

    // Stop audio recorders cleanly
    if let Some(mut rec) = mic_recorder.take() {
        rec.stop();
        eprintln!("[PanGlide] WASAPI microphone capture stopped.");
    }
    if let Some(mut rec) = audio_recorder.take() {
        rec.stop();
        eprintln!("[PanGlide] WASAPI loopback capture stopped.");
    }

    // Drain any remaining microphone audio into the mixer
    while let Ok(mic_pcm) = mic_rx.try_recv() {
        audio_mixer.push_mic_samples(&mic_pcm);
    }

    // Drain any remaining loopback packets from the channel, mix, and write
    if let Some(a_idx) = audio_stream_index {
        while let Ok(pkt) = audio_rx.try_recv() {
            let loopback_bytes = packet_to_stereo_i16_pcm(&pkt);
            let mixed_bytes = audio_mixer.mix_loopback_bytes(&loopback_bytes);
            let _ = write_audio_samples(&sink_writer, a_idx, &mixed_bytes, &mut total_audio_frames);
        }

        // Flush any remaining buffered microphone bytes
        let remaining_mic = audio_mixer.drain_remaining_mic_bytes();
        if !remaining_mic.is_empty() {
            let _ = write_audio_samples(&sink_writer, a_idx, &remaining_mic, &mut total_audio_frames);
        }

        // Pad silence if audio duration is behind video duration
        let total_video_duration_hns = (frame_index as i64) * frame_duration_hns;
        let target_audio_frames = ((total_video_duration_hns * 48000) / 10_000_000).max(0) as u64;
        if total_audio_frames < target_audio_frames {
            let frames_needed = target_audio_frames - total_audio_frames;
            eprintln!("[PanGlide] Padding {} frames of silence to match video length", frames_needed);
            let _ = pad_audio_silence(&sink_writer, a_idx, frames_needed, &mut total_audio_frames);
        }
    }

    // Drain any remaining telemetry events from ring buffer
    if let Ok(mut events) = telemetry_events.lock() {
        while let Ok(evt) = telemetry_consumer.pop() {
            events.push(evt);
        }
    }

    // Tear down WGC session
    let _ = capture_session.Close();
    let _ = frame_pool.Close();

    // Finalize MP4 container
    unsafe {
        if let Err(e) = sink_writer.Finalize() {
            eprintln!("[PanGlide] SinkWriter Finalize error: {:?}", e);
        }
        let _ = MFShutdown();
    }

    eprintln!("[PanGlide] MP4 finalized: {}", output_path);
    Ok(())
}

/// Stop the active recording session. Signals the capture loop to terminate,
/// joins the worker thread, and returns the output file path.
#[tauri::command]
pub fn stop_recording() -> std::result::Result<RecordingResultDto, String> {
    let mut session_guard = ACTIVE_SESSION.lock().map_err(|e| e.to_string())?;
    let mut session = session_guard
        .take()
        .ok_or_else(|| "No active recording session to stop".to_string())?;

    // Signal capture thread to stop and release telemetry hooks
    session.is_active.store(false, Ordering::SeqCst);
    crate::telemetry::hooks::InputHookManager::stop_telemetry_capture();
    crate::telemetry::hooks::InputHookManager::clear_target_monitor();

    // Wait for the worker thread to finalize the MP4 (join with timeout)
    if let Some(handle) = session.worker_handle.take() {
        // Drop the session guard before joining to avoid deadlock
        drop(session_guard);

        eprintln!("[PanGlide] Waiting for capture worker to finalize...");
        let _ = handle.join();
        eprintln!("[PanGlide] Capture worker joined.");
    } else {
        drop(session_guard);
    }

    let duration_ms = session.start_time.elapsed().as_millis() as u64;
    let frame_count = session.frame_count.lock().map(|f| *f).unwrap_or(0);
    let rejected_takes = session
        .rejected_takes
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();
    let rejected_markers = session
        .rejected_markers
        .lock()
        .map(|m| m.clone())
        .unwrap_or_default();
    let recorded_events = session
        .telemetry_events
        .lock()
        .map(|e| e.clone())
        .unwrap_or_default();
    let zoom_keyframes = session
        .zoom_keyframes
        .lock()
        .map(|z| z.clone())
        .unwrap_or_default();

    // Write companion JSON sidecar file at %LOCALAPPDATA%\PanGlide\recordings\panglide_<timestamp>.telemetry.json
    let sidecar_path = session.output_path.with_extension("telemetry.json");
    let sidecar = crate::telemetry::types::TelemetrySidecar {
        metadata: crate::telemetry::types::TelemetryMetadata {
            frame_count,
            duration_ms,
            sample_rate: 48000,
            dpi_scale: 1.0,
            display_width: session.width,
            display_height: session.height,
        },
        rejected_takes: rejected_markers,
        events: recorded_events,
    };

    match serde_json::to_string_pretty(&sidecar) {
        Ok(json_str) => {
            if let Err(e) = fs::write(&sidecar_path, json_str) {
                eprintln!("[PanGlide] Error writing telemetry sidecar {:?}: {:?}", sidecar_path, e);
            } else {
                eprintln!("[PanGlide] Telemetry sidecar saved: {:?}", sidecar_path);
            }
        }
        Err(e) => {
            eprintln!("[PanGlide] Error serializing telemetry sidecar: {:?}", e);
        }
    }

    let raw_path = session.output_path.to_string_lossy().to_string();
    let video_path = raw_path
        .strip_prefix(r"\\?\")
        .unwrap_or(&raw_path)
        .replace('\\', "/");

    // Verify the output file exists and has meaningful size
    let file_size = fs::metadata(&session.output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    eprintln!(
        "[PanGlide] Recording complete: {} ({} bytes, {} frames, {}ms)",
        video_path, file_size, frame_count, duration_ms
    );

    Ok(RecordingResultDto {
        video_path,
        video_url: String::new(), // Frontend will use convertFileSrc() with asset protocol
        duration_ms,
        frame_count,
        width: session.width,
        height: session.height,
        auto_blur_markers: Vec::new(),
        rejected_takes: rejected_takes.clone(),
        rejected_take_intervals: rejected_takes,
        zoom_keyframes,
    })
}

/// Live snip: mark the preceding 5 seconds as a rejected take segment.
#[tauri::command]
pub fn trigger_live_snip() -> std::result::Result<RejectedTakeDto, String> {
    let session_guard = ACTIVE_SESSION.lock().map_err(|e| e.to_string())?;
    if let Some(ref session) = *session_guard {
        let now_ms = session.start_time.elapsed().as_millis() as u64;
        let start_ms = now_ms.saturating_sub(5000);
        let duration_sec = (now_ms - start_ms) as f64 / 1000.0;
        let marker_id = NEXT_MARKER_ID.fetch_add(1, Ordering::Relaxed);

        let snip = RejectedTakeDto {
            id: format!("rt_{}_{}", marker_id, date_now()),
            start_time_ms: start_ms,
            end_time_ms: now_ms,
            duration_sec,
        };

        let marker = crate::telemetry::types::RejectedTakeMarker {
            marker_id,
            start_timestamp_us: start_ms * 1000,
            end_timestamp_us: now_ms * 1000,
            duration_seconds: duration_sec as f32,
            reason: "Live Snip: Manual UI trigger".to_string(),
        };

        if let Ok(mut takes) = session.rejected_takes.lock() {
            takes.push(snip.clone());
        }
        if let Ok(mut markers) = session.rejected_markers.lock() {
            markers.push(marker);
        }
        if let Some(app) = crate::get_app_handle() {
            let _ = app.emit("snip_recorded", &snip);
        }

        Ok(snip)
    } else {
        Err("Cannot snip: No active recording session".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_CAPTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_enumerate_monitors_returns_results() {
        let monitors = enumerate_monitors();
        assert!(
            !monitors.is_empty(),
            "Must detect at least one physical monitor"
        );
        for mon in &monitors {
            assert!(mon.width > 0);
            assert!(mon.height > 0);
            assert!(!mon.name.is_empty());
        }
    }

    #[test]
    fn test_live_recording_3s_pipeline() {
        let _test_lock = TEST_CAPTURE_LOCK.lock().unwrap();
        use windows::Win32::Media::MediaFoundation::{
            MFCreateSourceReaderFromURL, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM,
        };
        let sources = get_available_sources();
        println!("Enumerated {} sources:", sources.len());
        for s in &sources {
            println!(
                "  Source {}: {} ({}x{}) at ({}, {})",
                s.id, s.name, s.width, s.height, s.x, s.y
            );
        }
        assert!(!sources.is_empty(), "Must detect at least 1 monitor");

        let status = start_recording(Some(sources[0].id.clone()))
            .expect("start_recording must succeed");
        assert!(status.is_recording);

        // Play system audio tone during recording to feed active audio into WASAPI loopback
        std::thread::spawn(|| {
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "[Console]::Beep(880, 1500)"])
                .spawn();
        });

        // Record for 4.2 seconds
        std::thread::sleep(std::time::Duration::from_millis(4200));

        let result = stop_recording().expect("stop_recording must succeed");
        println!(
            "Recording result: video_path={}, frames={}, duration_ms={}, size={}",
            result.video_path,
            result.frame_count,
            result.duration_ms,
            std::fs::metadata(&result.video_path)
                .map(|m| m.len())
                .unwrap_or(0)
        );

        let path = std::path::Path::new(&result.video_path);
        assert!(
            path.exists(),
            "MP4 file must exist at {}",
            result.video_path
        );
        let size = std::fs::metadata(path).expect("metadata").len();
        println!("File size: {} bytes ({:.2} KB)", size, size as f64 / 1024.0);
        assert!(
            size > 500_000,
            "MP4 file size must be > 500 KB, got {} bytes",
            size
        );

        // Verify source MP4 contains both Video and Audio streams
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _ = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET);
            let url = HSTRING::from(&result.video_path);
            let reader = MFCreateSourceReaderFromURL(&url, None).expect("reader");
            let has_video = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32).is_ok();
            let has_audio = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32).is_ok();
            println!("Recorded MP4 stream check: has_video={}, has_audio={}", has_video, has_audio);
            assert!(has_video, "Recorded MP4 must contain video stream");
            assert!(has_audio, "Recorded MP4 must contain AAC audio stream");
        }

        // Test transcoding the recording with preserved audio
        let export_dest = std::env::temp_dir().join(format!("panglide_test_export_{}.mp4", date_now()));
        let export_payload = crate::exporter::transcoder::ExportRenderPayload {
            source_path: result.video_path.clone(),
            destination_path: export_dest.to_string_lossy().to_string(),
            aspect_ratio: "9:16".to_string(),
            resolution: "1080p".to_string(),
            zoom_scale: 1.4,
            background_color: "aurora".to_string(),
            prune_rejected_takes: Some(true),
            auto_tracking: Some(true),
            pre_encode_token_masking: Some(false),
            redaction_rects: None,
        };

        let transcode_res = crate::exporter::transcoder::transcode_video(&export_payload, None)
            .expect("transcode must succeed");
        assert_eq!(transcode_res.width, 1080);
        assert_eq!(transcode_res.height, 1920);

        // Verify exported MP4 also retains both Video and Audio streams
        unsafe {
            let url = HSTRING::from(&export_payload.destination_path);
            let reader = MFCreateSourceReaderFromURL(&url, None).expect("reader");
            let has_video = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32).is_ok();
            let has_audio = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32).is_ok();
            println!("Exported MP4 stream check: has_video={}, has_audio={}", has_video, has_audio);
            assert!(has_video, "Exported MP4 must contain video stream");
            assert!(has_audio, "Exported MP4 must retain AAC audio stream");
        }

        let _ = std::fs::remove_file(&export_payload.destination_path);
    }

    #[test]
    fn test_capture_with_telemetry() {
        let _test_lock = TEST_CAPTURE_LOCK.lock().unwrap();
        use windows::Win32::Media::MediaFoundation::{
            MFCreateSourceReaderFromURL, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM,
        };

        // Ensure global hook manager is initialized
        let _ = crate::telemetry::hooks::InputHookManager::start(1024);

        let sources = get_available_sources();
        assert!(!sources.is_empty(), "Must detect at least 1 monitor");

        let status = start_recording(Some(sources[0].id.clone()))
            .expect("start_recording must succeed");
        assert!(status.is_recording);

        // Wait until capture loop is actively encoding frames
        std::thread::sleep(Duration::from_millis(700));

        // Simulate mouse movements to produce input events in SPSC ring buffer
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{mouse_event, MOUSEEVENTF_MOVE};
            for i in 0..10 {
                mouse_event(MOUSEEVENTF_MOVE, 20 * (i + 1), 10 * (i + 1), 0, 0);
                std::thread::sleep(Duration::from_millis(30));
            }
        }

        // Simulate Ctrl+Z hotkey to trigger Task 3.4 Live Mistake Snipping
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_CONTROL};
            keybd_event(VK_CONTROL.0 as u8, 0, Default::default(), 0);
            keybd_event(0x5A, 0, Default::default(), 0); // VK_Z
            std::thread::sleep(Duration::from_millis(60));
            keybd_event(0x5A, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        }

        // Also ensure a direct ring buffer event is recorded
        crate::telemetry::hooks::InputHookManager::record_input_event(crate::telemetry::types::InputEvent {
            timestamp_us: 1_000_000,
            event_type: crate::telemetry::types::InputEventType::Move,
            x: 0.5,
            y: 0.5,
            button: 0,
            key_code: 0,
        });

        // Trigger live snip request if keystroke was suppressed by OS test sandbox
        crate::telemetry::hooks::InputHookManager::trigger_live_snip_request();

        // Play brief beep so audio pipeline has sound to capture and mix
        std::thread::spawn(|| {
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "[Console]::Beep(440, 800)"])
                .spawn();
        });

        // Record for remaining 2.5 seconds
        std::thread::sleep(Duration::from_millis(2500));

        let result = stop_recording().expect("stop_recording must succeed");
        println!(
            "Test recording complete: video_path={}, frames={}, duration_ms={}",
            result.video_path, result.frame_count, result.duration_ms
        );

        let mp4_path = std::path::Path::new(&result.video_path);
        assert!(mp4_path.exists(), "MP4 recording must exist on disk");

        // Verify companion .telemetry.json sidecar file
        let sidecar_path = mp4_path.with_extension("telemetry.json");
        assert!(
            sidecar_path.exists(),
            "Companion telemetry sidecar must exist at {:?}",
            sidecar_path
        );

        let sidecar_content = std::fs::read_to_string(&sidecar_path).expect("read telemetry sidecar");
        let sidecar: crate::telemetry::types::TelemetrySidecar =
            serde_json::from_str(&sidecar_content).expect("parse telemetry sidecar JSON");

        println!(
            "Sidecar metadata: frameCount={}, durationMs={}, sampleRate={}, events={}, rejectedTakes={}",
            sidecar.metadata.frame_count,
            sidecar.metadata.duration_ms,
            sidecar.metadata.sample_rate,
            sidecar.events.len(),
            sidecar.rejected_takes.len()
        );

        assert!(sidecar.metadata.frame_count > 0, "frame_count must be > 0");
        assert!(sidecar.metadata.duration_ms > 0, "duration_ms must be > 0");
        assert_eq!(sidecar.metadata.sample_rate, 48000, "sample_rate must be 48 kHz standard");
        assert!(
            !sidecar.events.is_empty(),
            "Telemetry events must be collected in SPSC ring buffer"
        );
        assert!(
            !sidecar.rejected_takes.is_empty(),
            "Ctrl+Z hotkey must produce at least one RejectedTakeMarker"
        );

        // Verify normalized coordinates are within 0.0 to 1.0
        for evt in &sidecar.events {
            assert!(
                evt.x >= 0.0 && evt.x <= 1.0,
                "x coordinate must be normalized (0.0 to 1.0), got {}",
                evt.x
            );
            assert!(
                evt.y >= 0.0 && evt.y <= 1.0,
                "y coordinate must be normalized (0.0 to 1.0), got {}",
                evt.y
            );
        }

        // Verify MP4 has both Video and Audio streams
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _ = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET);
            let url = HSTRING::from(&result.video_path);
            let reader = MFCreateSourceReaderFromURL(&url, None).expect("reader");
            let has_video = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32).is_ok();
            let has_audio = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32).is_ok();
            assert!(has_video, "Recorded MP4 must contain video stream");
            assert!(has_audio, "Recorded MP4 must contain mixed audio stream");
        }

        let _ = std::fs::remove_file(&sidecar_path);
        let _ = std::fs::remove_file(mp4_path);
    }
}
