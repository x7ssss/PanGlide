use std::path::Path;
use windows::core::{HSTRING, Result as WinResult, Interface};
use windows::Win32::Media::MediaFoundation::{
    IMFSinkWriter, MFCreateSourceReaderFromURL, MFCreateSinkWriterFromURL,
    MFStartup, MFShutdown, MF_VERSION, MFSTARTUP_NOSOCKET,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READER_FIRST_AUDIO_STREAM,
    MF_SOURCE_READER_ANY_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM,
    MFMediaType_Video, MFMediaType_Audio, MFVideoFormat_RGB32, MFVideoFormat_H264,
    MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFCreateMediaType, MFCreateSample, MFCreateMemoryBuffer,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_MT_FRAME_SIZE, MF_MT_FRAME_RATE,
    MF_MT_PIXEL_ASPECT_RATIO, MF_MT_INTERLACE_MODE, MF_MT_AVG_BITRATE,
    MF_MT_DEFAULT_STRIDE,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MFVideoInterlace_Progressive, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_LOW_LATENCY, MF_SINK_WRITER_DISABLE_THROTTLING, MFCreateAttributes,
    IMFSample, IMF2DBuffer,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use crate::capture::d3d11::D3D11Context;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRenderPayload {
    #[serde(alias = "source_path")]
    pub source_path: String,
    #[serde(alias = "destination_path")]
    pub destination_path: String,
    #[serde(alias = "aspect_ratio")]
    pub aspect_ratio: String,
    pub resolution: String,
    #[serde(alias = "zoom_scale")]
    pub zoom_scale: f32,
    #[serde(alias = "background_color")]
    pub background_color: String,
    #[serde(default, alias = "prune_rejected_takes")]
    pub prune_rejected_takes: Option<bool>,
    #[serde(default, alias = "auto_tracking")]
    pub auto_tracking: Option<bool>,
    #[serde(default, alias = "pre_encode_token_masking")]
    pub pre_encode_token_masking: Option<bool>,
    #[serde(default, alias = "redaction_rects")]
    pub redaction_rects: Option<Vec<crate::privacy::mask::RedactionRect>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRenderResult {
    pub destination_path: String,
    pub width: u32,
    pub height: u32,
    pub frames_rendered: u64,
    pub duration_sec: f64,
}

/// Compute output width & height based on aspect ratio and resolution preset
pub fn compute_output_dimensions(aspect_ratio: &str, resolution: &str) -> (u32, u32) {
    match (aspect_ratio, resolution) {
        ("16:9", "1080p") => (1920, 1080),
        ("16:9", "1440p") => (2560, 1440),
        ("16:9", "4K") => (3840, 2160),

        ("9:16", "1080p") => (1080, 1920),
        ("9:16", "1440p") => (1440, 2560),
        ("9:16", "4K") => (2160, 3840),

        ("1:1", "1080p") => (1080, 1080),
        ("1:1", "1440p") => (1440, 1440),
        ("1:1", "4K") => (2160, 2160),

        // Fallbacks
        ("9:16", _) => (1080, 1920),
        ("1:1", _) => (1080, 1080),
        _ => (1920, 1080),
    }
}

/// Parse hex / rgb / theme string into 32-bit BGRA bytes
pub fn parse_color_to_bgra(c: &str) -> [u8; 4] {
    let s = c.trim().to_lowercase();
    if s == "aurora" || s == "indigo" {
        return [0x4B, 0x1B, 0x1E, 0xFF]; // #1E1B4B in BGRA
    }
    if s == "cyber" {
        return [0x21, 0x17, 0x14, 0xFF]; // #141721 in BGRA
    }
    if s == "slate" || s.is_empty() {
        return [0x13, 0x0D, 0x0B, 0xFF]; // #0B0D13 in BGRA
    }
    if s.starts_with('#') {
        let hex = s.trim_start_matches('#');
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return [b, g, r, 255];
            }
        } else if hex.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..1].repeat(2), 16),
                u8::from_str_radix(&hex[1..2].repeat(2), 16),
                u8::from_str_radix(&hex[2..3].repeat(2), 16),
            ) {
                return [b, g, r, 255];
            }
        }
    } else if s.starts_with("rgb") {
        let nums: Vec<u8> = s
            .replace("rgb(", "")
            .replace("rgba(", "")
            .replace(')', "")
            .split(',')
            .filter_map(|part| part.trim().parse::<u8>().ok())
            .collect();
        if nums.len() >= 3 {
            return [nums[2], nums[1], nums[0], 255];
        }
    }
    [0x13, 0x0D, 0x0B, 255]
}

/// Bitrate based on output resolution
fn bitrate_for_resolution(resolution: &str) -> u32 {
    match resolution {
        "4K" => 50_000_000,
        "1440p" => 32_000_000,
        _ => 18_000_000, // 1080p
    }
}

/// Initialize IMFSinkWriter for hardware-accelerated H.264 CFR encode and optional AAC audio
unsafe fn init_sink_writer(
    output_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    bitrate_bps: u32,
    has_audio: bool,
) -> WinResult<(IMFSinkWriter, u32, Option<u32>)> {
    let mut attr = None;
    MFCreateAttributes(&mut attr, 3)?;
    let attr = attr.expect("Failed to create attributes");
    attr.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
    attr.SetUINT32(&MF_LOW_LATENCY, 1)?;
    attr.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;

    let output_url = HSTRING::from(output_path);
    let writer = MFCreateSinkWriterFromURL(&output_url, None, Some(&attr))?;

    // Target Output Stream: H.264
    let out_type = MFCreateMediaType()?;
    out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
    out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_bps)?;
    out_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
    out_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
    out_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
    out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;

    let video_stream_index = writer.AddStream(&out_type)?;

    // Input to SinkWriter: uncompressed RGB32 (BGRA)
    let in_type = MFCreateMediaType()?;
    in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
    in_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | (height as u64))?;
    in_type.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
    in_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
    in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
    in_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;

    writer.SetInputMediaType(video_stream_index, &in_type, None)?;

    let mut audio_stream_index = None;
    if has_audio {
        let audio_out_type = MFCreateMediaType()?;
        audio_out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        audio_out_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        audio_out_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 20_000)?;

        if let Ok(a_idx) = writer.AddStream(&audio_out_type) {
            let audio_in_type = MFCreateMediaType()?;
            audio_in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            audio_in_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            audio_in_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            audio_in_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
            audio_in_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            audio_in_type.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
            audio_in_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 48000 * 4)?;

            if writer.SetInputMediaType(a_idx, &audio_in_type, None).is_ok() {
                audio_stream_index = Some(a_idx);
                eprintln!("[PanGlide Transcoder] Added target AAC audio stream #{}", a_idx);
            }
        }
    }

    writer.BeginWriting()?;

    Ok((writer, video_stream_index, audio_stream_index))
}

/// Execute full hardware-accelerated transcode pipeline
pub fn transcode_video(
    payload: &ExportRenderPayload,
    progress_callback: Option<Box<dyn Fn(u32, u64) + Send>>,
) -> Result<ExportRenderResult, String> {
    let src_path = Path::new(&payload.source_path);
    if !src_path.exists() {
        return Err(format!("Source video does not exist: {}", payload.source_path));
    }

    let dest_path = Path::new(&payload.destination_path);
    if let Some(parent) = dest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let (out_width, out_height) = compute_output_dimensions(&payload.aspect_ratio, &payload.resolution);
    let bg_color = parse_color_to_bgra(&payload.background_color);
    let fps = 60u32;
    let bitrate = bitrate_for_resolution(&payload.resolution);
    let frame_duration_hns = 10_000_000i64 / (fps as i64); // 166666 hns per frame

    eprintln!(
        "[PanGlide Transcoder] Starting transcode: {} -> {} | {}x{} | zoom={:.2}x | bg={:?}",
        payload.source_path, payload.destination_path, out_width, out_height, payload.zoom_scale, bg_color
    );

    // Initialize D3D11 context for offscreen GPU surface blitting
    let d3d = D3D11Context::new().ok();
    let d3d_render_texture = d3d.as_ref().and_then(|ctx| {
        ctx.create_render_texture(out_width, out_height, DXGI_FORMAT_B8G8R8A8_UNORM).ok()
    });

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET)
            .map_err(|e| format!("Failed to startup Media Foundation: {:?}", e))?;

        // 1. Initialize Source Reader with hardware transforms and video processing enabled
        let mut src_attr = None;
        MFCreateAttributes(&mut src_attr, 3)
            .map_err(|e| format!("Failed to create source attributes: {:?}", e))?;
        let src_attr = src_attr.unwrap();
        src_attr.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
            .map_err(|e| format!("Failed to set HW transforms: {:?}", e))?;
        src_attr.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .map_err(|e| format!("Failed to enable video processing: {:?}", e))?;
        src_attr.SetUINT32(&MF_LOW_LATENCY, 1)
            .map_err(|e| format!("Failed to set low latency: {:?}", e))?;

        let src_url = HSTRING::from(&payload.source_path);
        let reader = MFCreateSourceReaderFromURL(&src_url, Some(&src_attr))
            .map_err(|e| format!("Failed to open source video with IMFSourceReader: {:?}", e))?;

        // Configure reader output to RGB32 (32-bit BGRA)
        let rgb_type = MFCreateMediaType()
            .map_err(|e| format!("Failed to create media type: {:?}", e))?;
        rgb_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| format!("Failed to set major type: {:?}", e))?;
        rgb_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
            .map_err(|e| format!("Failed to set subtype RGB32: {:?}", e))?;

        reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &rgb_type)
            .map_err(|e| format!("Failed to set source reader media type to RGB32: {:?}", e))?;

        // Query input dimensions
        let current_type = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .map_err(|e| format!("Failed to get current media type: {:?}", e))?;
        let frame_size = current_type.GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|e| format!("Failed to get frame size: {:?}", e))?;
        let in_width = (frame_size >> 32) as u32;
        let in_height = (frame_size & 0xFFFFFFFF) as u32;

        eprintln!("[PanGlide Transcoder] Source input dimensions: {}x{}", in_width, in_height);

        // Check if source recording has an audio stream via IMFSourceReader
        let has_audio = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32).is_ok();
        eprintln!("[PanGlide Transcoder] Source video has audio: {}", has_audio);

        if has_audio {
            // Configure source reader to decode audio to 48kHz stereo 16-bit PCM
            if let Ok(audio_pcm_type) = MFCreateMediaType() {
                let _ = audio_pcm_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
                let _ = audio_pcm_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM);
                let _ = audio_pcm_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2);
                let _ = audio_pcm_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000);
                let _ = audio_pcm_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16);
                let _ = audio_pcm_type.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4);
                let _ = audio_pcm_type.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 48000 * 4);

                if let Err(e) = reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, None, &audio_pcm_type) {
                    eprintln!("[PanGlide Transcoder] Warning: could not set PCM media type on source audio stream: {:?}", e);
                }
            }
        }

        // 2. Initialize Sink Writer for target resolution and H.264 CFR (+ optional AAC audio)
        let (sink_writer, video_sink_index, audio_sink_index) = init_sink_writer(
            &payload.destination_path,
            out_width,
            out_height,
            fps,
            bitrate,
            has_audio,
        ).map_err(|e| format!("Failed to initialize IMFSinkWriter: {:?}", e))?;

        // Identify stream indices in reader
        let mut video_reader_stream = 0u32;
        let mut audio_reader_stream = None;

        for idx in 0..10 {
            if let Ok(mt) = reader.GetCurrentMediaType(idx) {
                if let Ok(major) = mt.GetGUID(&MF_MT_MAJOR_TYPE) {
                    if major == MFMediaType_Video {
                        video_reader_stream = idx;
                    } else if major == MFMediaType_Audio {
                        audio_reader_stream = Some(idx);
                    }
                }
            }
        }
        eprintln!(
            "[PanGlide Transcoder] Reader stream mapping: video_stream={}, audio_stream={:?}",
            video_reader_stream, audio_reader_stream
        );

        // 3. Load companion telemetry sidecar if available (.telemetry.json)
        let src_path = Path::new(&payload.source_path);
        let sidecar_cand1 = src_path.with_extension("telemetry.json");
        let sidecar_cand2 = {
            let mut s = payload.source_path.clone();
            s.push_str(".telemetry.json");
            std::path::PathBuf::from(s)
        };

        let sidecar: Option<crate::telemetry::types::TelemetrySidecar> = if sidecar_cand1.exists() {
            std::fs::read_to_string(&sidecar_cand1)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
        } else if sidecar_cand2.exists() {
            std::fs::read_to_string(&sidecar_cand2)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
        } else {
            None
        };

        // Determine if dynamic camera auto-tracking is enabled
        let is_auto_tracking = payload.auto_tracking.unwrap_or(true);
        let mut camera_frames: Option<Vec<crate::kinematics::CameraFrame>> = if is_auto_tracking {
            sidecar.as_ref().map(|s| crate::kinematics::generate_camera_path(s, in_width, in_height))
        } else {
            None
        };

        if camera_frames.is_none() && is_auto_tracking {
            let solved = crate::kinematics::get_solved_camera_keyframes(payload.source_path.clone());
            if !solved.is_empty() {
                camera_frames = Some(solved);
            }
        }

        if let Some(ref cf) = camera_frames {
            eprintln!(
                "[PanGlide Transcoder] Kinematic Auto-Tracking active: {} camera frames generated",
                cf.len()
            );
        }

        // Determine if rejected takes should be auto-cut
        let should_prune_takes = payload.prune_rejected_takes.unwrap_or(true);
        let rejected_takes: Vec<crate::telemetry::types::RejectedTakeMarker> = if should_prune_takes {
            sidecar.as_ref().map(|s| s.rejected_takes.clone()).unwrap_or_default()
        } else {
            Vec::new()
        };

        if !rejected_takes.is_empty() {
            eprintln!(
                "[PanGlide Transcoder] Rejected take auto-cut active: {} intervals will be excised",
                rejected_takes.len()
            );
        }

        // Determine if pre-encode frosted glass token redaction is enabled
        let is_token_masking = payload.pre_encode_token_masking.unwrap_or(true);
        let redactions: Vec<crate::privacy::mask::RedactionRect> = if is_token_masking {
            let mut list = if let Some(ref rects) = payload.redaction_rects {
                rects.clone()
            } else {
                Vec::new()
            };

            if list.is_empty() {
                let privacy_cand1 = src_path.with_extension("privacy.json");
                let privacy_cand2 = {
                    let mut s = payload.source_path.clone();
                    s.push_str(".privacy.json");
                    std::path::PathBuf::from(s)
                };
                if privacy_cand1.exists() {
                    list = std::fs::read_to_string(&privacy_cand1)
                        .ok()
                        .and_then(|c| serde_json::from_str::<Vec<crate::privacy::mask::RedactionRect>>(&c).ok())
                        .unwrap_or_default();
                } else if privacy_cand2.exists() {
                    list = std::fs::read_to_string(&privacy_cand2)
                        .ok()
                        .and_then(|c| serde_json::from_str::<Vec<crate::privacy::mask::RedactionRect>>(&c).ok())
                        .unwrap_or_default();
                }
            }

            list
        } else {
            Vec::new()
        };

        if !redactions.is_empty() {
            eprintln!(
                "[PanGlide Transcoder] Pre-Encode Privacy Masking active: {} redaction rect(s)",
                redactions.len()
            );
        }

        // Precompute scale transform and centering geometry
        let scale_x = out_width as f64 / in_width as f64;
        let scale_y = out_height as f64 / in_height as f64;
        let base_fit_scale = scale_x.min(scale_y);

        // Precompute static mapping tables
        let static_zoom = (payload.zoom_scale as f64).max(0.1);
        let static_effective_scale = base_fit_scale * static_zoom;
        let static_dst_w = (in_width as f64 * static_effective_scale).round() as i32;
        let static_dst_h = (in_height as f64 * static_effective_scale).round() as i32;
        let static_dst_x = (out_width as i32 - static_dst_w) / 2;
        let static_dst_y = (out_height as i32 - static_dst_h) / 2;

        eprintln!(
            "[PanGlide Transcoder] Base Framing: static_dst_rect=({}, {}, {}x{}), canvas={}x{}",
            static_dst_x, static_dst_y, static_dst_w, static_dst_h, out_width, out_height
        );

        let mut static_x_map: Vec<Option<usize>> = Vec::with_capacity(out_width as usize);
        for x in 0..out_width {
            let xi = x as i32;
            if xi >= static_dst_x && xi < static_dst_x + static_dst_w {
                let sx = (((xi - static_dst_x) as f64 / static_dst_w as f64) * in_width as f64) as usize;
                static_x_map.push(Some(sx.min((in_width - 1) as usize)));
            } else {
                static_x_map.push(None);
            }
        }

        let mut static_y_map: Vec<Option<usize>> = Vec::with_capacity(out_height as usize);
        for y in 0..out_height {
            let yi = y as i32;
            if yi >= static_dst_y && yi < static_dst_y + static_dst_h {
                let sy = (((yi - static_dst_y) as f64 / static_dst_h as f64) * in_height as f64) as usize;
                static_y_map.push(Some(sy.min((in_height - 1) as usize)));
            } else {
                static_y_map.push(None);
            }
        }

        // Dynamic coordinate mapping closure for Kinematic Camera frames
        let compute_dynamic_maps = |cx: f64, cy: f64, zoom: f64| -> (Vec<Option<usize>>, Vec<Option<usize>>) {
            let scale = base_fit_scale * zoom.max(0.1);
            let inv_scale = 1.0 / scale;

            let mut xm = Vec::with_capacity(out_width as usize);
            for x in 0..out_width {
                let sx = cx + (x as f64 - out_width as f64 * 0.5) * inv_scale;
                if sx >= 0.0 && sx < in_width as f64 {
                    xm.push(Some((sx as usize).min((in_width - 1) as usize)));
                } else {
                    xm.push(None);
                }
            }

            let mut ym = Vec::with_capacity(out_height as usize);
            for y in 0..out_height {
                let sy = cy + (y as f64 - out_height as f64 * 0.5) * inv_scale;
                if sy >= 0.0 && sy < in_height as f64 {
                    ym.push(Some((sy as usize).min((in_height - 1) as usize)));
                } else {
                    ym.push(None);
                }
            }

            (xm, ym)
        };

        let out_buffer_size = out_width * out_height * 4;
        let mut composited_frame: Vec<u8> = vec![0u8; out_buffer_size as usize];
        let mut frame_index: u64 = 0;
        let mut continuous_audio_time_hns: i64 = 0;

        // 4. Processing Loop: Read, Composite, Offscreen Blit, Write Sample (Video + Audio)
        loop {
            let mut actual_stream_index = 0u32;
            let mut stream_flags = 0u32;
            let mut sample_time = 0i64;
            let mut in_sample: Option<IMFSample> = None;

            reader.ReadSample(
                MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut actual_stream_index),
                Some(&mut stream_flags),
                Some(&mut sample_time),
                Some(&mut in_sample),
            ).map_err(|e| format!("Error reading sample: {:?}", e))?;

            if (stream_flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                if actual_stream_index == video_reader_stream {
                    eprintln!("[PanGlide Transcoder] End of source video stream reached at frame {}", frame_index);
                    break;
                }
                continue;
            }

            let sample_time_us = (sample_time / 10).max(0) as u64;

            // Prune samples falling in rejected take intervals (mistake auto-cutting)
            if !rejected_takes.is_empty() {
                let in_rejected = rejected_takes.iter().any(|rt| {
                    sample_time_us >= rt.start_timestamp_us && sample_time_us <= rt.end_timestamp_us
                });
                if in_rejected {
                    // Skip frame or audio packet falling in cut zone
                    continue;
                }
            }

            if let Some(sample) = in_sample {
                if actual_stream_index == video_reader_stream {
                    let media_buffer = sample.ConvertToContiguousBuffer()
                        .map_err(|e| format!("Failed to get contiguous buffer: {:?}", e))?;

                    let mut p_src: *mut u8 = std::ptr::null_mut();
                    let mut src_pitch = (in_width * 4) as usize;

                    // Try 2D buffer lock for accurate row pitch
                    let buf2d_res: windows::core::Result<IMF2DBuffer> = media_buffer.cast();
                    let is_2d_locked = if let Ok(ref b2d) = buf2d_res {
                        let mut pitch = 0i32;
                        if b2d.Lock2D(&mut p_src, &mut pitch).is_ok() {
                            src_pitch = pitch.abs() as usize;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !is_2d_locked {
                        let mut max_len = 0u32;
                        let mut cur_len = 0u32;
                        media_buffer.Lock(&mut p_src, Some(&mut max_len), Some(&mut cur_len))
                            .map_err(|e| format!("Failed to lock buffer: {:?}", e))?;
                    }

                    // Determine dynamic camera framing coordinates or fallback to static
                    let dyn_maps: (Vec<Option<usize>>, Vec<Option<usize>>);
                    let (cur_cx, cur_cy, cur_zm): (f64, f64, f64);
                    let (x_map_ref, y_map_ref): (&[Option<usize>], &[Option<usize>]) = if let Some(ref cfs) = camera_frames {
                        let in_frame_idx = ((sample_time_us as f64 * fps as f64) / 1_000_000.0).round() as usize;
                        let (cx, cy, zm) = if let Some(cf) = cfs.get(in_frame_idx).or_else(|| cfs.last()) {
                            let zoom = (cf.zoom as f64) * (payload.zoom_scale as f64 / 1.0).max(1.0);
                            (cf.center_x as f64, cf.center_y as f64, zoom)
                        } else {
                            (in_width as f64 * 0.5, in_height as f64 * 0.5, payload.zoom_scale as f64)
                        };
                        cur_cx = cx;
                        cur_cy = cy;
                        cur_zm = zm;
                        dyn_maps = compute_dynamic_maps(cx, cy, zm);
                        (&dyn_maps.0, &dyn_maps.1)
                    } else {
                        cur_cx = in_width as f64 * 0.5;
                        cur_cy = in_height as f64 * 0.5;
                        cur_zm = payload.zoom_scale as f64;
                        (&static_x_map, &static_y_map)
                    };

                    // Composite into output buffer
                    for y in 0..out_height as usize {
                        let row_dst_offset = y * out_width as usize * 4;
                        let dst_row = &mut composited_frame[row_dst_offset..row_dst_offset + out_width as usize * 4];

                        match y_map_ref[y] {
                            None => {
                                // Scanline is entirely outside destination rectangle: fill with background color
                                for chunk in dst_row.chunks_exact_mut(4) {
                                    chunk.copy_from_slice(&bg_color);
                                }
                            }
                            Some(sy) => {
                                let src_row_ptr = p_src.add(sy * src_pitch);
                                for x in 0..out_width as usize {
                                    let dst_pixel = &mut dst_row[x * 4..x * 4 + 4];
                                    match x_map_ref[x] {
                                        None => {
                                            dst_pixel.copy_from_slice(&bg_color);
                                        }
                                        Some(sx) => {
                                            let sp = src_row_ptr.add(sx * 4);
                                            dst_pixel.copy_from_slice(std::slice::from_raw_parts(sp, 4));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if is_2d_locked {
                        if let Ok(ref b2d) = buf2d_res {
                            let _ = b2d.Unlock2D();
                        }
                    } else {
                        let _ = media_buffer.Unlock();
                    }

                    // Burn pre-encode frosted glass redaction blur directly into the composited frame buffer
                    if is_token_masking && !redactions.is_empty() {
                        for rect in &redactions {
                            if let Some(quad) = crate::privacy::mask::transform_redaction_to_canvas(
                                rect,
                                cur_cx,
                                cur_cy,
                                cur_zm,
                                in_width,
                                in_height,
                                out_width,
                                out_height,
                            ) {
                                crate::privacy::mask::apply_frosted_glass_redaction(
                                    &mut composited_frame,
                                    out_width,
                                    out_height,
                                    quad,
                                );
                            }
                        }
                    }

                    // Update offscreen D3D11 surface if available
                    if let (Some(ctx), Some(tex)) = (d3d.as_ref(), d3d_render_texture.as_ref()) {
                        ctx.context.UpdateSubresource(
                            tex,
                            0,
                            None,
                            composited_frame.as_ptr() as *const _,
                            out_width * 4,
                            0,
                        );
                    }

                    // Write composited sample to SinkWriter
                    let out_sample = MFCreateSample()
                        .map_err(|e| format!("Failed to create output sample: {:?}", e))?;
                    let out_buf = MFCreateMemoryBuffer(out_buffer_size)
                        .map_err(|e| format!("Failed to create output buffer: {:?}", e))?;

                    let mut p_dst: *mut u8 = std::ptr::null_mut();
                    let mut max_len = 0u32;
                    let mut cur_len = 0u32;
                    out_buf.Lock(&mut p_dst, Some(&mut max_len), Some(&mut cur_len))
                        .map_err(|e| format!("Failed to lock output buffer: {:?}", e))?;

                    std::ptr::copy_nonoverlapping(composited_frame.as_ptr(), p_dst, out_buffer_size as usize);

                    out_buf.Unlock()
                        .map_err(|e| format!("Failed to unlock output buffer: {:?}", e))?;
                    out_buf.SetCurrentLength(out_buffer_size)
                        .map_err(|e| format!("Failed to set buffer length: {:?}", e))?;

                    out_sample.AddBuffer(&out_buf)
                        .map_err(|e| format!("Failed to add buffer to sample: {:?}", e))?;

                    // Continuous CFR timestamp
                    let timestamp_hns = (frame_index as i64) * frame_duration_hns;
                    out_sample.SetSampleTime(timestamp_hns)
                        .map_err(|e| format!("Failed to set sample time: {:?}", e))?;
                    out_sample.SetSampleDuration(frame_duration_hns)
                        .map_err(|e| format!("Failed to set sample duration: {:?}", e))?;

                    sink_writer.WriteSample(video_sink_index, &out_sample)
                        .map_err(|e| format!("Failed to write sample {}: {:?}", frame_index, e))?;

                    frame_index += 1;

                    if let Some(ref cb) = progress_callback {
                        // Update periodically
                        if frame_index % 15 == 0 {
                            cb((frame_index % 100) as u32, frame_index);
                        }
                    }
                } else if Some(actual_stream_index) == audio_reader_stream {
                    if let Some(a_sink_idx) = audio_sink_index {
                        // Adjust audio sample time to continuous CFR timeline so excised intervals don't cause audio desync
                        let dur_hns = sample.GetSampleDuration().unwrap_or(0);
                        let _ = sample.SetSampleTime(continuous_audio_time_hns);
                        continuous_audio_time_hns += dur_hns;
                        if let Err(e) = sink_writer.WriteSample(a_sink_idx, &sample) {
                            eprintln!("[PanGlide Transcoder] Warning writing audio sample: {:?}", e);
                        }
                    }
                }
            }
        }

        // 5. Finalize Sink Writer
        sink_writer.Finalize()
            .map_err(|e| format!("Failed to finalize SinkWriter: {:?}", e))?;

        let _ = MFShutdown();

        let duration_sec = frame_index as f64 / fps as f64;
        eprintln!(
            "[PanGlide Transcoder] Successfully exported {} frames ({:.2}s) to {}",
            frame_index, duration_sec, payload.destination_path
        );

        Ok(ExportRenderResult {
            destination_path: payload.destination_path.clone(),
            width: out_width,
            height: out_height,
            frames_rendered: frame_index,
            duration_sec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_output_dimensions() {
        assert_eq!(compute_output_dimensions("16:9", "1080p"), (1920, 1080));
        assert_eq!(compute_output_dimensions("16:9", "1440p"), (2560, 1440));
        assert_eq!(compute_output_dimensions("16:9", "4K"), (3840, 2160));

        assert_eq!(compute_output_dimensions("9:16", "1080p"), (1080, 1920));
        assert_eq!(compute_output_dimensions("9:16", "1440p"), (1440, 2560));
        assert_eq!(compute_output_dimensions("9:16", "4K"), (2160, 3840));

        assert_eq!(compute_output_dimensions("1:1", "1080p"), (1080, 1080));
        assert_eq!(compute_output_dimensions("1:1", "1440p"), (1440, 1440));
        assert_eq!(compute_output_dimensions("1:1", "4K"), (2160, 2160));
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color_to_bgra("#0B0D13"), [0x13, 0x0D, 0x0B, 255]);
        assert_eq!(parse_color_to_bgra("#1E1B4B"), [0x4B, 0x1B, 0x1E, 255]);
        assert_eq!(parse_color_to_bgra("aurora"), [0x4B, 0x1B, 0x1E, 255]);
        assert_eq!(parse_color_to_bgra("rgb(30, 27, 75)"), [75, 27, 30, 255]);
    }

    #[test]
    fn test_transcode_real_sample_to_9_16() {
        let recordings_dir = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PanGlide/recordings");
        let mut source_file = None;
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "mp4") {
                    source_file = Some(path);
                    break;
                }
            }
        }

        if let Some(src) = source_file {
            let out_file = std::env::temp_dir().join("test_render_9_16.mp4");
            let payload = ExportRenderPayload {
                source_path: src.to_string_lossy().to_string(),
                destination_path: out_file.to_string_lossy().to_string(),
                aspect_ratio: "9:16".to_string(),
                resolution: "1080p".to_string(),
                zoom_scale: 1.4,
                background_color: "#1E1B4B".to_string(),
                prune_rejected_takes: Some(true),
                auto_tracking: Some(true),
                pre_encode_token_masking: Some(false),
                redaction_rects: None,
            };

            let res = transcode_video(&payload, None);
            eprintln!("Transcode result: {:?}", res);
            assert!(res.is_ok(), "Transcode failed: {:?}", res);

            let res = res.unwrap();
            assert_eq!(res.width, 1080);
            assert_eq!(res.height, 1920);
            assert!(out_file.exists());
            assert!(std::fs::metadata(&out_file).unwrap().len() > 1000);
            eprintln!("Successfully created 9:16 video of size: {} bytes", std::fs::metadata(&out_file).unwrap().len());
        }
    }

    #[test]
    fn test_dynamic_camera_export_with_sidecar() {
        let recordings_dir = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PanGlide/recordings");
        let mut source_file = None;
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "mp4") {
                    source_file = Some(path);
                    break;
                }
            }
        }

        if let Some(src) = source_file {
            // Create a temporary companion telemetry sidecar with click event and 5s rejected take
            let sidecar_path = src.with_extension("telemetry.json");
            let had_prior_sidecar = sidecar_path.exists();
            let prior_content = if had_prior_sidecar {
                std::fs::read_to_string(&sidecar_path).ok()
            } else {
                None
            };

            let test_sidecar = crate::telemetry::types::TelemetrySidecar {
                metadata: crate::telemetry::types::TelemetryMetadata {
                    frame_count: 120,
                    duration_ms: 2000,
                    sample_rate: 48000,
                    dpi_scale: 1.0,
                    display_width: 1920,
                    display_height: 1080,
                },
                rejected_takes: vec![
                    crate::telemetry::types::RejectedTakeMarker {
                        marker_id: 1,
                        start_timestamp_us: 500_000,
                        end_timestamp_us: 1_000_000,
                        duration_seconds: 0.5,
                        reason: "Unit test auto-cut take".to_string(),
                    }
                ],
                events: vec![
                    crate::telemetry::types::InputEvent {
                        timestamp_us: 100_000,
                        timestamp_ms: 100,
                        event_type: crate::telemetry::types::InputEventType::MouseDown,
                        x: 0.7,
                        y: 0.6,
                        button: 1,
                        key_code: 0,
                    }
                ],
            };

            let _ = std::fs::write(&sidecar_path, serde_json::to_string_pretty(&test_sidecar).unwrap());

            let out_file = std::env::temp_dir().join("test_dynamic_autotrack.mp4");
            let payload = ExportRenderPayload {
                source_path: src.to_string_lossy().to_string(),
                destination_path: out_file.to_string_lossy().to_string(),
                aspect_ratio: "16:9".to_string(),
                resolution: "1080p".to_string(),
                zoom_scale: 1.0,
                background_color: "#0B0D13".to_string(),
                prune_rejected_takes: Some(true),
                auto_tracking: Some(true),
                pre_encode_token_masking: Some(false),
                redaction_rects: None,
            };

            let res = transcode_video(&payload, None);
            eprintln!("Dynamic transcode result: {:?}", res);
            assert!(res.is_ok(), "Dynamic transcode failed: {:?}", res);

            let res = res.unwrap();
            assert_eq!(res.width, 1920);
            assert_eq!(res.height, 1080);
            assert!(out_file.exists());
            assert!(std::fs::metadata(&out_file).unwrap().len() > 1000);

            // Cleanup / restore original sidecar
            if let Some(content) = prior_content {
                let _ = std::fs::write(&sidecar_path, content);
            }
        }
    }

    #[test]
    fn test_pre_encode_token_masking_export() {
        let recordings_dir = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PanGlide/recordings");
        let mut source_file = None;
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "mp4") {
                    source_file = Some(path);
                    break;
                }
            }
        }

            if let Some(src) = source_file {
                let out_file = std::env::temp_dir().join("test_pre_encode_redaction.mp4");
                let redaction_rects = vec![
                    crate::privacy::mask::RedactionRect {
                        x: 100.0,
                        y: 100.0,
                        width: 300.0,
                        height: 50.0,
                        label: "Stripe Secret Key".to_string(),
                        time_ms: 0,
                    },
                    crate::privacy::mask::RedactionRect {
                        x: 0.5,
                        y: 0.5,
                        width: 0.2,
                        height: 0.05,
                        label: "Normalized GitHub Token".to_string(),
                        time_ms: 0,
                    }
                ];

                let payload = ExportRenderPayload {
                    source_path: src.to_string_lossy().to_string(),
                    destination_path: out_file.to_string_lossy().to_string(),
                    aspect_ratio: "16:9".to_string(),
                    resolution: "1080p".to_string(),
                    zoom_scale: 1.0,
                    background_color: "#0B0D13".to_string(),
                    prune_rejected_takes: Some(false),
                    auto_tracking: Some(false),
                    pre_encode_token_masking: Some(true),
                    redaction_rects: Some(redaction_rects),
                };

                let res = transcode_video(&payload, None);
                eprintln!("Pre-encode redaction transcode result: {:?}", res);
                assert!(res.is_ok(), "Redaction transcode failed: {:?}", res);

                let res = res.unwrap();
                assert_eq!(res.width, 1920);
                assert_eq!(res.height, 1080);
                assert!(out_file.exists());
            }
        }
    }
