# Product Requirements Document (PRD): FrameDrop

## 1. Executive Summary & Value Proposition
FrameDrop is a native, local-first 3D device framing and cinematic motion tool for Windows 10 (v1903+) and Windows 11 (with macOS parity). It allows creators, founders, and motion designers to wrap raw screen recordings or product walkthroughs into photorealistic 3D hardware scenes (MacBook Pro, iPhone 16 Pro, Studio Display, unbranded hardware) with dynamic lighting, contact shadows, and physically plausible camera movement—without touching Blender, After Effects, or cloud rendering engines.

* Target Audience: SaaS founders, indie hackers, developer advocates, and video creators on Windows who are locked out of Mac-only tools like Rotato.
* Positioning: "One minute from flat screen recording to studio-grade 3D product launch shot."
* Pricing Model: $29 one-time perpetual license (no subscriptions, no cloud render credits).

---

## 2. Core Functional Requirements

### 2.1 Native Hardware-Accelerated 3D Engine
* Engine Backend: Native wgpu / Direct3D 12 rendering pipeline in Rust (via Tauri v2). Zero Electron/WebGL browser bottleneck.
* Zero-Copy Video Texture Streaming: Direct GPU video decoding (NVDEC on Windows via Direct3D surface sharing). Bypasses CPU RAM copies (no `decode -> JS canvas -> WebGL upload` bottleneck). Decodes 4K 60fps video directly into a `wgpu::Texture`.
* Shading & Materials: Physically Based Rendering (PBR) metallic/roughness shaders with realistic glass absorption, bezel beveling, subtle chassis micro-scratches, and realistic screen specular highlights.
* Real-Time Contact Shadows: Screen-Space Ambient Occlusion (SSAO) and soft contact shadows anchoring the hardware realistically to ground surfaces (preventing the "floating sticker" effect).

### 2.2 The "Never-Crop" UI Protection System
* Safe Inset Boundaries: Hardware masks automatically account for Dynamic Islands, camera notches, and rounded glass radii, shifting the video canvas or applying safe letterbox margins so code lines, tab bars, and bottom action buttons are never clipped.
* Multi-Device Stack Compositions: One-click scene presets combining multi-device workflows (e.g. MacBook Pro in the center with companion iPhone floating in the bottom-right foreground with independent parallax depth).
* Dynamic Aspect Ratio Mapping: Exact parametric display matrices for standard aspect ratios (16:10 MacBook, 19.5:9 iPhone, 16:9 Desktop Monitor) preventing pixel stretching or edge clipping.

### 2.3 Semantic Cinematography Camera Presets
* No Raw Keyframing: Replaces complex Euler angle timelines with semantic motion recipes:
  1. Subtle Orbit: Slow 15-degree horizontal glide around the screen center.
  2. Desk Push-In: Starts at low-angle surface level and rises smoothly into a full-frontal screen focus.
  3. Parallax Float: Subtle, breathing camera motion mimicking professional product launch commercials.
  4. 3D Tilt Reveal: High-angle vertical pan sweeping over hardware edges into the active UI area.
* Deterministic Timing: Spring-damped easing curves that automatically stretch or compress camera paths to match the duration of the dropped video file without breaking motion curves.

### 2.4 Local Hardware Exporter
* Formats: 4K 60fps MP4 (H.264 / HEVC via NVENC/Media Foundation), transparent WebM (VP9 with alpha), and transparent ProRes 4444 MOV.
* Background Execution: Non-blocking render queue allowing full UI preview manipulation while rendering runs on a low-priority GPU context.

---

## 3. UI/UX Specifications
* Base Surface: `#0B0D13` with `#141721` inspector sidebars and `#252B3B` borders.
* Primary Accent: Electric Indigo (`#6366F1`) with Emerald (`#10B981`) render progress indicators.
* Controls: Single-slider controls for "Camera Distance," "Orbit Intensity," "Surface Roughness," and "Shadow Softness."