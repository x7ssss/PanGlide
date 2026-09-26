# Engineering Tasks & Implementation Status: PanGlide

## Phase 1: Workspace Scaffolding & Native Toolchain Setup
- [x] Task 1.1: Initialize Tauri v2 project structure with React, TypeScript, and Tailwind CSS.
- [x] Task 1.2: Configure Cargo.toml with strict dependencies: windows, windows-version, rtrb, tauri, reqwest, sha2, serde.
- [x] Task 1.3: Set up MSVC build profiles with strip = true, lto = "fat", and codegen-units = 1.
- [x] Task 1.4: Establish global theme tokens (#0B0D13, #141721, #252B3B, #6366F1, #F59E0B) in Tailwind config.

## Phase 2: Native Windows Capture & Audio Engine
- [x] Task 2.1: Implement Windows.Graphics.Capture session via windows-rs targeting real display handles.
- [x] Task 2.2: Bind frame delivery to Direct3D 11 texture surfaces with ID3D11Multithread thread-safe protection.
- [ ] Task 2.3: Build WASAPI loopback audio stream recorder on a high-priority background thread.
- [ ] Task 2.4: Build WASAPI microphone capture stream with automatic sample rate conversion to 48 kHz.
- [ ] Task 2.5: Implement CFR frame multiplexer: lock video frame output to audio hardware master clock.

## Phase 3: Input Hook, SPSC Ring Buffer & Live Mistake Snipping
- [ ] Task 3.1: Implement low-level mouse hook (WH_MOUSE_LL) on a dedicated OS thread with an active GetMessageW loop.
- [ ] Task 3.2: Implement low-level keyboard hook (WH_KEYBOARD_LL) to track hotkeys and modifier keystrokes.
- [ ] Task 3.3: Wire hook callbacks to write raw coordinate and click structs into an atomic SPSC ring buffer (rtrb).
- [ ] Task 3.4: Implement Live Snip logic: detecting Ctrl+Z writes a rejected_take segment marker.
- [ ] Task 3.5: Build multi-monitor Per-Monitor V2 DPI coordinate normalizer.

## Phase 4: Kinematic Motion & Shader Pipeline
- [ ] Task 4.1: Implement second-order spring-damper equations in Rust (tension: 170, damping: 26) for camera viewport gliding.
- [ ] Task 4.2: Implement cubic Bézier spline interpolation and Kalman filtering over raw cursor coordinates.
- [ ] Task 4.3: Implement velocity thresholds and spatial dead-zones (ignoring cursor noise under 100px within 200ms).
- [ ] Task 4.4: Write Direct3D 11 HLSL pixel shader for Dual Kawase frosted-glass blur with noise displacement.

## Phase 5: Privacy Engine & Pattern Recognition
- [ ] Task 5.1: Build token pattern regex library for API keys (Stripe sk_live, GitHub ghp, AWS AKIA), passwords, JWTs, and email strings.
- [ ] Task 5.2: Integrate Windows.Media.Ocr on dirty-rect regions to extract on-screen text coordinates.
- [ ] Task 5.3: Bind detected text bounding rectangles to camera coordinate transforms.
- [ ] Task 5.4: Apply shader blur masks pre-encode to guarantee permanent redaction on exported files.

## Phase 6: Frontend UI & Studio Canvas
- [x] Task 6.1: Build FloatingController.tsx: capsule overlay with dynamic monitor dropdown and timer.
- [x] Task 6.2: Build StudioCanvas.tsx: 16:9 canvas with Tauri v2 asset protocol video playback.
- [ ] Task 6.3: Render dashed focus reticle and amber frosted auto-redact overlays over real video coordinates.
- [x] Task 6.4: Build InspectorSidebar.tsx: sliders for Zoom Scale, easing selection, and privacy toggles.
- [ ] Task 6.5: Build Timeline.tsx: interactive scrubber with real telemetry zoom badges and auto-blur markers.
- [x] Task 6.6: Build ExportModal.tsx: resolution toggles and aspect ratio presets.
- [x] Task 6.7: Build LicenseModal.tsx: key input dialog with offline status reporting.

## Phase 7: Hardware Exporter & NLE Dual-Track Render
- [x] Task 7.1: Build Windows Media Foundation (MFT) hardware encoding pipeline for MP4 (H.264 CFR 60 FPS).
- [ ] Task 7.2: Build transparent WebM / ProRes 4444 alpha-channel exporter for cursor, clicks, and keystrokes.
- [ ] Task 7.3: Generate companion JSON telemetry sidecar file containing frame-by-frame cursor paths and zoom keyframes.
- [ ] Task 7.4: Implement cluster-based 9:16 vertical crop algorithm using accessibility bounding boxes.

## Phase 8: Offline Licensing & Binary Protection
- [ ] Task 8.1: Build hardware fingerprint module: SHA-256 hash of SMBIOS UUID, CPUID, and PhysicalDrive0 serial.
- [ ] Task 8.2: Build Lemon Squeezy activation client (POST /v1/licenses/activate with instance_name = HWID).
- [ ] Task 8.3: Implement Windows DPAPI local vault (CryptProtectData / CryptUnprotectData) for offline storage.
- [ ] Task 8.4: Wire application startup guard: verify DPAPI vault against current hardware in under 1ms.
- [x] Task 8.5: Verify release binary stripping and run end-to-end capture and export validation tests.