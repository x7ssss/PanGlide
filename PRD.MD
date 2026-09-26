# Product Requirements Document (PRD): PanGlide

## 1. Executive Summary & Value Proposition
PanGlide is a high-performance, local-first screen capture and auto-zoom utility designed natively for Windows 10 (v1903+) and Windows 11 (with cross-platform parity planned). It transforms raw screen recordings into production-grade product demos without cloud processing or subscription lock-in.

* Target Audience: Indie hackers, SaaS founders, software engineers, DevRel educators, and technical content creators.
* Core Philosophy: Record casually, look polished immediately, and keep private credentials secure.
* Pricing Model: $49 one-time perpetual license with 1 year of updates via Lemon Squeezy.

---

## 2. Competitive Differentiators (The PanGlide Moat)

1. True CFR Hardware Audio Locking: Eliminates audio desync in long recordings by using the WASAPI hardware clock as the absolute master timebase, padding video frames dynamically to prevent NLE drift.
2. Live Mistake Snipping: Pressing a hotkey (Ctrl+Z) during recording flags the prior 5 seconds in the ring buffer as a rejected take, automatically cutting errors before opening the editor.
3. Alpha-Channel Cursor & Telemetry Export: Exports the clean screen video as MP4 and the smoothed cursor, click ripples, and keystrokes as a separate transparent WebM / ProRes 4444 file with a JSON telemetry sidecar for NLE compositing in Premiere and DaVinci Resolve.
4. Pre-Encode Sensitive Credential Redaction: Native OCR and regex scanning for API keys (sk_live, ghp, AKIA), passwords, JWTs, and emails, pinning an amber frosted-glass shader mask over coordinates before encoding.
5. Cluster-Based 9:16 Vertical Reframing: Reframes vertical videos around active interactive UI element clusters (dialogs, forms, code blocks) using accessibility bounding boxes instead of blindly tracking jittery mouse tips.
6. Local Offline DPAPI Licensing: Unlocks instantly in under 1ms on startup via Windows DPAPI credential encryption with zero network dependency.

---

## 3. Technical Architecture & Engine Invariants

### 3.1 Display Capture Pipeline
* API: Windows Graphics Capture (WGC) via windows-rs.
* Texture Handling: Direct3D 11 surface allocations in GPU VRAM (ID3D11Texture2D). Zero CPU RAM round-trips.
* Frame Target: 60 FPS locked Constant Frame Rate (CFR) at 1080p, 1440p, and 4K UHD.
* Multi-Monitor Support: Per-Monitor V2 DPI scaling normalization to prevent coordinate displacement across mixed-DPI setups.

### 3.2 Low-Latency Telemetry & Input Interception
* OS Hooks: Win32 WH_MOUSE_LL and WH_KEYBOARD_LL hooks running on an isolated, pinned OS worker thread with an active message pump.
* Buffer Architecture: Events pushed to a wait-free Single-Producer Single-Consumer (SPSC) ring buffer (rtrb crate). Hook callback processing completes in under 500ns to prevent OS unhook timeouts.
* Telemetry Captured: Microsecond timestamp, normalized coordinates (X, Y), mouse click state, drag boundaries, and modifier key combinations.

### 3.3 Audio Synchronization Engine
* API: WASAPI Loopback for system audio and WASAPI capture for microphone input.
* Timebase: Video frame delivery is pegged directly to WASAPI audio packet presentation timestamps. Dropped frames are duplicated to maintain strict sample-rate alignment and eliminate drift on takes exceeding 60 minutes.

### 3.4 Kinematic Zoom & Motion Physics
* Camera Motion: Second-order spring-damper equations (tension: 170, damping: 26) calculate viewport transitions to eliminate whip-pan motion sickness.
* Cursor Smoothing: Raw mouse coordinate paths are filtered using cubic Bézier splines with temporal dead-zones (ignoring micro-jitters under 100px within 200ms).
* Viewport Clamping: Prevents camera over-zoom by capping magnification between 1.1x and 2.2x.

### 3.5 Privacy & Redaction Engine
* Detection: Runs on dirty-rect regions using regex pattern matching and localized OCR (Windows.Media.Ocr).
* Masking: HLSL pixel shader executes Dual Kawase frosted-glass blur with noise displacement directly on the Direct3D 11 surface.
* Safety: Non-destructive in the project file, but permanently flattened in exported video files.

### 3.6 Hardware-Accelerated Exporter
* Encoder: Windows Media Foundation (MFT) / NVENC hardware acceleration into MP4 (H.264/HEVC) and WebM.
* Presets: 16:9 Landscape (Desktop/YouTube), 9:16 Vertical (TikTok/Reels/Shorts), and 1:1 Square (Feed).
* Alpha Export: Optional multi-track render outputting raw video plus transparent interaction layer.

### 3.7 Offline Licensing & Hardware Binding
* Verification: Lemon Squeezy API (POST /v1/licenses/activate) on initial setup.
* Hardware ID: SHA-256 hash of SMBIOS UUID + CPUID + PhysicalDrive0 serial.
* Storage: License state encrypted via Windows DPAPI (CryptProtectData). Offline verification runs on every launch in under 1ms with zero external API calls.

---

## 4. UI/UX Specifications

* Color Palette:
  * Background: #0B0D13 (Deep dark slate)
  * Panels & Surfaces: #141721 with 1px border (#252B3B)
  * Primary Accent: #6366F1 (Electric Indigo)
  * Redaction / Security Accent: #F59E0B (Amber Gold)
  * Text: #F9FAFB (High-contrast white) and #94A3B8 (Muted slate)
* Views:
  1. Floating Controller: 48px height capsule overlay, pulsing red record indicator, monospace timer (00:04:12), display selector, microphone VU meter, and hotkey hints.
  2. Studio Canvas: 16:9 framed video viewport with adjustable corner radius (0-32px), drop shadow spread (0-64px), dashed focus indicator, and amber auto-redact overlays.
  3. Inspector Sidebar: Controls for zoom scale, spring/cubic easing, privacy auto-blur toggles, and canvas framing backdrops.
  4. Non-Destructive Timeline: Playhead scrubber, zoom badges, auto-blur markers, and rejected take indicators.
  5. Export Modal: Resolution selectors (1080p, 4K), aspect ratios (16:9, 9:16), alpha-channel toggles, and encoder settings.
  6. License Activation Modal: Input field for Lemon Squeezy license key with instant offline status reporting.

---

## 5. Non-Functional Requirements
* Idle RAM Consumption: Under 50 MB.
* Active Recording Overhead: Under 5% CPU usage and under 250 MB RAM on Windows 10 (v1903+) and Windows 11.
* Binary Footprint: Self-contained installer under 25 MB (Tauri v2 + Rust).
* Cold Start Latency: Under 400ms on solid-state storage.
* Data Privacy: Zero background telemetry, zero cloud tracking, and zero remote data transfers.