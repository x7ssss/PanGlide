# Engineering Tasks: DemoType

## Phase 1: Local Speech Inference Engine (Whisper)
- [ ] Task 1.1: Build Rust backend service wrapping CTranslate2 / faster-whisper with int8 CUDA bindings for Windows.
- [ ] Task 1.2: Implement audio extractor (extracts 16 kHz mono WAV stream directly from MP4/MOV containers).
- [ ] Task 1.3: Build word-level timestamp alignment parser (extracts `[start_ms, end_ms, word, probability]`).
- [ ] Task 1.4: Implement local model weight manager (downloads quantized small/medium Whisper models once on setup).

## Phase 2: Screen Region & UI Collision Analyzer
- [ ] Task 2.1: Implement frame grabber sampling at 5 FPS during audio transcription pass.
- [ ] Task 2.2: Build dirty-region and text density heatmapper using adaptive thresholding and edge detection.
- [ ] Task 2.3: Build R-Tree spatial index storing busy bounding boxes (code blocks, terminals, modal popups).
- [ ] Task 2.4: Implement dynamic placement resolver (evaluates bottom vs top vs rail safe zones per subtitle segment).

## Phase 3: Telemetry Callout & Keystroke Badge Engine
- [ ] Task 3.1: Build parser for PanGlide companion JSON telemetry sidecars (reads click events and key combinations).
- [ ] Task 3.2: Implement heuristic matcher: aligns spoken words ("open command palette") with nearby telemetry events (`Ctrl+P`).
- [ ] Task 3.3: Build kinetic overlay renderer generating animated vector badges for clicks and shortcuts.

## Phase 4: Frontend Transcript Editor & Typography Studio
- [ ] Task 4.1: Build dual-pane UI: left-hand transcript editor with word-click playhead navigation.
- [ ] Task 4.2: Build subtitle style picker: Kinetic Word Pop, Minimalist Lower Third, Terminal Inline, and Clean SaaS.
- [ ] Task 4.3: Implement manual safe-zone override tools (allows creator to draw "never cover" exclusion rectangles).
- [ ] Task 4.4: Wire real-time video preview rendering typography dynamically over the canvas.

## Phase 5: Exporter & Licensing
- [ ] Task 5.1: Build video compositor applying subtitle overlays pre-encode to output MP4 via Media Foundation.
- [ ] Task 5.2: Build transparent alpha-channel subtitle export (ProRes 4444 / WebM alpha) for NLE round-tripping.
- [ ] Task 5.3: Export standard `.srt` and `.vtt` sidecars with formatting tags.
- [ ] Task 5.4: Implement Lemon Squeezy offline DPAPI licensing verification ($29 perpetual).