import React, { useRef, useEffect } from "react";
import {
  Play,
  Pause,
  SkipBack,
  Scissors,
  ShieldAlert,
  Search,
} from "lucide-react";
import type { AutoBlurMarker, RejectedTakeSegment, ZoomKeyframe } from "../types";

interface TimelineProps {
  durationMs: number;
  currentPlayheadMs: number;
  onSeek: (ms: number) => void;
  zoomKeyframes: ZoomKeyframe[];
  autoBlurMarkers: AutoBlurMarker[];
  rejectedTakes: RejectedTakeSegment[];
  isPlaying?: boolean;
  onTogglePlay?: () => void;
  videoLoaded?: boolean;
  videoRef?: React.RefObject<HTMLVideoElement | null>;
}

export default function Timeline({
  durationMs,
  currentPlayheadMs,
  onSeek,
  zoomKeyframes,
  autoBlurMarkers,
  rejectedTakes,
  isPlaying = false,
  onTogglePlay,
  videoLoaded = false,
  videoRef,
}: TimelineProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef(false);

  const formatTime = (ms: number) => {
    if (!ms || isNaN(ms) || ms < 0) return "00:00.00";
    const totalSec = Math.floor(ms / 1000);
    const m = Math.floor(totalSec / 60);
    const s = totalSec % 60;
    const cs = Math.floor((ms % 1000) / 10);
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(cs).padStart(2, "0")}`;
  };

  const handleSeekFromEvent = (clientX: number) => {
    if (!trackRef.current || durationMs <= 0) return;
    const rect = trackRef.current.getBoundingClientRect();
    const ratio = (clientX - rect.left) / rect.width;
    const clampedRatio = Math.max(0, Math.min(1, ratio));
    onSeek(clampedRatio * durationMs);
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    if (durationMs <= 0) return;
    isDraggingRef.current = true;
    handleSeekFromEvent(e.clientX);

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (isDraggingRef.current) {
        handleSeekFromEvent(moveEvent.clientX);
      }
    };

    const handleMouseUp = () => {
      isDraggingRef.current = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  };

  const onTogglePlayRef = useRef(onTogglePlay);
  onTogglePlayRef.current = onTogglePlay;
  const onSeekRef = useRef(onSeek);
  onSeekRef.current = onSeek;
  const currentPlayheadMsRef = useRef(currentPlayheadMs);
  currentPlayheadMsRef.current = currentPlayheadMs;
  const durationMsRef = useRef(durationMs);
  durationMsRef.current = durationMs;

  // Keyboard shortcut: Spacebar to toggle Play/Pause, ArrowLeft/ArrowRight for Scrubbing
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeTag = document.activeElement?.tagName.toLowerCase();
      if (activeTag === "input" || activeTag === "textarea") {
        return;
      }

      if (e.code === "Space") {
        if (videoLoaded && onTogglePlayRef.current) {
          e.preventDefault();
          onTogglePlayRef.current();
        }
        return;
      }

      if (videoLoaded && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        const deltaSec = e.shiftKey ? 1.0 : 1.0 / 60.0;

        if (videoRef?.current) {
          const currentTime = videoRef.current.currentTime;
          const duration = videoRef.current.duration || durationMsRef.current / 1000;
          if (e.key === "ArrowLeft") {
            videoRef.current.currentTime = Math.max(0, currentTime - deltaSec);
          } else {
            videoRef.current.currentTime = Math.min(duration, currentTime + deltaSec);
          }
          onSeekRef.current(videoRef.current.currentTime * 1000);
        } else {
          const deltaMs = deltaSec * 1000;
          const targetMs =
            e.key === "ArrowLeft"
              ? Math.max(0, currentPlayheadMsRef.current - deltaMs)
              : Math.min(durationMsRef.current, currentPlayheadMsRef.current + deltaMs);
          onSeekRef.current(targetMs);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [videoLoaded, videoRef]);

  const playheadPercent = durationMs > 0 ? Math.min(100, Math.max(0, (currentPlayheadMs / durationMs) * 100)) : 0;

  return (
    <div className="w-full h-36 bg-[#141721] border-t border-[#252B3B] flex flex-col select-none p-3 space-y-2">
      {/* Timeline Controls Header */}
      <div className="flex items-center justify-between px-2">
        <div className="flex items-center space-x-3">
          <button
            onClick={() => onSeek(0)}
            disabled={!videoLoaded}
            title="Rewind to Start"
            className="p-1 rounded text-[#94A3B8] hover:text-[#F9FAFB] hover:bg-[#252B3B] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            <SkipBack className="w-4 h-4" />
          </button>
          <button
            onClick={onTogglePlay}
            disabled={!videoLoaded}
            title={isPlaying ? "Pause (Space)" : "Play (Space)"}
            className="p-1.5 rounded-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 disabled:cursor-not-allowed text-white shadow-md shadow-indigo-600/30 transition-colors"
          >
            {isPlaying ? (
              <Pause className="w-3.5 h-3.5 fill-current" />
            ) : (
              <Play className="w-3.5 h-3.5 fill-current ml-0.5" />
            )}
          </button>
          <div className="font-mono text-xs text-[#F9FAFB] tracking-wider">
            <span>{formatTime(currentPlayheadMs)}</span>
            <span className="text-[#94A3B8] mx-1">/</span>
            <span className="text-[#94A3B8]">{formatTime(durationMs)}</span>
          </div>
        </div>

        {/* Legend */}
        <div className="flex items-center space-x-4 text-[11px] text-[#94A3B8]">
          <div className="flex items-center space-x-1.5">
            <span className="w-2.5 h-2.5 rounded bg-indigo-500" />
            <span>Kinematic Zoom</span>
          </div>
          <div className="flex items-center space-x-1.5">
            <span className="w-2.5 h-2.5 rounded bg-amber-400" />
            <span>Privacy Auto-Blur</span>
          </div>
          <div className="flex items-center space-x-1.5">
            <span className="w-2.5 h-2.5 rounded bg-rose-500/80 border border-rose-400" />
            <span>5s Live Snip (Ctrl+Z)</span>
          </div>
        </div>
      </div>

      {/* Main Timeline Scrubber Track Area */}
      <div className="relative flex-1 bg-[#0B0D13] border border-[#252B3B] rounded-lg overflow-hidden flex flex-col justify-center px-4">
        {/* Interactive Track Base */}
        <div
          ref={trackRef}
          onMouseDown={handleMouseDown}
          className={`relative w-full h-16 bg-[#141721]/50 rounded cursor-pointer group ${
            !videoLoaded ? "cursor-default opacity-60" : ""
          }`}
        >
          {/* Time ruler ticks */}
          <div className="absolute inset-0 flex justify-between items-start pointer-events-none opacity-20 px-1 pt-1">
            {Array.from({ length: 11 }).map((_, i) => (
              <div key={i} className="flex flex-col items-center">
                <div className="w-[1px] h-3 bg-white" />
                <span className="text-[8px] font-mono text-white mt-0.5">
                  {durationMs > 0 ? `${Math.round((i / 10) * (durationMs / 1000))}s` : `${i * 3}s`}
                </span>
              </div>
            ))}
          </div>

          {/* Rejected Take Indicators (Red/Amber Striped Cutout Bands) */}
          {rejectedTakes.map((take) => {
            const startPct = durationMs > 0 ? (take.startTimeMs / durationMs) * 100 : 0;
            const widthPct = durationMs > 0 ? ((take.endTimeMs - take.startTimeMs) / durationMs) * 100 : 0;
            return (
              <div
                key={take.id}
                className="absolute top-0 bottom-0 border-l-2 border-r-2 border-amber-500/80 z-10 flex items-center justify-center overflow-hidden pointer-events-none"
                style={{
                  left: `${startPct}%`,
                  width: `${widthPct}%`,
                  backgroundImage:
                    "repeating-linear-gradient(45deg, rgba(245, 158, 11, 0.25), rgba(245, 158, 11, 0.25) 8px, rgba(239, 68, 68, 0.35) 8px, rgba(239, 68, 68, 0.35) 16px)",
                }}
                title={`Excised Take: ${take.durationSec}s cut (Ctrl+Z)`}
              >
                <div className="flex items-center space-x-1 px-1.5 py-0.5 rounded bg-rose-950/90 text-[9px] font-mono text-amber-300 border border-amber-500/50 shadow-md">
                  <Scissors className="w-2.5 h-2.5 text-amber-400" />
                  <span>5s Cut Zone</span>
                </div>
              </div>
            );
          })}

          {/* Zoom Badges */}
          {zoomKeyframes.map((kf) => {
            const posPct = durationMs > 0 ? (kf.timeMs / durationMs) * 100 : 0;
            return (
              <div
                key={kf.id}
                className="absolute bottom-1 z-20 -translate-x-1/2 flex items-center space-x-1 px-1.5 py-0.5 rounded bg-indigo-600 text-white text-[9px] font-bold shadow-md shadow-indigo-600/40 pointer-events-none"
                style={{ left: `${posPct}%` }}
                title={`Zoom Keyframe: ${kf.scale}x`}
              >
                <Search className="w-2.5 h-2.5" />
                <span>{kf.scale.toFixed(1)}x</span>
              </div>
            );
          })}

          {/* Auto-Blur Markers */}
          {autoBlurMarkers.map((ab) => {
            const posPct = durationMs > 0 ? (ab.timeMs / durationMs) * 100 : 0;
            return (
              <div
                key={ab.id}
                className="absolute top-1 z-20 -translate-x-1/2 flex items-center space-x-1 px-1.5 py-0.5 rounded bg-amber-500/20 border border-amber-400 text-amber-300 text-[9px] font-medium backdrop-blur-sm pointer-events-none"
                style={{ left: `${posPct}%` }}
                title={`Redaction: ${ab.tokenType} (${ab.preview})`}
              >
                <ShieldAlert className="w-2.5 h-2.5 text-amber-400" />
                <span>{ab.tokenType}</span>
              </div>
            );
          })}

          {/* Playhead Needle */}
          {videoLoaded && (
            <div
              className="absolute top-0 bottom-0 z-30 pointer-events-none transition-all duration-75 flex flex-col items-center"
              style={{ left: `${playheadPercent}%` }}
            >
              {/* Playhead Head */}
              <div className="w-3.5 h-3.5 bg-indigo-400 rounded-full shadow-lg shadow-indigo-400/50 -mt-1.5 border-2 border-white" />
              {/* Playhead Line */}
              <div className="w-0.5 flex-1 bg-indigo-400 shadow-sm shadow-indigo-400/80" />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
