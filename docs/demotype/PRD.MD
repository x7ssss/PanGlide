# Product Requirements Document (PRD): DemoType

## 1. Executive Summary & Value Proposition
DemoType is a 100% offline, privacy-first desktop transcription and UI-aware captioning tool for Windows 10 (v1903+) and Windows 11 and macOS. Built specifically for technical software demos, SaaS tutorials, and coding walkthroughs, it automatically transcribes voiceover on-device and routes subtitles to empty screen negative space so they never collide with active code editors, terminal sessions, or interactive UI controls.

* Target Audience: Developers, tech founders, software educators, and creators tired of recurring subscriptions (CapCut Pro, Descript) and privacy risks.
* Positioning: "Captions that know the screen is the subject. 100% local, zero cloud uploads."
* Pricing Model: $29 one-time perpetual license.

---

## 2. Core Functional Requirements

### 2.1 On-Device Speech Inference Engine
* Inference Runtime:
  * Windows: `faster-whisper` (CTranslate2 int8 quantization with CUDA / NVIDIA acceleration) and fallback CPU ONNX runtime.
  * macOS: `whisper.cpp` with Apple Silicon Metal acceleration.
* Performance & Latency: Real-time factor (RTF) of < 0.15 on RTX GPUs (a 3-minute video transcribes in under 15 seconds) with memory footprint under 1.5 GB VRAM.
* Data Sovereignty: 100% offline. Zero external API calls, zero server logs, zero cloud dependencies. Safe for unreleased product demos and enterprise source code.

### 2.2 Spatial UI Collision Avoidance (Smart Subtitle Placement)
* Screen Region Analysis: Samples the video frames at 5–10 FPS during ingest, identifying high-density visual regions (syntax-highlighted code editors, terminal windows, modal dialogs) versus low-density negative space (canvas margins, background wallpaper, empty toolbars).
* Placement Hierarchy: Dynamic priority scoring: Bottom-Center -> Top-Center -> Floating Rail -> Subtitle Shifting. If the bottom of the screen has an active terminal or code editor, captions route automatically to the top or side margin.
* Dynamic Safe Zone Margin: Maintains a 24px safety buffer around active bounding boxes extracted from mouse clicks and window focus events.

### 2.3 Telemetry-Driven Callout Badges (Click-to-Card)
* Hotkey & Modifier Parsing: Automatically correlates audio phrases (e.g. "press control shift P") with captured keyboard telemetry, rendering a floating glassmorphic keyboard badge (`[Ctrl + Shift + P]`) synchronized with the speech timestamp.
* Action Badges: Translates click telemetry on UI labels into kinetic visual badges (e.g., `[Click 'Deploy']`) that pop up alongside the cursor and fade out cleanly.

### 2.4 Typography & Kinetic Styling
* Technical Presets: High-contrast monospace (coding demos), clean neo-grotesque sans (SaaS demos), and kinetic word-by-word highlights.
* Syntax-Aware Formatting: Formats file paths (`/src/main.rs`), terminal commands (`cargo build`), and code keywords with distinct monospace styling directly inside the caption stream.
* Exporters: Burned-in video (MP4 NVENC), transparent video overlay (WebM / ProRes alpha), and standard subtitle sidecars (`.srt`, `.vtt`, `.json`).

---

## 3. UI/UX Specifications
* Base Surface: `#0B0D13` with `#141721` text panels and `#252B3B` borders.
* Accent Palette: Electric Indigo (`#6366F1`) for playheads, Amber Gold (`#F59E0B`) for callout tags.
* Interface: Transcript-first dual-pane layout—left side displays the word-level interactive transcript with clickable timestamps; right side displays the real-time collision-avoiding video preview.