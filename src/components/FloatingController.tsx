import { useState, useEffect } from "react";
import { Circle, Square, Monitor, Mic, Volume2, Scissors } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RecordingState, CaptureSource, RejectedTakeSegment } from "../types";

interface FloatingControllerProps {
  recordingState: RecordingState;
  onToggleRecord: () => void;
  onTriggerSnip: () => void;
  onSourceChange?: (sourceId: string) => void;
}

export default function FloatingController({
  recordingState,
  onToggleRecord,
  onTriggerSnip,
  onSourceChange,
}: FloatingControllerProps) {
  const [sources, setSources] = useState<CaptureSource[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<string>("");
  const [showSnipBadge, setShowSnipBadge] = useState<boolean>(false);

  // Dynamically enumerate monitors from Rust backend on mount
  useEffect(() => {
    invoke<CaptureSource[]>("get_available_sources")
      .then((result) => {
        setSources(result);
        if (result.length > 0) {
          setSelectedSourceId(result[0].id);
          onSourceChange?.(result[0].id);
        }
      })
      .catch((err) => {
        console.warn("[PanGlide] Failed to enumerate sources:", err);
      });
  }, []);

  const handleSourceChange = (sourceId: string) => {
    setSelectedSourceId(sourceId);
    onSourceChange?.(sourceId);
  };

  // Format milliseconds into MM:SS:cs
  const formatTimer = (ms: number) => {
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    const centiseconds = Math.floor((ms % 1000) / 10);
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(centiseconds).padStart(2, "0")}`;
  };

  // Listen for snip_recorded event from Rust backend to show visual snip badge
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<RejectedTakeSegment>("snip_recorded", () => {
      setShowSnipBadge(true);
      const timer = setTimeout(() => {
        setShowSnipBadge(false);
      }, 2500);
      return () => clearTimeout(timer);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err) => {
        console.warn("[PanGlide] Failed to listen to snip_recorded in FloatingController:", err);
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Keyboard shortcut listener for Ctrl+Shift+R, F9, and Ctrl+Z
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey && e.shiftKey && (e.key === "r" || e.key === "R")) || e.key === "F9") {
        e.preventDefault();
        onToggleRecord();
      } else if (e.ctrlKey && (e.key === "z" || e.key === "Z")) {
        if (recordingState.isRecording) {
          e.preventDefault();
          onTriggerSnip();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [recordingState.isRecording, onToggleRecord, onTriggerSnip]);

  const cleanDisplayName = (name: string) => {
    return name
      .replace(/\\\\?\.\\DISPLAY/gi, "Display ")
      .replace(/\\\\?\.\\/g, "")
      .replace(/Display\s*(\d+)/i, "Display $1");
  };

  return (
    <div className="fixed top-6 left-1/2 -translate-x-1/2 z-50 flex items-center h-12 px-4 bg-[#141721]/95 backdrop-blur-md border border-[#252B3B] rounded-full shadow-2xl space-x-3 select-none transition-all duration-200 hover:border-[#3B4358]">
      {/* Record / Stop Capsule Button */}
      <button
        onClick={onToggleRecord}
        className={`flex items-center space-x-2 px-3 py-1.5 rounded-full text-xs font-semibold tracking-wide transition-all cursor-pointer ${
          recordingState.isRecording
            ? "bg-rose-500/20 text-rose-400 hover:bg-rose-500/30 border border-rose-500/40"
            : "bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-600/30"
        }`}
      >
        {recordingState.isRecording ? (
          <>
            <span className="relative flex h-2.5 w-2.5">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-rose-500"></span>
            </span>
            <Square className="w-3.5 h-3.5 fill-current" />
            <span>STOP</span>
          </>
        ) : (
          <>
            <Circle className="w-3.5 h-3.5 fill-current" />
            <span>REC</span>
          </>
        )}
      </button>

      {/* Monospace Duration Timer with Live Snip Badge */}
      <div className="relative flex items-center px-2 py-1 bg-[#0B0D13] border border-[#252B3B] rounded-md font-mono text-xs text-[#F9FAFB] tracking-wider min-w-[78px] justify-center">
        {formatTimer(recordingState.elapsedMs)}
        {showSnipBadge && (
          <span className="absolute -top-3.5 right-0 bg-amber-500 text-black text-[9px] font-black px-1.5 py-0.2 rounded-full shadow-lg flex items-center space-x-0.5 animate-bounce tracking-tight">
            <span>-5s ✂️</span>
          </span>
        )}
      </div>

      <div className="h-4 w-[1px] bg-[#252B3B]" />

      {/* Source Selector — dynamically populated from get_available_sources */}
      <div className="flex items-center space-x-1.5 text-xs text-[#94A3B8]">
        <Monitor className="w-3.5 h-3.5 text-indigo-400" />
        <select
          value={selectedSourceId}
          onChange={(e) => handleSourceChange(e.target.value)}
          disabled={recordingState.isRecording}
          className="bg-transparent text-xs text-[#F9FAFB] font-medium focus:outline-none cursor-pointer pr-1 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {sources.length === 0 ? (
            <option value="" className="bg-[#141721] text-[#F9FAFB]">
              Detecting monitors…
            </option>
          ) : (
            sources.map((s) => (
              <option key={s.id} value={s.id} className="bg-[#141721] text-[#F9FAFB]">
                {cleanDisplayName(s.name)}
              </option>
            ))
          )}
        </select>
      </div>

      <div className="h-4 w-[1px] bg-[#252B3B]" />

      {/* Live VU Meters (Mic & System Audio) */}
      <div className="flex items-center space-x-2 px-1">
        {/* Mic Meter */}
        <div className="flex items-center space-x-1" title="Microphone Level (48 kHz)">
          <Mic className="w-3 h-3 text-[#94A3B8]" />
          <div className="w-12 h-2 bg-[#0B0D13] rounded-full overflow-hidden flex border border-[#252B3B]">
            <div
              className="h-full bg-gradient-to-r from-emerald-500 via-yellow-500 to-rose-500 transition-all duration-75"
              style={{ width: `${Math.min(100, Math.max(0, recordingState.micVuLevel * 100))}%` }}
            />
          </div>
        </div>

        {/* System Audio Meter */}
        <div className="flex items-center space-x-1" title="System Loopback Level">
          <Volume2 className="w-3 h-3 text-[#94A3B8]" />
          <div className="w-12 h-2 bg-[#0B0D13] rounded-full overflow-hidden flex border border-[#252B3B]">
            <div
              className="h-full bg-gradient-to-r from-emerald-500 via-indigo-400 to-purple-500 transition-all duration-75"
              style={{ width: `${Math.min(100, Math.max(0, recordingState.sysVuLevel * 100))}%` }}
            />
          </div>
        </div>
      </div>

      <div className="h-4 w-[1px] bg-[#252B3B]" />

      {/* Live Snip Trigger (Ctrl+Z) */}
      <button
        onClick={onTriggerSnip}
        disabled={!recordingState.isRecording}
        title="Live Snip: Snip preceding 5 seconds (Ctrl+Z)"
        className={`flex items-center space-x-1.5 px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
          recordingState.isRecording
            ? "border-amber-500/40 text-amber-400 bg-amber-500/10 hover:bg-amber-500/20 cursor-pointer"
            : "border-[#252B3B] text-[#94A3B8]/40 cursor-not-allowed"
        }`}
      >
        <Scissors className="w-3 h-3" />
        <span>Snip 5s (Ctrl+Z)</span>
      </button>

      {/* Hotkey Hint */}
      <div className="hidden lg:flex items-center text-[10px] text-[#94A3B8] font-mono">
        <kbd className="px-1.5 py-0.5 bg-[#0B0D13] border border-[#252B3B] rounded text-[10px] text-indigo-400">
          Ctrl+Shift+R • F9
        </kbd>
      </div>
    </div>
  );
}
