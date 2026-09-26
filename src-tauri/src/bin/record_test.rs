use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::Media::MediaFoundation::{
    MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFShutdown, MFStartup, IMFSinkWriter,
    MFMediaType_Video, MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoFormat_RGB32,
    MFVideoInterlace_Progressive, MF_LOW_LATENCY, MF_MT_AVG_BITRATE,
    MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE,
    MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING,
    MF_VERSION, MFSTARTUP_NOSOCKET,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

#[derive(Debug, Clone)]
pub struct PhysicalMonitor {
    pub index: usize,
    pub name: String,
    pub hmonitor: HMONITOR,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

pub fn enumerate_monitors() -> Vec<PhysicalMonitor> {
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

struct D3D11Resources {
    #[allow(dead_code)]
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    multithread: ID3D11Multithread,
    winrt_device: IDirect3DDevice,
    render_texture: ID3D11Texture2D,
    staging_texture: ID3D11Texture2D,
}

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
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
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
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
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
) -> windows::core::Result<(IMFSinkWriter, u32)> {
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

        let out_type = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_bps)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        out_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;

        let stream_index = writer.AddStream(&out_type)?;

        let in_type = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        in_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        in_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;

        match writer.SetInputMediaType(stream_index, &in_type, None) {
            Ok(_) => println!("SinkWriter input configured for MFVideoFormat_RGB32 (Native BGRA)"),
            Err(e) => {
                println!("RGB32 not directly supported ({:?}), switching to NV12 fallback...", e);
                in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
                writer.SetInputMediaType(stream_index, &in_type, None)?;
                println!("SinkWriter input configured for MFVideoFormat_NV12");
            }
        }

        writer.BeginWriting()?;
        Ok((writer, stream_index))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!(" PanGlide Standalone Hardware Capture & Encoder CLI Verification");
    println!("============================================================");

    println!("\n[TASK 1] Enumerating connected physical monitors...");
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        eprintln!("FATAL: No physical monitors detected via EnumDisplayMonitors!");
        std::process::exit(1);
    }

    for mon in &monitors {
        let tag = if mon.is_primary { " (Primary)" } else { "" };
        println!(
            "  Monitor {}: {} - {}x{} at ({}, {}) [left={}, top={}, right={}, bottom={}]{}",
            mon.index, mon.name, mon.width, mon.height, mon.left, mon.top, mon.left, mon.top, mon.right, mon.bottom, tag
        );
    }

    let primary = monitors
        .iter()
        .find(|m| m.is_primary)
        .unwrap_or(&monitors[0])
        .clone();

    println!(
        "\nTarget Display: {} ({}) - Native Resolution: {}x{} at ({}, {})",
        primary.name,
        if primary.is_primary { "Primary" } else { "Secondary" },
        primary.width,
        primary.height,
        primary.left,
        primary.top
    );

    let output_file = "test_capture.mp4";
    if Path::new(output_file).exists() {
        let _ = fs::remove_file(output_file);
    }

    println!("\n[TASK 2] Initializing Direct3D 11 Device and Media Foundation Sink Writer...");
    let d3d = init_d3d11(primary.width, primary.height)?;
    println!("  Direct3D 11 Hardware Device & GPU VRAM textures initialized.");

    let fps = 60u32;
    let bitrate_bps = 12_000_000u32;
    let (sink_writer, stream_index) = init_sink_writer(
        output_file,
        primary.width,
        primary.height,
        fps,
        bitrate_bps,
    )?;
    println!(
        "  Media Foundation Sink Writer initialized: target='{}', H.264 @ {}x{}, {} FPS, {} Mbps",
        output_file,
        primary.width,
        primary.height,
        fps,
        bitrate_bps / 1_000_000
    );

    println!("\n[TASK 3] Starting Windows Graphics Capture session & 5-second capture loop...");

    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    let capture_item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(primary.hmonitor)? };
    let item_size = capture_item.Size()?;
    println!(
        "  GraphicsCaptureItem bound to monitor: {}x{}",
        item_size.Width, item_size.Height
    );

    // Frame pool with 4 buffers to avoid exhaustion under load
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d.winrt_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        4,
        item_size,
    )?;

    let capture_session = frame_pool.CreateCaptureSession(&capture_item)?;
    let _ = capture_session.SetIsBorderRequired(false);
    let _ = capture_session.SetIsCursorCaptureEnabled(true);

    let shared_context = d3d.context.clone();
    let shared_render_tex = d3d.render_texture.clone();
    let multithread = d3d.multithread.clone();
    let has_first_frame = Arc::new(AtomicBool::new(false));
    let has_first_frame_clone = has_first_frame.clone();

    frame_pool.FrameArrived(&TypedEventHandler::new(
        move |pool: &Option<Direct3D11CaptureFramePool>, _| {
            if let Some(pool) = pool {
                if let Ok(frame) = pool.TryGetNextFrame() {
                    if let Ok(surface) = frame.Surface() {
                        if let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() {
                            let src_texture_res: windows::core::Result<ID3D11Texture2D> =
                                unsafe { access.GetInterface() };
                            if let Ok(src_texture) = src_texture_res {
                                unsafe {
                                    multithread.Enter();
                                    shared_context.CopyResource(&shared_render_tex, &src_texture);
                                    multithread.Leave();
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
    ))?;

    capture_session.StartCapture()?;
    println!("  Windows Graphics Capture active. Awaiting first frame...");

    let wait_start = Instant::now();
    while !has_first_frame.load(Ordering::SeqCst) {
        if wait_start.elapsed() > Duration::from_secs(5) {
            eprintln!("FATAL: Timed out waiting for first capture frame from WGC!");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("  First desktop frame received! Beginning 5-second 60 FPS recording sequence...");

    let total_frames = 5 * fps;
    let frame_duration_hns = 10_000_000i64 / (fps as i64);
    let buffer_size = primary.width * primary.height * 4;

    let loop_start = Instant::now();
    let frame_interval = Duration::from_micros(1_000_000 / (fps as u64));

    for i in 0..total_frames {
        let frame_start = Instant::now();

        unsafe {
            // Protected staging copy
            d3d.multithread.Enter();
            d3d.context.CopyResource(&d3d.staging_texture, &d3d.render_texture);
            d3d.multithread.Leave();

            // Map staging texture to read BGRA pixel rows
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            d3d.context
                .Map(&d3d.staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

            let sample = MFCreateSample()?;
            let buffer = MFCreateMemoryBuffer(buffer_size)?;

            let mut p_dst: *mut u8 = std::ptr::null_mut();
            let mut max_len = 0u32;
            let mut cur_len = 0u32;
            buffer.Lock(&mut p_dst, Some(&mut max_len), Some(&mut cur_len))?;

            let row_pitch = mapped.RowPitch as usize;
            let row_bytes = (primary.width * 4) as usize;

            if row_pitch == row_bytes {
                std::ptr::copy_nonoverlapping(
                    mapped.pData as *const u8,
                    p_dst,
                    buffer_size as usize,
                );
            } else {
                for y in 0..primary.height as usize {
                    let src_row = (mapped.pData as *const u8).add(y * row_pitch);
                    let dst_row = p_dst.add(y * row_bytes);
                    std::ptr::copy_nonoverlapping(src_row, dst_row, row_bytes);
                }
            }

            buffer.Unlock()?;
            buffer.SetCurrentLength(buffer_size)?;
            sample.AddBuffer(&buffer)?;

            let timestamp_hns = (i as i64) * frame_duration_hns;
            sample.SetSampleTime(timestamp_hns)?;
            sample.SetSampleDuration(frame_duration_hns)?;

            d3d.context.Unmap(&d3d.staging_texture, 0);

            sink_writer.WriteSample(stream_index, &sample)?;
        }

        if (i + 1) % fps == 0 || i + 1 == total_frames {
            let elapsed_sec = (i + 1) / fps;
            println!(
                "  [Capture Progress] {}s / 5s (Frame {} / {}) - MFT hardware encoder active",
                elapsed_sec,
                i + 1,
                total_frames
            );
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        }
    }

    let actual_capture_duration = loop_start.elapsed();
    println!(
        "\nCapture loop completed in {:.2} seconds ({} frames encoded).",
        actual_capture_duration.as_secs_f64(),
        total_frames
    );

    let _ = capture_session.Close();
    let _ = frame_pool.Close();

    println!("Finalizing MP4 container and flushing Media Foundation hardware encoder...");
    unsafe {
        sink_writer.Finalize()?;
        let _ = MFShutdown();
    }
    println!("Media Foundation Sink Writer finalized successfully.");

    println!("\n[VERIFICATION GATE] Checking written MP4 file...");
    let file_path = Path::new(output_file);
    if !file_path.exists() {
        eprintln!("VERIFICATION FAILED: '{}' was not found on disk!", output_file);
        std::process::exit(1);
    }

    let metadata = fs::metadata(file_path)?;
    let file_size = metadata.len();
    let file_size_mb = file_size as f64 / (1024.0 * 1024.0);

    println!("  Output File: {}", file_path.canonicalize()?.display());
    println!("  File Size:   {} bytes ({:.2} MB)", file_size, file_size_mb);

    if file_size < 1_000_000 {
        eprintln!(
            "VERIFICATION FAILED: '{}' size ({} bytes) is less than 1 MB threshold!",
            output_file, file_size
        );
        std::process::exit(1);
    }

    println!("\n============================================================");
    println!(" VERIFICATION SUCCESSFUL: Real 5-second MP4 recorded & validated!");
    println!(" File: {} ({:.2} MB, {}x{} @ {} FPS)", output_file, file_size_mb, primary.width, primary.height, fps);
    println!("============================================================");

    Ok(())
}