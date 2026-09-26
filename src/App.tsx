import { useState, useEffect, useRef } from "react";
import {
  Download,
  Video,
  CheckCircle2,
  X,
} from "lucide-react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import FloatingController from "./components/FloatingController";
import StudioCanvas from "./components/StudioCanvas";
import InspectorSidebar from "./components/InspectorSidebar";
import Timeline from "./components/Timeline";
import ExportModal, { ExportConfig } from "./components/ExportModal";
import type {
  AspectRatioPreset,
  AutoBlurMarker,
  CameraFrame,
  RecordingState,
  RecordingResult,
  RejectedTakeSegment,
  ZoomKeyframe,
} from "./types";

export default function App() {
  // App state
  const [aspectRatio, setAspectRatio] = useState<AspectRatioPreset>("16:9");
  const [zoomScale, setZoomScale] = useState(1.0);
  const [cornerRadius, setCornerRadius] = useState(16);
  const [dropShadowSpread, setDropShadowSpread] = useState(36);
  const [backdropId, setBackdropId] = useState("aurora");
  const [showFocusReticle, setShowFocusReticle] = useState(true);
  const [autoRedactEnabled] = useState(true);
  const [springPreset, setSpringPreset] = useState("snappy");
  const [deadzoneEnabled, setDeadzoneEnabled] = useState(true);
  const [pruneRejectedTakes, setPruneRejectedTakes] = useState(true);
  const [autoTrackingEnabled, setAutoTrackingEnabled] = useState(true);

  // Modals state
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const [exportToast, setExportToast] = useState<{
    filePath: string;
    resolution: string;
  } | null>(null);

  // Recording State (Wired to Rust Backend)
  const [recordingState, setRecordingState] = useState<RecordingState>({
    isRecording: false,
    isPaused: false,
    elapsedMs: 0,
    micVuLevel: 0.0,
    sysVuLevel: 0.0,
    activeSource: "None",
  });

  // Selected source ID from monitor dropdown
  const selectedSourceRef = useRef<string>("");

  // Real Captured Video (via asset protocol, no base64)
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [rawVideoPath, setRawVideoPath] = useState<string | null>(null);
  const [solvedKeyframes, setSolvedKeyframes] = useState<CameraFrame[]>([]);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);

  // Dynamic Telemetry State (Empty by default - NO FAKE DATA)
  const [durationMs, setDurationMs] = useState(0);
  const [currentPlayheadMs, setCurrentPlayheadMs] = useState(0);
  const [zoomKeyframes, setZoomKeyframes] = useState<ZoomKeyframe[]>([]);
  const [autoBlurMarkers, setAutoBlurMarkers] = useState<AutoBlurMarker[]>([]);
  const [rejectedTakes, setRejectedTakes] = useState<RejectedTakeSegment[]>([]);

  // Camera tracking center
  const [cameraCenter, setCameraCenter] = useState({ x: 0.5, y: 0.5 });
  const [telemetryPoint, setTelemetryPoint] = useState<{ x: number; y: number } | null>(null);

  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // 2. Active Recording Poller: Polls Rust backend for real elapsed time and frame count
  useEffect(() => {
    if (recordingState.isRecording) {
      pollIntervalRef.current = setInterval(async () => {
        try {
          const status = await invoke<{
            isRecording: boolean;
            elapsedMs: number;
            frameCount: number;
            micVuLevel: number;
            sysVuLevel: number;
            activeSource: string;
            latestCameraX: number;
            latestCameraY: number;
          }>("get_recording_status");

          setRecordingState((prev) => ({
            ...prev,
            elapsedMs: status.elapsedMs,
            micVuLevel: status.micVuLevel,
            sysVuLevel: status.sysVuLevel,
          }));

          setCameraCenter({
            x: status.latestCameraX,
            y: status.latestCameraY,
          });

          setTelemetryPoint({
            x: status.latestCameraX,
            y: status.latestCameraY,
          });
        } catch (e) {
          console.error("[PanGlide] Error polling recording status:", e);
        }
      }, 250); // Lightweight 4 Hz status poll — no frame streaming
    } else {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      setTelemetryPoint(null);
    }

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
    };
  }, [recordingState.isRecording]);

  // 3. Playback Transport & Seeking
  const handleTogglePlay = () => {
    if (!videoRef.current) return;
    if (videoRef.current.paused) {
      videoRef.current
        .play()
        .then(() => setIsPlaying(true))
        .catch((e) => console.error("[PanGlide] Video playback error:", e));
    } else {
      videoRef.current.pause();
      setIsPlaying(false);
    }
  };

  const handleSeek = (ms: number) => {
    setCurrentPlayheadMs(ms);
    if (videoRef.current) {
      videoRef.current.currentTime = ms / 1000;
    }
  };

  // 4. Start or Stop Recording via native Tauri IPC + Window Lifecycle
  const startRecording = async () => {
    try {
      if (videoRef.current) {
        videoRef.current.pause();
      }
      setIsPlaying(false);
      setVideoUrl(null);
      setRawVideoPath(null);
      setSolvedKeyframes([]);
      setAutoBlurMarkers([]);
      setRejectedTakes([]);
      setZoomKeyframes([]);
      setDurationMs(0);
      setCurrentPlayheadMs(0);

      await invoke("start_recording", { sourceId: selectedSourceRef.current || null });

      setRecordingState((prev) => ({
        ...prev,
        isRecording: true,
        elapsedMs: 0,
      }));

      // Auto-minimize PanGlide window so it doesn't obstruct desktop recording
      try {
        await getCurrentWindow().minimize();
      } catch (winErr) {
        console.error("[PanGlide] Window minimize failed:", winErr);
      }
    } catch (err) {
      console.error("[PanGlide] Failed to start recording:", err);
    }
  };

  const stopRecording = async () => {
    try {
      const result = await invoke<RecordingResult>("stop_recording");

      setRecordingState((prev) => ({
        ...prev,
        isRecording: false,
        micVuLevel: 0.0,
        sysVuLevel: 0.0,
      }));

      // Auto-restore and focus PanGlide window
      try {
        const appWindow = getCurrentWindow();
        await appWindow.unminimize();
        await appWindow.show();
        await appWindow.setFocus();
      } catch (winErr) {
        console.error("[PanGlide] Window restore failed:", winErr);
      }

      // Use Tauri asset protocol to play back the MP4 — clean URL normalization
      if (result.videoPath) {
        setRawVideoPath(result.videoPath);
        const cleanPath = result.videoPath.replace(/\\/g, "/");
        const assetUrl = convertFileSrc(cleanPath);
        console.log("[PanGlide] Loaded video asset URL:", assetUrl, "from clean path:", cleanPath);
        setVideoUrl(assetUrl);
      }
      setSolvedKeyframes(result.cameraKeyframes || []);
      setDurationMs(result.durationMs);
      setAutoBlurMarkers(result.autoBlurMarkers);
      setRejectedTakes(result.rejectedTakeIntervals || result.rejectedTakes || []);
      setZoomKeyframes(result.zoomKeyframes);
      setCurrentPlayheadMs(0);
    } catch (err) {
      console.error("[PanGlide] Failed to stop recording:", err);
      setRecordingState((prev) => ({ ...prev, isRecording: false }));
    }
  };

  // 4. Start or Stop Recording via native Tauri IPC + Window Lifecycle
  const handleToggleRecord = async () => {
    if (!recordingState.isRecording) {
      await startRecording();
    } else {
      await stopRecording();
    }
  };

  // Keep ref up to date for global hotkey listener
  const handleToggleRecordRef = useRef(handleToggleRecord);
  handleToggleRecordRef.current = handleToggleRecord;

  // 5. Global Hotkey Listener: Ctrl+Shift+R or F9 emitted from native WH_KEYBOARD_LL hook
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("global_hotkey_toggle_record", () => {
      console.log("[PanGlide] Global hotkey toggle record received via low-level hook");
      handleToggleRecordRef.current();
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err) => {
        console.warn("[PanGlide] Failed to listen to global hotkey:", err);
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 6. Live Snip Listener: Receive snip_recorded event from low-level Ctrl+Z hook or trigger_live_snip
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<RejectedTakeSegment>("snip_recorded", (event) => {
      console.log("[PanGlide] snip_recorded event received:", event.payload);
      setRejectedTakes((prev) => {
        if (prev.some((take) => take.id === event.payload.id)) {
          return prev;
        }
        return [...prev, event.payload];
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err) => {
        console.warn("[PanGlide] Failed to listen to snip_recorded:", err);
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 7. Live Snip (Ctrl+Z): Cuts preceding 5 seconds from active take via Rust backend
  const handleTriggerSnip = async () => {
    if (!recordingState.isRecording) return;
    try {
      const snip = await invoke<RejectedTakeSegment>("trigger_live_snip");
      setRejectedTakes((prev) => {
        if (prev.some((take) => take.id === snip.id)) {
          return prev;
        }
        return [...prev, snip];
      });
    } catch (err) {
      console.error("[PanGlide] Live snip error:", err);
    }
  };

  // 7. Reset Video to return to empty "Ready to Record" studio state
  const handleResetVideo = () => {
    if (videoRef.current) {
      videoRef.current.pause();
      videoRef.current.currentTime = 0;
    }
    setIsPlaying(false);
    setVideoUrl(null);
    setRawVideoPath(null);
    setAutoBlurMarkers([]);
    setRejectedTakes([]);
    setZoomKeyframes([]);
    setDurationMs(0);
    setCurrentPlayheadMs(0);
  };

  // 8. Source change handler from FloatingController
  const handleSourceChange = (sourceId: string) => {
    selectedSourceRef.current = sourceId;
  };

  const handleStartExport = (config: ExportConfig) => {
    console.log("[PanGlide] Master export finished:", config);
  };

  return (
    <div className="flex flex-col h-screen w-screen bg-[#0B0D13] text-[#F9FAFB] overflow-hidden select-none font-sans">
      {/* Top Application Header */}
      <header className="flex items-center justify-between h-12 px-4 bg-[#141721] border-b border-[#252B3B] z-40">
        {/* Brand */}
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-2">
            <div className="p-1.5 rounded-lg bg-indigo-600 text-white shadow-md shadow-indigo-600/30">
              <Video className="w-4 h-4" />
            </div>
            <span className="text-sm font-bold tracking-tight text-white">PanGlide</span>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-[#0B0D13] text-indigo-400 border border-[#252B3B]">
            v1.0.0 • Open Source
          </span>
        </div>

        {/* Header Actions */}
        <div className="flex items-center space-x-3">
          {/* Export Master Button */}
          <button
            onClick={() => setIsExportModalOpen(true)}
            className="flex items-center space-x-1.5 px-3 py-1 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs shadow-md shadow-indigo-600/30 transition-all cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            <span>Export</span>
          </button>
        </div>
      </header>

      {/* Main Workspace (Canvas + Inspector Sidebar + Floating Controller) */}
      <div className="relative flex-1 flex overflow-hidden">
        {/* Floating Capsule Controller (Top-Center) */}
        <FloatingController
          recordingState={recordingState}
          onToggleRecord={handleToggleRecord}
          onTriggerSnip={handleTriggerSnip}
          onSourceChange={handleSourceChange}
        />

        {/* Studio Canvas Viewport */}
        <div className="flex-1 h-full overflow-hidden flex items-center justify-center">
          <StudioCanvas
            aspectRatio={aspectRatio}
            cornerRadius={cornerRadius}
            dropShadowSpread={dropShadowSpread}
            backdropId={backdropId}
            zoomScale={zoomScale}
            showFocusReticle={showFocusReticle}
            autoRedactEnabled={autoRedactEnabled}
            autoBlurMarkers={autoBlurMarkers}
            cameraCenter={cameraCenter}
            isRecording={recordingState.isRecording}
            videoUrl={videoUrl}
            videoPath={rawVideoPath}
            solvedKeyframes={solvedKeyframes}
            onResetVideo={handleResetVideo}
            telemetryPoint={telemetryPoint}
            videoRef={videoRef}
            isPlaying={isPlaying}
            onTogglePlay={handleTogglePlay}
            onTimeUpdate={(tSec) => {
              const currentMs = tSec * 1000;
              if (pruneRejectedTakes && rejectedTakes.length > 0) {
                const activeTake = rejectedTakes.find(
                  (take) => currentMs >= take.startTimeMs && currentMs < take.endTimeMs
                );
                if (activeTake && videoRef.current) {
                  videoRef.current.currentTime = activeTake.endTimeMs / 1000;
                  setCurrentPlayheadMs(activeTake.endTimeMs);
                  return;
                }
              }
              setCurrentPlayheadMs(currentMs);
            }}
            onLoadedMetadata={(durSec) => setDurationMs(durSec * 1000)}
            onEnded={() => {
              setIsPlaying(false);
              setCurrentPlayheadMs(0);
              if (videoRef.current) {
                videoRef.current.currentTime = 0;
              }
            }}
          />
        </div>

        {/* Right Inspector Sidebar */}
        <InspectorSidebar
          zoomScale={zoomScale}
          onZoomChange={setZoomScale}
          aspectRatio={aspectRatio}
          onAspectRatioChange={setAspectRatio}
          cornerRadius={cornerRadius}
          onCornerRadiusChange={setCornerRadius}
          dropShadowSpread={dropShadowSpread}
          onDropShadowChange={setDropShadowSpread}
          backdropId={backdropId}
          onBackdropChange={setBackdropId}
          showFocusReticle={showFocusReticle}
          onToggleFocusReticle={() => setShowFocusReticle(!showFocusReticle)}
          springPreset={springPreset}
          onSpringPresetChange={setSpringPreset}
          deadzoneEnabled={deadzoneEnabled}
          onToggleDeadzone={() => setDeadzoneEnabled(!deadzoneEnabled)}
          pruneRejectedTakes={pruneRejectedTakes}
          onTogglePruneRejectedTakes={() => setPruneRejectedTakes(!pruneRejectedTakes)}
          autoTrackingEnabled={autoTrackingEnabled}
          onToggleAutoTracking={() => setAutoTrackingEnabled(!autoTrackingEnabled)}
        />
      </div>

      {/* Bottom Timeline Scrubber */}
      <Timeline
        durationMs={durationMs}
        currentPlayheadMs={currentPlayheadMs}
        onSeek={handleSeek}
        zoomKeyframes={zoomKeyframes}
        autoBlurMarkers={autoBlurMarkers}
        rejectedTakes={rejectedTakes}
        isPlaying={isPlaying}
        onTogglePlay={handleTogglePlay}
        videoLoaded={!!videoUrl}
        videoRef={videoRef}
      />

      {/* Modals */}
      <ExportModal
        isOpen={isExportModalOpen}
        onClose={() => setIsExportModalOpen(false)}
        onStartExport={handleStartExport}
        onExportSuccess={(res) => {
          setExportToast({
            filePath: res.destinationPath,
            resolution: res.resolution,
          });
          setTimeout(() => {
            setExportToast(null);
          }, 8000);
        }}
        currentVideoPath={rawVideoPath}
        activeAspectRatio={aspectRatio}
        activeZoomScale={zoomScale}
        activeBackdropId={backdropId}
        pruneRejectedTakes={pruneRejectedTakes}
        autoTrackingEnabled={autoTrackingEnabled}
        autoRedactEnabled={autoRedactEnabled}
        autoBlurMarkers={autoBlurMarkers}
      />


      {/* Export Success Toast */}
      {exportToast && (
        <div className="fixed bottom-20 right-6 z-50 flex items-start space-x-3 p-4 bg-[#141721]/95 border border-emerald-500/60 rounded-xl shadow-2xl backdrop-blur-md max-w-md animate-in slide-in-from-bottom-5 duration-300">
          <div className="p-1 rounded-full bg-emerald-500/20 text-emerald-400 mt-0.5">
            <CheckCircle2 className="w-4 h-4" />
          </div>
          <div className="flex-1 space-y-1">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-white">Rendered Video Exported!</span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-950/80 text-emerald-300 border border-emerald-500/40">
                {exportToast.resolution}
              </span>
            </div>
            <p className="text-[11px] font-mono text-[#94A3B8] break-all leading-tight">
              {exportToast.filePath}
            </p>
          </div>
          <button
            onClick={() => setExportToast(null)}
            className="text-[#94A3B8] hover:text-white transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}
    </div>
  );
}
