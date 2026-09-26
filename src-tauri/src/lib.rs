use std::sync::OnceLock;

pub mod audio;
pub mod capture;
pub mod error;
pub mod export;
pub mod exporter;
pub mod kinematics;
pub mod motion;
pub mod privacy;
pub mod shader;
pub mod telemetry;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn get_app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

pub mod commands {
    use std::os::windows::process::CommandExt;

    /// Save / copy a recorded MP4 file to a user-chosen destination cleanly.
    #[tauri::command]
    pub fn save_recording_to_destination(source_path: String, destination_path: String) -> Result<(), String> {
        let src = std::path::Path::new(&source_path);
        if !src.exists() {
            return Err(format!("Source video file does not exist at '{}'", source_path));
        }

        let dest = std::path::Path::new(&destination_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        std::fs::copy(src, dest)
            .map_err(|e| format!("Failed to copy recording from '{}' to '{}': {}", source_path, destination_path, e))?;

        eprintln!("[PanGlide] Recording successfully saved to: {}", destination_path);
        Ok(())
    }

    /// Open the native Windows Save File dialog without external plugins or COM conflicts.
    #[tauri::command]
    pub fn pick_export_destination(default_name: Option<String>) -> Result<Option<String>, String> {
        let def_name = default_name.unwrap_or_else(|| "panglide_recording.mp4".to_string());

        let script = format!(
            r#"[System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms') | Out-Null; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.Title = 'Export PanGlide Recording'; $d.Filter = 'MP4 Video (*.mp4)|*.mp4|All Files (*.*)|*.*'; $d.FileName = '{}'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.FileName }}"#,
            def_name
        );

        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| format!("Failed to open Save File dialog: {}", e))?;

        if !output.status.success() {
            return Ok(None);
        }

        let chosen_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if chosen_path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(chosen_path.replace('\\', "/")))
        }
    }

    /// Hardware-accelerated export transcode pass baking aspect ratio, zoom scale, background framing, and resolution.
    #[tauri::command]
    pub async fn export_rendered_video(
        app: tauri::AppHandle,
        payload: crate::export::transcoder::ExportRenderPayload,
    ) -> Result<crate::export::transcoder::ExportRenderResult, String> {
        let app_handle = app.clone();
        tokio::task::spawn_blocking(move || {
            crate::export::transcoder::transcode_video(
                &payload,
                Some(Box::new(move |progress, current_frame| {
                    use tauri::Emitter;
                    let _ = app_handle.emit(
                        "export-progress",
                        serde_json::json!({
                            "progress": progress,
                            "currentFrame": current_frame,
                        }),
                    );
                })),
            )
        })
        .await
        .map_err(|e| format!("Transcode task failed: {:?}", e))?
    }

    /// Scan an input string for known sensitive credentials (API keys, JWTs, Stripe, AWS, GitHub tokens)
    #[tauri::command]
    pub fn scan_text_for_secrets(text: String) -> Vec<crate::privacy::patterns::SecretMatch> {
        crate::privacy::patterns::find_sensitive_spans(&text)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_scan_text_for_secrets() {
            let res = scan_text_for_secrets("Here is my key: sk_live_abcdefghijklmnopqrstuvwxyz123456789".into());
            assert_eq!(res.len(), 1);
            assert_eq!(res[0].label, "Stripe API Key");
        }

        #[test]
        fn test_save_recording_to_destination() {
            let temp_dir = std::env::temp_dir().join(format!("panglide_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
            let _ = std::fs::create_dir_all(&temp_dir);
            let src_file = temp_dir.join("test_input.mp4");
            let dest_file = temp_dir.join("subfolder").join("test_output.mp4");

            // Write mock bytes to source
            std::fs::write(&src_file, b"MOCK_MP4_DATA").unwrap();

            // Run command
            let res = save_recording_to_destination(
                src_file.to_string_lossy().to_string(),
                dest_file.to_string_lossy().to_string(),
            );
            assert!(res.is_ok(), "Expected save_recording_to_destination to succeed");
            assert!(dest_file.exists(), "Destination file must exist");
            assert_eq!(std::fs::read(&dest_file).unwrap(), b"MOCK_MP4_DATA");

            // Clean up
            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        #[test]
        fn test_save_recording_nonexistent_source() {
            let res = save_recording_to_destination(
                "C:/nonexistent_file_panglide_123.mp4".to_string(),
                "C:/dest.mp4".to_string(),
            );
            assert!(res.is_err(), "Expected error for non-existent source file");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = APP_HANDLE.set(handle);
            // Initialize input hook manager for global hotkeys (Ctrl+Shift+R) and live snip
            let _ = crate::telemetry::hooks::InputHookManager::start(1024);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::capture::session::start_recording,
            crate::capture::session::stop_recording,
            crate::capture::session::get_recording_status,
            crate::capture::session::trigger_live_snip,
            crate::capture::session::get_available_sources,
            crate::kinematics::camera::get_solved_camera_keyframes,
            crate::commands::save_recording_to_destination,
            crate::commands::pick_export_destination,
            crate::commands::export_rendered_video,
            crate::commands::scan_text_for_secrets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PanGlide application");
}
