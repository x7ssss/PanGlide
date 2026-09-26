# PanGlide 🎬

> **The ultra-lightweight (~7 MB), open-source Windows alternative to Screen Studio.**  
> Native 60 FPS desktop capture with kinematic auto-gliding, spring physics zoom, live mistake snipping, and hardware-accelerated exports.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2B%20%7C%2011-0078D6.svg)](https://microsoft.com/windows)
[![Architecture](https://img.shields.io/badge/Architecture-Rust%20%2B%20Tauri%20v2%20%2B%20React%2019-indigo.svg)](src-tauri)
[![Release](https://img.shields.io/badge/Release-v1.0.0-emerald.svg)](https://github.com/x7ssss/PanGlide/releases)

---

## ✨ Features

- **🚀 Native Direct3D 11 & Windows Graphics Capture (WGC)**  
  Zero lag, 60 FPS constant frame rate (CFR) desktop screen capture using DirectX 11 hardware surfaces.
- **🎥 Kinematic Auto-Gliding & Spring-Damper Zoom**  
  Click anywhere during a recording, and the camera smoothly punches in (1.5x) and glides directly to your click coordinates using second-order spring physics (`tension: 170.0`, `damping: 26.0`). Dwells for 2.0s before smoothly returning to 1.0x center framing.
- **✂️ Live Mistake Snipping (`Ctrl+Z`)**  
  Made a typo or stuttered during your demo? Press `Ctrl+Z` to snip away the preceding 5 seconds on the fly without restarting your take.
- **🎙️ WASAPI Audio Loopback & Microphone Mixing**  
  High-fidelity system audio capture alongside microphone input with real-time VU level monitoring.
- **⚡ Hardware-Accelerated Transcoder (MFT)**  
  Exports baked MP4 videos using Windows Media Foundation Transform (MFT) hardware encoders. Smooth camera glides, crops, and backdrop framing are baked directly into the output video.
- **📱 1-Click Multi-Aspect Exports**  
  Instantly format videos for **16:9** (YouTube/Desktop), **9:16** (TikTok, Instagram Reels, YouTube Shorts), or **1:1** (Square/LinkedIn).
- **🔒 100% Free & Open-Source**  
  No telemetry locks, no trial watermarks, no cloud dependencies. Everything runs locally and offline on your PC.

---

## ⌨️ Global Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + Shift + R` or `F9` | **Start / Stop Recording** |
| `Ctrl + Z` | **Live Snip (Cut preceding 5 seconds)** |

---

## 🛠️ Tech Stack & Architecture

- **Backend (Rust)**:
  - [Tauri v2](https://v2.tauri.app/) for the native Windows application shell.
  - Direct Win32 APIs: `windows-rs` bindings for Direct3D 11, Windows Graphics Capture, WASAPI, and Media Foundation.
  - Custom deterministic second-order spring-damper physics solver for 60 FPS camera trajectories.
- **Frontend (TypeScript)**:
  - React 19 + TypeScript + Vite.
  - Tailwind CSS + Lucide Icons.
  - High-performance `requestAnimationFrame` canvas & video styling.

---

## 📦 Building from Source

### Prerequisites

1. **Windows 10 (version 1903+) or Windows 11**
2. **Node.js** (v18.0 or newer) and `npm`
3. **Rust** (`stable-x86_64-pc-windows-msvc` toolchain)
4. **Visual Studio C++ Build Tools** (with Windows 10/11 SDK)

### Step-by-Step Instructions

1. **Clone the repository:**
   ```bash
   git clone https://github.com/x7ssss/PanGlide.git
   cd PanGlide
   ```

2. **Install frontend dependencies:**
   ```bash
   npm install
   ```

3. **Build the production frontend:**
   ```bash
   npm run build
   ```

4. **Compile the standalone native binary:**
   ```bash
   npx @tauri-apps/cli build --no-bundle
   ```

The compiled standalone executable (~7 MB) will be generated at:
```
src-tauri/target/release/panglide.exe
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) - see the [LICENSE](LICENSE) file for details.  
Copyright (c) 2026 x7ssss / PanGlide Contributors.
