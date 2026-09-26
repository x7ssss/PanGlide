use std::path::{Path, PathBuf};
use panglide_lib::export::transcoder::{transcode_video, ExportRenderPayload};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!(" PanGlide Hardware Transcoder Test Verification (9:16 @ 1.4x)");
    println!("============================================================");

    // 1. Locate existing recording
    let local_app_data = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set");
    let recordings_dir = PathBuf::from(local_app_data).join("PanGlide").join("recordings");
    println!("Looking for recordings in: {}", recordings_dir.display());

    let mut source_file: Option<PathBuf> = None;
    if recordings_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(&recordings_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "mp4"))
            .collect();
        // Sort newest first
        entries.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        entries.reverse();

        for p in entries {
            if let Ok(meta) = std::fs::metadata(&p) {
                if meta.len() > 100_000 {
                    source_file = Some(p);
                    break;
                }
            }
        }
    }

    let source = source_file.expect("No recording file found to test export!");
    println!("Selected Source Video: {}", source.display());
    println!("Source File Size:      {} bytes", std::fs::metadata(&source)?.len());

    let dest_file = Path::new("test_render_9_16.mp4").canonicalize().unwrap_or_else(|_| PathBuf::from("test_render_9_16.mp4"));
    if dest_file.exists() {
        let _ = std::fs::remove_file(&dest_file);
    }

    let payload = ExportRenderPayload {
        source_path: source.to_string_lossy().to_string(),
        destination_path: dest_file.to_string_lossy().to_string(),
        aspect_ratio: "9:16".to_string(),
        resolution: "1080p".to_string(),
        zoom_scale: 1.4,
        background_color: "#1E1B4B".to_string(),
        prune_rejected_takes: Some(true),
        auto_tracking: Some(true),
        pre_encode_token_masking: Some(true),
        redaction_rects: None,
    };

    println!("\nExecuting real hardware-accelerated transcode pass:");
    println!("  Target Canvas:      9:16 Vertical (1080x1920)");
    println!("  Hardware Zoom:      1.40x Magnification");
    println!("  Framing Background: #1E1B4B (Deep Aurora)");
    println!("  Output Destination: {}", payload.destination_path);

    let start = std::time::Instant::now();
    let result = transcode_video(&payload, Some(Box::new(|pct, frame| {
        println!("  [Transcode Progress] {:3}% (Frame {})", pct, frame);
    })))?;

    let elapsed = start.elapsed();
    println!("\nTranscode Completed in {:.2}s!", elapsed.as_secs_f64());
    println!("  Output Path:     {}", result.destination_path);
    println!("  Output Width:    {} px", result.width);
    println!("  Output Height:   {} px", result.height);
    println!("  Frames Rendered: {}", result.frames_rendered);
    println!("  Duration:        {:.2}s", result.duration_sec);

    let meta = std::fs::metadata(&dest_file)?;
    println!("  Output File Size: {} bytes ({:.2} MB)", meta.len(), meta.len() as f64 / 1_048_576.0);

    assert_eq!(result.width, 1080, "Output width must be strictly 1080");
    assert_eq!(result.height, 1920, "Output height must be strictly 1920");
    assert!(meta.len() > 100_000, "Output file must have valid video data");

    println!("\n============================================================");
    println!(" VERIFICATION SUCCESSFUL: 9:16 @ 1.4x zoom rendered to MP4!");
    println!("============================================================");

    Ok(())
}
