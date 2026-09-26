import { useState } from "react";
import {
  X,
  Key,
  ShieldCheck,
  Cpu,
  Check,
  Copy,
  Zap,
  Lock,
  Loader2,
  AlertCircle,
  CheckCircle2,
} from "lucide-react";
import type { LicenseStatus } from "../types";

interface LicenseModalProps {
  isOpen: boolean;
  onClose: () => void;
  status: LicenseStatus;
  onActivate: (key: string) => Promise<boolean>;
  onDeactivate: () => void;
}

export default function LicenseModal({
  isOpen,
  onClose,
  status,
  onActivate,
  onDeactivate,
}: LicenseModalProps) {
  const [inputKey, setInputKey] = useState(status.licenseKey || "");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [copiedHwid, setCopiedHwid] = useState(false);
  const [feedback, setFeedback] = useState<{ message: string; type: "success" | "error" } | null>(null);

  if (!isOpen) return null;

  const handleActivate = async () => {
    if (!inputKey.trim()) return;
    setIsSubmitting(true);
    setFeedback(null);
    try {
      const success = await onActivate(inputKey.trim());
      if (success) {
        setFeedback({
          message: "License activated successfully! Stored in Windows DPAPI vault.",
          type: "success",
        });
      } else {
        setFeedback({
          message: "Invalid license key. Please check your Lemon Squeezy receipt.",
          type: "error",
        });
      }
    } catch (err: unknown) {
      setFeedback({
        message: err instanceof Error ? err.message : String(err) || "Activation failed. Please check network connectivity.",
        type: "error",
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  const copyHwid = () => {
    navigator.clipboard.writeText(status.hardwareFingerprint);
    setCopiedHwid(true);
    setTimeout(() => setCopiedHwid(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm select-none p-4">
      <div className="w-full max-w-md bg-[#141721] border border-[#252B3B] rounded-2xl shadow-2xl overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-[#252B3B]">
          <div className="flex items-center space-x-2.5">
            <div className="p-2 rounded-lg bg-amber-500/20 text-amber-400 border border-amber-500/30">
              <Key className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-[#F9FAFB]">PanGlide Licensing</h2>
              <p className="text-xs text-[#94A3B8]">Offline-first perpetual license management</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-[#94A3B8] hover:text-[#F9FAFB] hover:bg-[#252B3B] transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Body */}
        <div className="p-5 space-y-4 text-xs text-[#94A3B8]">
          {/* Status Badge */}
          <div
            className={`p-3.5 rounded-xl border flex items-start space-x-3 ${
              status.isActivated
                ? "bg-emerald-950/30 border-emerald-500/40 text-emerald-200"
                : "bg-amber-950/30 border-amber-500/40 text-amber-200"
            }`}
          >
            <div className="p-1 rounded bg-black/30 mt-0.5">
              {status.isActivated ? (
                <ShieldCheck className="w-4 h-4 text-emerald-400" />
              ) : (
                <Lock className="w-4 h-4 text-amber-400" />
              )}
            </div>
            <div className="flex-1">
              <div className="font-semibold text-xs text-white">
                {status.isActivated ? "Perpetual License Active" : "Unlicensed Evaluation Mode"}
              </div>
              <div className="text-[11px] mt-0.5 opacity-80">
                {status.isActivated
                  ? `Verified locally in ${status.verificationLatencyMs.toFixed(2)}ms with zero network calls.`
                  : "Activate your $49 Lemon Squeezy license key to remove evaluation watermarks."}
              </div>
            </div>
          </div>

          {/* Machine HWID (Fingerprint) */}
          <div className="space-y-1.5">
            <div className="flex justify-between items-center text-xs text-[#94A3B8]">
              <span className="flex items-center space-x-1.5">
                <Cpu className="w-3.5 h-3.5 text-indigo-400" />
                <span>Hardware Fingerprint (HWID)</span>
              </span>
              <button
                onClick={copyHwid}
                className="flex items-center space-x-1 text-[11px] text-indigo-400 hover:text-indigo-300 transition-colors"
              >
                {copiedHwid ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                <span>{copiedHwid ? "Copied" : "Copy"}</span>
              </button>
            </div>
            <div className="p-2.5 bg-[#0B0D13] border border-[#252B3B] rounded-lg font-mono text-[11px] text-[#F9FAFB] break-all select-all">
              {status.hardwareFingerprint}
            </div>
            <p className="text-[10px] text-[#94A3B8]/70">
              SHA-256 of SMBIOS UUID + CPUID + Drive0 serial. Stored in Windows DPAPI vault.
            </p>
          </div>

          {/* License Key Input */}
          <div className="space-y-1.5 pt-1">
            <label className="text-xs font-semibold text-[#F9FAFB]">Lemon Squeezy License Key</label>
            <input
              type="text"
              placeholder="e.g. 7A8B-9C0D-1E2F-3A4B-5C6D"
              value={inputKey}
              onChange={(e) => setInputKey(e.target.value)}
              disabled={status.isActivated}
              className="w-full px-3 py-2 bg-[#0B0D13] border border-[#252B3B] rounded-lg text-xs font-mono text-[#F9FAFB] placeholder-[#94A3B8]/50 focus:outline-none focus:border-indigo-500 transition-colors"
            />
          </div>

          {/* Status Feedback Message */}
          {feedback && (
            <div
              className={`p-3 rounded-lg border flex items-center space-x-2 text-xs transition-all ${
                feedback.type === "success"
                  ? "bg-emerald-950/40 border-emerald-500/50 text-emerald-300"
                  : "bg-rose-950/40 border-rose-500/50 text-rose-300"
              }`}
            >
              {feedback.type === "success" ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
              ) : (
                <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
              )}
              <span>{feedback.message}</span>
            </div>
          )}

          {/* Features Checklist */}
          <div className="p-3 bg-[#0B0D13]/60 rounded-lg border border-[#252B3B] space-y-1.5 text-[11px]">
            <div className="flex items-center space-x-2 text-[#F9FAFB]">
              <Zap className="w-3.5 h-3.5 text-amber-400" />
              <span>Offline boot verification in &lt;1ms</span>
            </div>
            <div className="flex items-center space-x-2 text-[#F9FAFB]">
              <Zap className="w-3.5 h-3.5 text-amber-400" />
              <span>Zero cloud accounts, zero telemetry</span>
            </div>
            <div className="flex items-center space-x-2 text-[#F9FAFB]">
              <Zap className="w-3.5 h-3.5 text-amber-400" />
              <span>Windows DPAPI (CryptProtectData) encrypted vault</span>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-5 border-t border-[#252B3B] bg-[#0B0D13]/60 flex items-center justify-between">
          {status.isActivated ? (
            <button
              onClick={onDeactivate}
              className="text-xs text-rose-400 hover:text-rose-300 hover:underline transition-colors"
            >
              Deactivate on this PC
            </button>
          ) : (
            <span className="text-[11px] text-[#94A3B8]">Perpetual lifetime license</span>
          )}

          <div className="flex items-center space-x-3">
            <button
              onClick={onClose}
              className="px-4 py-2 rounded-lg text-xs font-medium text-[#94A3B8] hover:text-[#F9FAFB] hover:bg-[#252B3B] transition-colors"
            >
              Close
            </button>
            {!status.isActivated && (
              <button
                onClick={handleActivate}
                disabled={isSubmitting || !inputKey.trim()}
                className="px-5 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs shadow-lg shadow-indigo-600/30 transition-all cursor-pointer disabled:opacity-50 flex items-center space-x-1.5"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    <span>Verifying...</span>
                  </>
                ) : (
                  <span>Activate License</span>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
