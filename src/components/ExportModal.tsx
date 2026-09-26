import { useState, useEffect } from "react";
import {
  X,
  Download,
  Film,
  Sparkles,
  Layers,
  FileJson,
  Cpu,
  CheckCircle2,
  FolderCheck,
  AlertCircle,
  Sliders,
  ShieldCheck,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AspectRatioPreset, AutoBlurMarker, ResolutionPreset } from "../types";

interface ExportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onStartExport?: (config: ExportConfig) => void;
  onExportSuccess?: (result: {
    destinationPath: string;
    resolution: string;
    width: number;
    height: number;
  }) => void;
  currentVideoPath?: string | null;
  activeAspectRatio?: AspectRatioPreset;
  activeZoomScale?: number;
  activeBackdropId?: string;
  pruneRejectedTakes?: boolean;
  autoTrackingEnabled?: boolean;
  autoRedactEnabled?: boolean;
  autoBlurMarkers?: AutoBlurMarker[];
}

export interface ExportConfig {
  resolution: ResolutionPreset;
  aspectRatio: AspectRatioPreset;
  exportAlphaTrack: boolean;
  exportTelemetrySidecar: boolean;
  codec: "h264" | "hevc";
  encoderBackend: "mft" | "nvenc";
  destinationPath?: string;
}

export default function ExportModal({
  isOpen,
  onClose,
  onStartExport,
  onExportSuccess,
  currentVideoPath,
  activeAspectRatio = "16:9",
  activeZoomScale = 1.0,
  activeBackdropId = "aurora",
  pruneRejectedTakes = true,
  autoTrackingEnabled = true,
  autoRedactEnabled = true,
  autoBlurMarkers = [],
}: ExportModalProps) {
  const [resolution, setResolution] = useState<ResolutionPreset>("1080p");
  const [aspectRatio, setAspectRatio] = useState<AspectRatioPreset>(activeAspectRatio);
  const [zoomScale, setZoomScale] = useState<number>(activeZoomScale);
  const [exportAlphaTrack, setExportAlphaTrack] = useState(true);
  const [exportTelemetrySidecar, setExportTelemetrySidecar] = useState(true);
  const [codec, setCodec] = useState<"h264" | "hevc">("h264");
  const [encoderBackend, setEncoderBackend] = useState<"mft" | "nvenc">("mft");
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);
  const [savedDestination, setSavedDestination] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setAspectRatio(activeAspectRatio);
      setZoomScale(activeZoomScale);
    }
  }, [isOpen, activeAspectRatio, activeZoomScale]);

  if (!isOpen) return null;

  const handleExport = async () => {
    setErrorMessage(null);
    setSavedDestination(null);

    if (!currentVideoPath) {
      setErrorMessage("No active recording found to export. Please record a take first.");
      return;
    }

    try {
      // 1. Open native Windows Save File dialog
      const chosenPath = await invoke<string | null>("pick_export_destination", {
        defaultName: `panglide_${aspectRatio.replace(":", "_")}_${Date.now()}.mp4`,
      });

      // User canceled the dialog
      if (!chosenPath) {
        return;
      }

      setIsExporting(true);
      setExportProgress(10);

      // Listen for progress emitted from backend transcoder
      let unlistenProgress: (() => void) | undefined;
      try {
        const unlisten1 = await listen<{ progress: number; currentFrame: number }>(
          "export_progress",
          (event) => {
            setExportProgress(Math.min(95, Math.max(10, event.payload.progress)));
          }
        );
        const unlisten2 = await listen<{ progress: number; currentFrame: number }>(
          "export-progress",
          (event) => {
            setExportProgress(Math.min(95, Math.max(10, event.payload.progress)));
          }
        );
        unlistenProgress = () => {
          unlisten1();
          unlisten2();
        };
      } catch (evtErr) {
        console.warn("[PanGlide] Could not bind progress event:", evtErr);
      }

      // 2. Perform the real hardware-accelerated transcode pass
      const result = await invoke<{
        destinationPath: string;
        width: number;
        height: number;
        framesRendered: number;
        durationSec: number;
      }>("export_rendered_video", {
        payload: {
          sourcePath: currentVideoPath,
          destinationPath: chosenPath,
          aspectRatio,
          resolution,
          zoomScale,
          backgroundColor: activeBackdropId || "aurora",
          pruneRejectedTakes,
          autoTracking: autoTrackingEnabled,
          preEncodeTokenMasking: autoRedactEnabled,
          redactionRects: autoBlurMarkers?.map((marker) => ({
            x: marker.bounds.x > 1.0 ? marker.bounds.x / 100 : marker.bounds.x,
            y: marker.bounds.y > 1.0 ? marker.bounds.y / 100 : marker.bounds.y,
            width: marker.bounds.width > 1.0 ? marker.bounds.width / 100 : marker.bounds.width,
            height: marker.bounds.height > 1.0 ? marker.bounds.height / 100 : marker.bounds.height,
            label: marker.tokenType || "Secret Token",
          })) || [],
        },
      });

      if (unlistenProgress) {
        unlistenProgress();
      }

      setExportProgress(100);
      setSavedDestination(result.destinationPath);

      onStartExport?.({
        resolution,
        aspectRatio,
        exportAlphaTrack,
        exportTelemetrySidecar,
        codec,
        encoderBackend,
        destinationPath: result.destinationPath,
      });

      const resString = `${result.width}x${result.height}`;

      // Notify parent & show success toast
      if (onExportSuccess) {
        onExportSuccess({
          destinationPath: result.destinationPath,
          resolution: resString,
          width: result.width,
          height: result.height,
        });
      }

      // Auto-close modal after brief completion display
      setTimeout(() => {
        handleModalClose();
      }, 700);
    } catch (err: any) {
      console.error("[PanGlide] Export error:", err);
      setErrorMessage(typeof err === "string" ? err : err?.message || "Failed to export video.");
      setIsExporting(false);
    }
  };

  const handleModalClose = () => {
    if (isExporting) return;
    setSavedDestination(null);
    setErrorMessage(null);
    setIsExporting(false);
    setExportProgress(0);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm select-none p-4">
      <div className="w-full max-w-lg bg-[#141721] border border-[#252B3B] rounded-2xl shadow-2xl overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-[#252B3B]">
          <div className="flex items-center space-x-2.5">
            <div className="p-2 rounded-lg bg-indigo-600/20 text-indigo-400 border border-indigo-500/30">
              <Film className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-[#F9FAFB]">Export Master Recording</h2>
              <p className="text-xs text-[#94A3B8]">Hardware-accelerated NLE dual-track render</p>
            </div>
          </div>
          <button
            onClick={handleModalClose}
            className="p-1 rounded-lg text-[#94A3B8] hover:text-[#F9FAFB] hover:bg-[#252B3B] transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-5 space-y-5 text-xs text-[#94A3B8]">
          {/* Success Banner */}
          {savedDestination && (
            <div className="p-3.5 rounded-lg bg-emerald-950/40 border border-emerald-500/50 text-emerald-200 space-y-1 animate-in fade-in duration-200">
              <div className="flex items-center space-x-2 text-xs font-semibold text-emerald-400">
                <FolderCheck className="w-4 h-4" />
                <span>Video Successfully Exported!</span>
              </div>
              <p className="text-[11px] font-mono break-all text-emerald-300/90 pl-6">
                {savedDestination}
              </p>
            </div>
          )}

          {/* Error Banner */}
          {errorMessage && (
            <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-500/50 text-rose-200 flex items-center space-x-2 text-xs">
              <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* Resolution & Aspect Ratio */}
          <div className="grid grid-cols-2 gap-4">
            {/* Resolution Selector */}
            <div className="space-y-2">
              <label className="text-xs font-semibold text-[#F9FAFB]">Resolution</label>
              <div className="grid grid-cols-3 gap-1.5">
                {(["1080p", "1440p", "4K"] as ResolutionPreset[]).map((res) => (
                  <button
                    key={res}
                    onClick={() => setResolution(res)}
                    className={`py-2 rounded border font-semibold transition-colors ${
                      resolution === res
                        ? "bg-indigo-600 text-white border-indigo-500"
                        : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:text-[#F9FAFB]"
                    }`}
                  >
                    {res}
                  </button>
                ))}
              </div>
            </div>

            {/* Aspect Ratio Presets */}
            <div className="space-y-2">
              <label className="text-xs font-semibold text-[#F9FAFB]">Format Preset</label>
              <div className="grid grid-cols-3 gap-1.5">
                {[
                  { id: "16:9", label: "16:9" },
                  { id: "9:16", label: "9:16" },
                  { id: "1:1", label: "1:1" },
                ].map((item) => (
                  <button
                    key={item.id}
                    onClick={() => setAspectRatio(item.id as AspectRatioPreset)}
                    className={`py-2 rounded border font-semibold transition-colors ${
                      aspectRatio === item.id
                        ? "bg-indigo-600 text-white border-indigo-500"
                        : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:text-[#F9FAFB]"
                    }`}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Hardware Zoom & Canvas Framing */}
            <div className="col-span-2 p-3 bg-[#0B0D13] border border-[#252B3B] rounded-lg space-y-2">
              <div className="flex items-center justify-between text-xs">
                <span className="font-semibold text-[#F9FAFB] flex items-center space-x-1.5">
                  <Sliders className="w-3.5 h-3.5 text-indigo-400" />
                  <span>Hardware Zoom & Framing Magnification</span>
                </span>
                <span className="font-mono text-indigo-400 font-bold">{zoomScale.toFixed(2)}x</span>
              </div>
              <input
                type="range"
                min="1.0"
                max="2.5"
                step="0.1"
                value={zoomScale}
                onChange={(e) => setZoomScale(parseFloat(e.target.value))}
                className="w-full accent-indigo-500 h-1.5 bg-[#141721] rounded-lg appearance-none cursor-pointer"
              />
              <div className="flex items-center justify-between text-[11px] text-[#94A3B8]">
                <span>Canvas Format: {aspectRatio} • Target: {resolution}</span>
                <span>Background Framing: {activeBackdropId}</span>
              </div>
            </div>
          </div>

          {/* Codec & Encoder Backend */}
          <div className="grid grid-cols-2 gap-4 pt-1">
            <div className="space-y-2">
              <label className="text-xs font-semibold text-[#F9FAFB]">Video Codec</label>
              <div className="grid grid-cols-2 gap-1.5">
                <button
                  onClick={() => setCodec("h264")}
                  className={`py-2 rounded border font-medium transition-colors ${
                    codec === "h264"
                      ? "bg-indigo-600/30 border-indigo-500 text-indigo-300 font-semibold"
                      : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8]"
                  }`}
                >
                  H.264 (MP4)
                </button>
                <button
                  onClick={() => setCodec("hevc")}
                  className={`py-2 rounded border font-medium transition-colors ${
                    codec === "hevc"
                      ? "bg-indigo-600/30 border-indigo-500 text-indigo-300 font-semibold"
                      : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8]"
                  }`}
                >
                  HEVC (H.265)
                </button>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-xs font-semibold text-[#F9FAFB] flex items-center space-x-1">
                <Cpu className="w-3.5 h-3.5 text-indigo-400" />
                <span>Hardware Acceleration</span>
              </label>
              <div className="grid grid-cols-2 gap-1.5">
                <button
                  onClick={() => setEncoderBackend("mft")}
                  className={`py-2 rounded border font-medium transition-colors ${
                    encoderBackend === "mft"
                      ? "bg-indigo-600/30 border-indigo-500 text-indigo-300 font-semibold"
                      : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8]"
                  }`}
                >
                  Windows MFT
                </button>
                <button
                  onClick={() => setEncoderBackend("nvenc")}
                  className={`py-2 rounded border font-medium transition-colors ${
                    encoderBackend === "nvenc"
                      ? "bg-indigo-600/30 border-indigo-500 text-indigo-300 font-semibold"
                      : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8]"
                  }`}
                >
                  NVENC GPU
                </button>
              </div>
            </div>
          </div>

          <div className="h-[1px] bg-[#252B3B]" />

          {/* Differentiators: Alpha Track & Telemetry Sidecar */}
          <div className="space-y-2.5">
            <span className="text-xs font-semibold text-[#F9FAFB] flex items-center space-x-1.5">
              <Sparkles className="w-3.5 h-3.5 text-indigo-400" />
              <span>NLE Post-Production Bundles</span>
            </span>

            {/* Alpha Layer Toggle */}
            <label className="flex items-center justify-between p-3 rounded-lg bg-[#0B0D13] border border-[#252B3B] cursor-pointer hover:border-[#3B4358] transition-colors">
              <div className="flex items-center space-x-2.5">
                <Layers className="w-4 h-4 text-purple-400" />
                <div>
                  <div className="text-xs font-medium text-[#F9FAFB]">Transparent Alpha-Channel Layer</div>
                  <div className="text-[11px] text-[#94A3B8]">
                    Export cursor ripples & clicks as transparent WebM / ProRes 4444
                  </div>
                </div>
              </div>
              <input
                type="checkbox"
                checked={exportAlphaTrack}
                onChange={(e) => setExportAlphaTrack(e.target.checked)}
                className="w-4 h-4 accent-indigo-500 rounded cursor-pointer"
              />
            </label>

            {/* Telemetry Sidecar Toggle */}
            <label className="flex items-center justify-between p-3 rounded-lg bg-[#0B0D13] border border-[#252B3B] cursor-pointer hover:border-[#3B4358] transition-colors">
              <div className="flex items-center space-x-2.5">
                <FileJson className="w-4 h-4 text-emerald-400" />
                <div>
                  <div className="text-xs font-medium text-[#F9FAFB]">JSON Telemetry Sidecar</div>
                  <div className="text-[11px] text-[#94A3B8]">
                    Frame-by-frame cursor paths, velocity vectors, and zoom keyframes
                  </div>
                </div>
              </div>
              <input
                type="checkbox"
                checked={exportTelemetrySidecar}
                onChange={(e) => setExportTelemetrySidecar(e.target.checked)}
                className="w-4 h-4 accent-indigo-500 rounded cursor-pointer"
              />
            </label>

            {/* Pre-Encode Privacy Token Redaction Status */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-[#0B0D13] border border-amber-500/30">
              <div className="flex items-center space-x-2.5">
                <ShieldCheck className="w-4 h-4 text-amber-400" />
                <div>
                  <div className="text-xs font-medium text-[#F9FAFB]">Pre-Encode Frosted Glass Token Redaction</div>
                  <div className="text-[11px] text-[#94A3B8]">
                    {autoRedactEnabled
                      ? "Active: API keys, JWTs, and credentials permanently blurred (#F59E0B) before H.264 compression"
                      : "Disabled: Raw screen credentials will not be obscured"}
                  </div>
                </div>
              </div>
              <span
                className={`text-[10px] font-bold px-2 py-0.5 rounded ${
                  autoRedactEnabled
                    ? "bg-amber-500/20 text-amber-300 border border-amber-500/40"
                    : "bg-gray-800 text-gray-400 border border-gray-700"
                }`}
              >
                {autoRedactEnabled ? "PROTECTED" : "OFF"}
              </span>
            </div>
          </div>

          {/* Export Progress Bar when active */}
          {isExporting && (
            <div className="p-3 bg-[#0B0D13] rounded-lg border border-indigo-500/40 space-y-2">
              <div className="flex justify-between items-center text-xs text-[#F9FAFB]">
                <span className="font-semibold text-indigo-400 flex items-center space-x-1.5">
                  <Film className="w-3.5 h-3.5 animate-pulse" />
                  <span>Hardware-Accelerated Transcoding (Windows MFT)...</span>
                </span>
                <span className="font-mono">{exportProgress}%</span>
              </div>
              <div className="w-full h-2 bg-[#141721] rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 transition-all duration-150"
                  style={{ width: `${exportProgress}%` }}
                />
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-5 border-t border-[#252B3B] bg-[#0B0D13]/60 flex items-center justify-between">
          <div className="flex items-center space-x-1.5 text-xs text-emerald-400">
            <CheckCircle2 className="w-4 h-4" />
            <span>Hardware CFR 60 FPS • Real GPU Transcode</span>
          </div>

          <div className="flex items-center space-x-3">
            <button
              onClick={handleModalClose}
              disabled={isExporting}
              className="px-4 py-2 rounded-lg text-xs font-medium text-[#94A3B8] hover:text-[#F9FAFB] hover:bg-[#252B3B] transition-colors disabled:opacity-40"
            >
              {savedDestination ? "Done" : "Cancel"}
            </button>
            <button
              onClick={handleExport}
              disabled={isExporting || !currentVideoPath}
              className="flex items-center space-x-2 px-5 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs shadow-lg shadow-indigo-600/30 transition-all cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Download className="w-4 h-4" />
              <span>{isExporting ? "Transcoding..." : savedDestination ? "Export Again" : "Export Rendered Video"}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
