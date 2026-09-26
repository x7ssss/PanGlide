# Product Roadmap: PanGlide & The Creator Ecosystem

## Vision & Architecture Philosophy
PanGlide begins as a dedicated, native Windows screen recorder that solves the structural failures of existing tools: eliminating audio desync, preventing motion sickness, and protecting sensitive credentials. 

Rather than bloating PanGlide into a generic video editor, the expansion strategy follows a modular micro-utility architecture. Each companion tool performs one specific post-production job with high quality, available individually or as an integrated bundle.

---

## Milestone 1: PanGlide Core Engine (v1.0)
*Focus: Rock-solid Windows 10 (v1903+) and Windows 11 capture, kinematic auto-zoom, and native privacy protection.*

* Native Windows Graphics Capture (WGC) writing directly to Direct3D 11 textures in GPU VRAM.
* Master audio clock locking via WASAPI Loopback to guarantee true Constant Frame Rate (CFR) and eliminate desync.
* WH_MOUSE_LL and WH_KEYBOARD_LL low-level hooks with wait-free SPSC ring buffer (latency under 500ns).
* Second-order spring-damper camera physics (tension: 170, damping: 26) with cubic Bézier cursor smoothing.
* Pre-encode credential auto-redaction for API keys, passwords, and sensitive tokens.
* Live Mistake Snipping (Ctrl+Z during capture flags bad takes for automatic timeline pruning).
* Dual-format export: Clean MP4 master plus transparent alpha-channel cursor and telemetry layer for NLEs.
* Offline perpetual licensing ($49 one-time) via Lemon Squeezy and Windows DPAPI encryption.

---

## Milestone 2: Creator & Social Reframing Update (v1.1)
*Focus: Short-form video distribution for TikTok, Instagram Reels, and YouTube Shorts.*

* Cluster-Based 9:16 Vertical Reframing: Automatically detects interactive UI elements via system accessibility trees, centering vertical crops around active interaction clusters rather than jittery cursor points.
* Two-Tier Stacked Layout Presets: Instant split layout with webcam in top band, zoomed UI in middle band, and caption-safe region in bottom band.
* Multi-Track Audio Stems: Export discrete system audio and microphone channels as separate WAV files alongside the video container.
* Keystroke Glassmorphic HUD: Polished floating modifier badges for keyboard shortcuts during software walkthroughs.
* Custom Click Ripple Packs: Per-click styling controls (ripple, orb, pressure wave) with brand color customization.

---

## Milestone 3: Companion App 1 - FrameDrop (v1.2)
*Focus: Instant 3D device framing and mockup presentation.*

* Product Positioning: Turn flat screencasts into realistic MacBook, iPhone, and Studio Display launch shots in 30 seconds.
* Core Capabilities:
  * Import exports directly from PanGlide with one click.
  * 16 bundled photorealistic 3D hardware models (MacBook Pro, iPhone 16 Pro, Studio Display, generic unbranded frames).
  * GPU-accelerated lighting, contact shadows, and realistic surface reflections.
  * 5 camera movement recipes: static product card, gentle 3D tilt, subtle orbit, desk push-in, and vertical reveal.
  * Fit protection modes: zero cropping on code or UI content.
* Pricing: $29 standalone one-time license, or $19 add-on for PanGlide owners.

---

## Milestone 4: Companion App 2 - DemoType (v1.3)
*Focus: Local, UI-safe kinetic captions and interactive callouts.*

* Product Positioning: Word-by-word animated captions and feature badges that never obscure on-screen software UI.
* Core Capabilities:
  * 100% on-device local transcription using Whisper.cpp (zero cloud API fees).
  * UI Collision Avoidance: Automatically detects code blocks, sidebars, and active dialogs, routing caption placement to empty negative space.
  * Interaction-Driven Badges: Automatically turns click telemetry into visual callout pills (e.g., Click Deploy, Press Ctrl+K).
  * Export options: Burned-in video, transparent caption-only video overlay, and SRT/VTT sidecars.
* Pricing: $29 standalone one-time license.

---

## Milestone 5: The PanGlide Suite & Cross-Platform Parity (v2.0)
*Focus: Unified ecosystem and macOS / Linux expansion.*

* PanGlide macOS Port: Replaces WGC with ScreenCaptureKit and Metal while preserving the exact same Tauri v2 frontend and project format.
* PanGlide Linux Port: Implements PipeWire ScreenCast portal and Vulkan rendering.
* Unified Project Workspace: Seamless handoff between PanGlide (Record) -> FrameDrop (3D Device Frame) -> DemoType (Captions) -> LaunchCut (Multi-Format Batch Export).
* PanGlide Creator Suite Bundle: Complete four-tool bundle sold for $99 one-time perpetual license.