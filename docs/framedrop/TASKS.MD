# Engineering Tasks: FrameDrop

## Phase 1: Native 3D wgpu Core & Window Setup
- [ ] Task 1.1: Initialize Tauri v2 workspace with a dedicated Rust 3D viewport canvas.
- [ ] Task 1.2: Set up `wgpu` rendering context with Direct3D 12 and Vulkan backends.
- [ ] Task 1.3: Build asset loader for glTF/GLB models with separate meshes for Screen Glass, Bezel, Chassis, and Buttons.
- [ ] Task 1.4: Implement PBR shader pipeline (roughness, metallic, normal mapping, and environment HDR map lighting).

## Phase 2: Zero-Copy Video Texture Pipeline
- [ ] Task 2.1: Implement hardware-accelerated video decoding using Media Foundation / Direct3D 11 texture sharing.
- [ ] Task 2.2: Bind the decoded video surface directly as a dynamic diffuse texture on the 3D screen mesh.
- [ ] Task 2.3: Build proxy preview pipeline (downscales 4K texture to 1080p during viewport camera rotation, restores 4K on export).

## Phase 3: Shadows, Shaders & Cinematography Presets
- [ ] Task 3.1: Implement soft ground contact shadows and Screen-Space Ambient Occlusion (SSAO).
- [ ] Task 3.2: Implement procedural environment reflection probe (softbox lighting, window reflection textures).
- [ ] Task 3.3: Build semantic camera engine: Quaternion interpolation for Orbit, Push-In, and Tilt paths.
- [ ] Task 3.4: Build safe-inset masking shader ensuring Dynamic Island / rounded corners never clip inner screen pixels.

## Phase 4: Frontend Studio & Hardware Catalog
- [ ] Task 4.1: Build model picker drawer: MacBook Pro 16", iPhone 16 Pro, Studio Display, Generic Unbranded Slate.
- [ ] Task 4.2: Build motion preset carousel with live animated thumbnail previews.
- [ ] Task 4.3: Implement canvas background selectors (mesh gradients, dark studio floor, transparent alpha).
- [ ] Task 4.4: Wire drag-and-drop video dropzone with auto-duration detection.

## Phase 5: Hardware Exporter & Licensing
- [ ] Task 5.1: Wire headless frame evaluation: render frame [N] to offscreen render target and pipe to NVENC hardware encoder.
- [ ] Task 5.2: Support transparent background export (ProRes 4444 / WebM with alpha).
- [ ] Task 5.3: Integrate Lemon Squeezy offline activation via Windows DPAPI credential store ($29 lifetime).