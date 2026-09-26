import { useRef, useState, useEffect } from "react";
import { Video, ShieldCheck, Sparkles, RotateCcw, Play, Lock } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { AspectRatioPreset, AutoBlurMarker, CameraFrame } from "../types";

interface StudioCanvasProps {
  aspectRatio: AspectRatioPreset;
  cornerRadius: number;
  dropShadowSpread: number;
  backdropId: string;
  zoomScale: number;
  showFocusReticle: boolean;
  autoRedactEnabled: boolean;
  preEncodeTokenMasking?: boolean;
  autoBlurMarkers: AutoBlurMarker[];
  cameraCenter: { x: number; y: number }; // normalized [0..1]
  // Real Capture & Video props
  isRecording: boolean;
  videoUrl: string | null;
  videoPath?: string | null;
  solvedKeyframes?: CameraFrame[];
  onResetVideo?: () => void;
  telemetryPoint?: { x: number; y: number } | null;
  // Custom video playback bindings
  videoRef?: React.RefObject<HTMLVideoElement | null>;
  isPlaying?: boolean;
  onTogglePlay?: () => void;
  onTimeUpdate?: (currentTimeSec: number) => void;
  onLoadedMetadata?: (durationSec: number) => void;
  onEnded?: () => void;
}

export default function StudioCanvas({
  aspectRatio,
  cornerRadius,
  dropShadowSpread,
  backdropId,
  zoomScale,
  showFocusReticle,
  autoRedactEnabled,
  preEncodeTokenMasking,
  autoBlurMarkers,
  isRecording,
  videoUrl,
  videoPath,
  solvedKeyframes,
  onResetVideo,
  telemetryPoint: _telemetryPoint,
  videoRef: externalVideoRef,
  isPlaying = false,
  onTogglePlay,
  onTimeUpdate,
  onLoadedMetadata,
  onEnded,
}: StudioCanvasProps) {
  const canvasContainerRef = useRef<HTMLDivElement>(null);
  const internalVideoRef = useRef<HTMLVideoElement>(null);
  const activeVideoRef = externalVideoRef || internalVideoRef;

  const [cameraFrames, setCameraFrames] = useState<CameraFrame[]>(solvedKeyframes || []);
  const [reticleState, setReticleState] = useState<{ x: number; y: number; zoom: number }>({
    x: 0.5,
    y: 0.5,
    zoom: 1.0,
  });

  // On recording load, fetch solved camera frames via get_solved_camera_keyframes
  useEffect(() => {
    if (solvedKeyframes && solvedKeyframes.length > 0) {
      setCameraFrames(solvedKeyframes);
      return;
    }

    if (videoUrl) {
      invoke<CameraFrame[]>("get_solved_camera_keyframes", {
        sessionId: videoPath || "latest",
      })
        .then((frames) => {
          if (frames && frames.length > 0) {
            console.log(`[PanGlide Studio] Loaded ${frames.length} solved camera frames`);
            setCameraFrames(frames);
          }
        })
        .catch((err) => {
          console.warn("[PanGlide Studio] Failed to fetch solved camera keyframes:", err);
        });
    } else {
      setCameraFrames([]);
      setReticleState({ x: 0.5, y: 0.5, zoom: 1.0 });
    }
  }, [videoUrl, videoPath, solvedKeyframes]);

  // Video playback requestAnimationFrame loop
  useEffect(() => {
    let animId: number;

    const updateCameraStyling = () => {
      const video = activeVideoRef.current;
      if (video && cameraFrames.length > 0) {
        const curSec = video.currentTime;
        const frameIdx = Math.min(
          cameraFrames.length - 1,
          Math.max(0, Math.round(curSec * 60))
        );
        const frame = cameraFrames[frameIdx];

        if (frame) {
          // Set transformOrigin = `${frame.x * 100}% ${frame.y * 100}%`
          video.style.transformOrigin = `${frame.x * 100}% ${frame.y * 100}%`;
          // Set transform = `scale(${frame.zoom})`
          video.style.transform = `scale(${frame.zoom})`;

          // Update ACTIVE FOCUS reticle badge text and target coordinates
          setReticleState({
            x: frame.x,
            y: frame.y,
            zoom: frame.zoom,
          });
        }
      } else if (video && cameraFrames.length === 0) {
        video.style.transformOrigin = "50% 50%";
        video.style.transform = `scale(${zoomScale})`;
      }

      animId = requestAnimationFrame(updateCameraStyling);
    };

    animId = requestAnimationFrame(updateCameraStyling);
    return () => cancelAnimationFrame(animId);
  }, [cameraFrames, zoomScale]);

  // Backdrop background CSS styles
  const getBackdropStyle = () => {
    switch (backdropId) {
      case "aurora":
        return "bg-gradient-to-br from-[#0B0D13] via-[#1E1B4B] to-[#0F172A]";
      case "cyber":
        return "bg-gradient-to-tr from-[#1E112A] via-[#141721] to-[#0C1E33]";
      case "indigo":
        return "bg-gradient-to-b from-[#141721] via-[#1E1B4B] to-[#0B0D13]";
      case "slate":
        return "bg-[#0B0D13]";
      case "transparent":
        return "bg-[radial-gradient(#252B3B_1px,transparent_1px)] [background-size:16px_16px] bg-[#0B0D13]";
      default:
        return "bg-gradient-to-br from-[#0B0D13] via-[#141721] to-[#1E293B]";
    }
  };

  // Compute container aspect ratio class
  const getAspectClass = () => {
    switch (aspectRatio) {
      case "9:16":
        return "aspect-[9/16] max-h-[82vh]";
      case "1:1":
        return "aspect-square max-h-[75vh]";
      case "16:9":
      default:
        return "aspect-[16/9] w-full max-w-[1100px]";
    }
  };

  return (
    <div
      ref={canvasContainerRef}
      className={`relative flex items-center justify-center w-full h-full p-8 overflow-hidden select-none transition-colors duration-300 ${getBackdropStyle()}`}
    >
      {/* Framed Canvas Box */}
      <div
        className={`relative overflow-hidden transition-all duration-300 border border-[#252B3B] bg-[#0B0D13] ${getAspectClass()}`}
        style={{
          borderRadius: `${cornerRadius}px`,
          boxShadow: `0px 20px ${dropShadowSpread}px rgba(0, 0, 0, 0.7), 0px 0px ${Math.floor(
            dropShadowSpread * 0.4
          )}px rgba(99, 102, 241, 0.15)`,
        }}
      >
        {/* Scalable Content Surface with smooth cubic-bezier zoom */}
        <div
          className="relative w-full h-full origin-center"
          style={{
            transform: videoUrl ? "none" : `scale(${zoomScale})`,
            transformOrigin: "center center",
            transition: "transform 0.25s cubic-bezier(0.16, 1, 0.3, 1)",
          }}
        >
          {/* STATE 1: Completed Video Playback (Custom Controls - Native browser controls stripped) */}
          {videoUrl ? (
            <div
              className="relative w-full h-full bg-black flex items-center justify-center cursor-pointer group overflow-hidden"
              onClick={onTogglePlay}
            >
              <video
                ref={activeVideoRef}
                src={videoUrl}
                autoPlay={false}
                preload="auto"
                playsInline
                className="w-full h-full object-contain pointer-events-none will-change-transform"
                onTimeUpdate={(e) => onTimeUpdate?.(e.currentTarget.currentTime)}
                onLoadedMetadata={(e) => onLoadedMetadata?.(e.currentTarget.duration)}
                onEnded={onEnded}
                onError={(e) => console.error("Video Error:", e.currentTarget.error)}
              />

              {/* Center Play Overlay Icon when paused */}
              {!isPlaying && (
                <div className="absolute inset-0 flex items-center justify-center bg-black/30 pointer-events-none transition-opacity duration-200">
                  <div className="w-16 h-16 rounded-full bg-indigo-600/90 text-white flex items-center justify-center shadow-2xl backdrop-blur-sm group-hover:scale-110 transition-transform">
                    <Play className="w-7 h-7 fill-current ml-1" />
                  </div>
                </div>
              )}

              {/* Reset to Ready State Button */}
              {onResetVideo && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onResetVideo();
                  }}
                  className="absolute top-4 right-4 z-40 flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-[#141721]/90 hover:bg-[#1E2333] text-xs text-white border border-[#252B3B] shadow-xl backdrop-blur-md transition-all cursor-pointer"
                >
                  <RotateCcw className="w-3.5 h-3.5 text-indigo-400" />
                  <span>Record New Take</span>
                </button>
              )}
            </div>
          ) : isRecording ? (
            /* STATE 2: Active Recording — Clean lightweight UI (NO base64 frame streaming) */
            <div className="relative w-full h-full bg-[#0F111A] flex items-center justify-center overflow-hidden">
              <div className="flex flex-col items-center justify-center text-center p-6">
                <div className="relative flex h-5 w-5 mb-3">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-5 w-5 bg-rose-500"></span>
                </div>
                <span className="text-sm font-semibold text-white tracking-wide">
                  Recording in Progress
                </span>
                <span className="text-xs text-rose-400 mt-1 font-mono">
                  D3D11 → WGC → MFT H.264 • 60 FPS CFR
                </span>
                <span className="text-[10px] text-[#94A3B8] mt-2 font-mono">
                  Hardware encoder active — output to %LOCALAPPDATA%\PanGlide\recordings
                </span>
              </div>

              {/* Live Recording Watermark / Status */}
              <div className="absolute top-4 left-4 z-30 flex items-center space-x-2 px-2.5 py-1 rounded bg-black/60 backdrop-blur-md border border-rose-500/40 text-[11px] font-mono text-rose-400">
                <div className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
                <span className="font-bold tracking-wider">LIVE MFT CAPTURE</span>
              </div>
            </div>
          ) : (
            /* STATE 3: Clean Empty State (Ready to Record) */
            <div className="relative w-full h-full bg-[#0F111A] flex items-center justify-center overflow-hidden">
              <div className="relative z-10 flex flex-col items-center text-center p-8 max-w-md select-none">
                <div className="w-14 h-14 rounded-2xl bg-indigo-600/20 border border-indigo-500/30 flex items-center justify-center text-indigo-400 mb-4 shadow-xl shadow-indigo-600/20">
                  <Video className="w-7 h-7" />
                </div>
                <h3 className="text-base font-bold text-white tracking-tight">Ready to Record</h3>
                <p className="text-xs text-[#94A3B8] mt-1.5 leading-relaxed">
                  Press{" "}
                  <kbd className="px-1.5 py-0.5 rounded bg-[#141721] border border-[#252B3B] text-indigo-300 font-mono text-[11px]">
                    Ctrl+Shift+R
                  </kbd>{" "}
                  or click <span className="text-rose-400 font-semibold">REC</span> to start
                  capturing your real desktop screen.
                </p>

                <div className="mt-5 flex flex-wrap items-center justify-center gap-2 text-[11px] text-[#94A3B8]">
                  <span className="flex items-center space-x-1 px-2.5 py-1 rounded-full bg-[#141721] border border-[#252B3B]">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                    <span>WGC • MFT H.264 • 60 FPS</span>
                  </span>
                  <span className="flex items-center space-x-1 px-2.5 py-1 rounded-full bg-[#141721] border border-[#252B3B]">
                    <Sparkles className="w-3.5 h-3.5 text-indigo-400" />
                    <span>Kinematic Auto-Zoom</span>
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* DYNAMIC: Amber Frosted Auto-Redact Overlays */}
          {(isRecording || videoUrl) &&
            (autoRedactEnabled ?? preEncodeTokenMasking ?? true) &&
            autoBlurMarkers.map((marker) => (
              <div
                key={marker.id}
                className="absolute z-20 flex items-center justify-between px-2.5 py-1 bg-amber-500/25 backdrop-blur-md border-2 border-dashed border-amber-400 rounded-lg shadow-lg shadow-amber-500/10 transition-all duration-150"
                style={{
                  left: `${marker.bounds.x}%`,
                  top: `${marker.bounds.y}%`,
                  width: `${marker.bounds.width}%`,
                  height: `${marker.bounds.height}%`,
                }}
              >
                <div className="flex items-center space-x-1.5 text-[10px] text-amber-300 font-bold tracking-wider uppercase">
                  <Lock className="w-3 h-3 text-amber-400" />
                  <span>REDACTED</span>
                </div>
                <span className="text-[9px] font-mono text-amber-200/90">{marker.preview}</span>
              </div>
            ))}
        </div>

        {/* DYNAMIC: AMBER ANIMATED DASHED FOCUS RETICLE (Only rendered when video is actively loaded) */}
        {showFocusReticle && !!videoUrl && (
          <div className="absolute inset-0 pointer-events-none z-30">
            <div
              className="absolute w-52 h-36 border-2 border-dashed border-amber-400/90 rounded-xl shadow-2xl shadow-amber-500/20 animate-pulse transition-all duration-75"
              style={{
                left: `${reticleState.x * 100}%`,
                top: `${reticleState.y * 100}%`,
                transform: "translate(-50%, -50%)",
              }}
            >
              {/* Corner Accent Brackets */}
              <div className="absolute -top-1.5 -left-1.5 w-3.5 h-3.5 border-t-2 border-l-2 border-amber-400" />
              <div className="absolute -top-1.5 -right-1.5 w-3.5 h-3.5 border-t-2 border-r-2 border-amber-400" />
              <div className="absolute -bottom-1.5 -left-1.5 w-3.5 h-3.5 border-b-2 border-l-2 border-amber-400" />
              <div className="absolute -bottom-1.5 -right-1.5 w-3.5 h-3.5 border-b-2 border-r-2 border-amber-400" />

              {/* Center Crosshair */}
              <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-5 h-5 flex items-center justify-center">
                <div className="w-3 h-0.5 bg-amber-400" />
                <div className="w-0.5 h-3 bg-amber-400 absolute" />
              </div>

              {/* Reticle Badge */}
              <div className="absolute -bottom-7 left-1/2 -translate-x-1/2 px-2.5 py-0.5 rounded-full bg-[#0B0D13]/95 border border-amber-500/50 text-[10px] font-mono text-amber-300 font-semibold tracking-wider whitespace-nowrap shadow-md">
                ACTIVE FOCUS • {reticleState.zoom.toFixed(2)}x
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
