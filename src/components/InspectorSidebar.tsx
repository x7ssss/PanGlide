import {
  ZoomIn,
  Sliders,
  Shield,
  Layers,
  Sparkles,
  MousePointer2,
  Scissors,
} from "lucide-react";
import type { AspectRatioPreset } from "../types";

interface InspectorSidebarProps {
  zoomScale: number;
  onZoomChange: (val: number) => void;
  aspectRatio: AspectRatioPreset;
  onAspectRatioChange: (val: AspectRatioPreset) => void;
  cornerRadius: number;
  onCornerRadiusChange: (val: number) => void;
  dropShadowSpread: number;
  onDropShadowChange: (val: number) => void;
  backdropId: string;
  onBackdropChange: (val: string) => void;
  showFocusReticle: boolean;
  onToggleFocusReticle: () => void;
  autoRedactEnabled: boolean;
  onToggleAutoRedact: () => void;
  springPreset: string;
  onSpringPresetChange: (val: string) => void;
  deadzoneEnabled: boolean;
  onToggleDeadzone: () => void;
  pruneRejectedTakes?: boolean;
  onTogglePruneRejectedTakes?: () => void;
  autoTrackingEnabled?: boolean;
  onToggleAutoTracking?: () => void;
}

export default function InspectorSidebar({
  zoomScale,
  onZoomChange,
  aspectRatio,
  onAspectRatioChange,
  cornerRadius,
  onCornerRadiusChange,
  dropShadowSpread,
  onDropShadowChange,
  backdropId,
  onBackdropChange,
  showFocusReticle,
  onToggleFocusReticle,
  autoRedactEnabled,
  onToggleAutoRedact,
  springPreset,
  onSpringPresetChange,
  deadzoneEnabled,
  onToggleDeadzone,
  pruneRejectedTakes = true,
  onTogglePruneRejectedTakes,
  autoTrackingEnabled = true,
  onToggleAutoTracking,
}: InspectorSidebarProps) {
  const backdrops = [
    { id: "aurora", name: "Aurora" },
    { id: "indigo", name: "Indigo" },
    { id: "cyber", name: "Cyber" },
    { id: "slate", name: "Slate" },
    { id: "transparent", name: "Checker" },
  ];

  return (
    <aside className="w-80 h-full bg-[#141721] border-l border-[#252B3B] flex flex-col overflow-y-auto select-none p-5 space-y-6">
      {/* Sidebar Header */}
      <div className="flex items-center justify-between pb-3 border-b border-[#252B3B]">
        <div className="flex items-center space-x-2">
          <Sliders className="w-4 h-4 text-indigo-400" />
          <h2 className="text-sm font-semibold text-[#F9FAFB] tracking-wide">Studio Inspector</h2>
        </div>
        <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-[#0B0D13] text-[#94A3B8] border border-[#252B3B]">
          v1.0 Core
        </span>
      </div>

      {/* 1. Kinematic Zoom Control */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-1.5 text-xs font-medium text-[#F9FAFB]">
            <ZoomIn className="w-3.5 h-3.5 text-indigo-400" />
            <span>Magnification Scale</span>
          </div>
          <span className="font-mono text-xs text-indigo-400 bg-[#0B0D13] px-2 py-0.5 rounded border border-[#252B3B]">
            {zoomScale.toFixed(2)}x
          </span>
        </div>

        <input
          type="range"
          min="1.0"
          max="2.2"
          step="0.05"
          value={zoomScale}
          onChange={(e) => onZoomChange(parseFloat(e.target.value))}
          className="w-full accent-indigo-500 bg-[#0B0D13] h-1.5 rounded-lg cursor-pointer"
        />

        {/* Quick Zoom Presets */}
        <div className="grid grid-cols-4 gap-1.5">
          {[1.0, 1.4, 1.8, 2.2].map((s) => (
            <button
              key={s}
              onClick={() => onZoomChange(s)}
              className={`py-1 text-[11px] font-mono rounded border transition-colors ${
                Math.abs(zoomScale - s) < 0.05
                  ? "bg-indigo-600/30 border-indigo-500 text-indigo-300 font-semibold"
                  : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:text-[#F9FAFB] hover:border-[#3B4358]"
              }`}
            >
              {s.toFixed(1)}x
            </button>
          ))}
        </div>
      </div>

      <div className="h-[1px] bg-[#252B3B]" />

      {/* 2. Physics & Motion Presets */}
      <div className="space-y-3">
        <div className="flex items-center space-x-1.5 text-xs font-medium text-[#F9FAFB]">
          <Sparkles className="w-3.5 h-3.5 text-indigo-400" />
          <span>Spring Physics & Easing</span>
        </div>

        <div className="space-y-1.5">
          {[
            { id: "snappy", title: "PanGlide Snappy", desc: "Tension: 170, Damping: 26" },
            { id: "cinematic", title: "Cinematic Glide", desc: "Tension: 120, Damping: 22" },
            { id: "subtle", title: "Subtle Gentle", desc: "Tension: 80, Damping: 18" },
          ].map((preset) => (
            <button
              key={preset.id}
              onClick={() => onSpringPresetChange(preset.id)}
              className={`w-full text-left px-3 py-2 rounded border transition-all ${
                springPreset === preset.id
                  ? "bg-indigo-600/20 border-indigo-500 text-[#F9FAFB]"
                  : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:border-[#3B4358]"
              }`}
            >
              <div className="text-xs font-semibold">{preset.title}</div>
              <div className="text-[10px] text-[#94A3B8] font-mono">{preset.desc}</div>
            </button>
          ))}
        </div>

        {/* Spatial Dead-zone Toggle */}
        <label className="flex items-center justify-between p-2.5 rounded bg-[#0B0D13] border border-[#252B3B] cursor-pointer hover:border-[#3B4358] transition-colors">
          <div className="flex items-center space-x-2">
            <MousePointer2 className="w-3.5 h-3.5 text-indigo-400" />
            <div className="text-xs text-[#F9FAFB] font-medium">Velocity Dead-Zone</div>
          </div>
          <input
            type="checkbox"
            checked={deadzoneEnabled}
            onChange={onToggleDeadzone}
            className="w-4 h-4 accent-indigo-500 rounded cursor-pointer"
          />
        </label>
        <p className="text-[10px] text-[#94A3B8] leading-tight">
          Suppresses twitching by ignoring sub-100px cursor micro-jitters within 200ms.
        </p>

        {/* Auto-Cut Snipped Mistakes Toggle */}
        <label className="flex items-center justify-between p-2.5 rounded bg-[#0B0D13] border border-[#252B3B] cursor-pointer hover:border-[#3B4358] transition-colors">
          <div className="flex items-center space-x-2">
            <Scissors className="w-3.5 h-3.5 text-rose-400" />
            <div className="text-xs text-[#F9FAFB] font-medium">Auto-Cut Snipped Mistakes</div>
          </div>
          <input
            type="checkbox"
            checked={pruneRejectedTakes}
            onChange={onTogglePruneRejectedTakes}
            className="w-4 h-4 accent-rose-500 rounded cursor-pointer"
          />
        </label>
        <p className="text-[10px] text-[#94A3B8] leading-tight">
          Excises marked 5-second mistake takes (Ctrl+Z) seamlessly during hardware export.
        </p>

        {/* Auto-Tracking Kinematics Toggle */}
        <label className="flex items-center justify-between p-2.5 rounded bg-[#0B0D13] border border-[#252B3B] cursor-pointer hover:border-[#3B4358] transition-colors">
          <div className="flex items-center space-x-2">
            <Sparkles className="w-3.5 h-3.5 text-indigo-400" />
            <div className="text-xs text-[#F9FAFB] font-medium">Auto-Tracking Kinematics</div>
          </div>
          <input
            type="checkbox"
            checked={autoTrackingEnabled}
            onChange={onToggleAutoTracking}
            className="w-4 h-4 accent-indigo-500 rounded cursor-pointer"
          />
        </label>
        <p className="text-[10px] text-[#94A3B8] leading-tight">
          Bakes second-order spring camera panning & click zoom into exported video.
        </p>
      </div>

      <div className="h-[1px] bg-[#252B3B]" />

      {/* 3. Privacy & Redaction Toggles */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-1.5 text-xs font-medium text-[#F9FAFB]">
            <Shield className="w-3.5 h-3.5 text-amber-400" />
            <span>Privacy Auto-Redaction</span>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/30">
            Amber Kawase
          </span>
        </div>

        <label className="flex items-center justify-between p-2.5 rounded bg-[#0B0D13] border border-[#252B3B] cursor-pointer">
          <div className="text-xs text-[#F9FAFB] font-medium">Pre-Encode Token Masking</div>
          <input
            type="checkbox"
            checked={autoRedactEnabled}
            onChange={onToggleAutoRedact}
            className="w-4 h-4 accent-amber-500 rounded cursor-pointer"
          />
        </label>

        {/* Reticle Focus Indicator Toggle */}
        <label className="flex items-center justify-between p-2.5 rounded bg-[#0B0D13] border border-[#252B3B] cursor-pointer">
          <div className="text-xs text-[#F9FAFB] font-medium">Show Focus Reticle</div>
          <input
            type="checkbox"
            checked={showFocusReticle}
            onChange={onToggleFocusReticle}
            className="w-4 h-4 accent-indigo-500 rounded cursor-pointer"
          />
        </label>
      </div>

      <div className="h-[1px] bg-[#252B3B]" />

      {/* 4. Canvas Framing & Backdrops */}
      <div className="space-y-3">
        <div className="flex items-center space-x-1.5 text-xs font-medium text-[#F9FAFB]">
          <Layers className="w-3.5 h-3.5 text-indigo-400" />
          <span>Canvas Framing</span>
        </div>

        {/* Aspect Ratio Switcher */}
        <div className="grid grid-cols-3 gap-1.5">
          {(["16:9", "9:16", "1:1"] as AspectRatioPreset[]).map((ar) => (
            <button
              key={ar}
              onClick={() => onAspectRatioChange(ar)}
              className={`py-1.5 text-xs font-semibold rounded border transition-colors ${
                aspectRatio === ar
                  ? "bg-indigo-600 text-white border-indigo-500"
                  : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:text-[#F9FAFB]"
              }`}
            >
              {ar}
            </button>
          ))}
        </div>

        {/* Corner Radius Slider */}
        <div className="space-y-1.5 pt-1">
          <div className="flex items-center justify-between text-xs text-[#94A3B8]">
            <span>Corner Radius</span>
            <span className="font-mono text-indigo-400">{cornerRadius}px</span>
          </div>
          <input
            type="range"
            min="0"
            max="32"
            value={cornerRadius}
            onChange={(e) => onCornerRadiusChange(parseInt(e.target.value))}
            className="w-full accent-indigo-500 bg-[#0B0D13] h-1.5 rounded-lg cursor-pointer"
          />
        </div>

        {/* Drop Shadow Spread Slider */}
        <div className="space-y-1.5 pt-1">
          <div className="flex items-center justify-between text-xs text-[#94A3B8]">
            <span>Shadow Spread</span>
            <span className="font-mono text-indigo-400">{dropShadowSpread}px</span>
          </div>
          <input
            type="range"
            min="0"
            max="64"
            value={dropShadowSpread}
            onChange={(e) => onDropShadowChange(parseInt(e.target.value))}
            className="w-full accent-indigo-500 bg-[#0B0D13] h-1.5 rounded-lg cursor-pointer"
          />
        </div>

        {/* Backdrop Presets */}
        <div className="space-y-1.5 pt-1">
          <span className="text-xs text-[#94A3B8]">Backdrop Theme</span>
          <div className="grid grid-cols-3 gap-1.5">
            {backdrops.map((b) => (
              <button
                key={b.id}
                onClick={() => onBackdropChange(b.id)}
                className={`py-1 text-[11px] font-medium rounded border transition-colors ${
                  backdropId === b.id
                    ? "bg-indigo-600/30 border-indigo-500 text-indigo-300"
                    : "bg-[#0B0D13] border-[#252B3B] text-[#94A3B8] hover:text-[#F9FAFB]"
                }`}
              >
                {b.name}
              </button>
            ))}
          </div>
        </div>
      </div>
    </aside>
  );
}
