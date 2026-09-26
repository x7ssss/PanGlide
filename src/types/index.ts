export type AspectRatioPreset = "16:9" | "9:16" | "1:1";
export type ResolutionPreset = "1080p" | "1440p" | "4K";

export interface BackdropPreset {
  id: string;
  name: string;
  className: string;
  style?: React.CSSProperties;
}

export interface ZoomKeyframe {
  id: string;
  timeMs: number;
  scale: number;
  label: string;
}

export interface AutoBlurMarker {
  id: string;
  timeMs: number;
  tokenType: string;
  preview: string;
  bounds: { x: number; y: number; width: number; height: number };
}

export interface RejectedTakeSegment {
  id: string;
  startTimeMs: number;
  endTimeMs: number;
  durationSec: number;
}

export interface RecordingState {
  isRecording: boolean;
  isPaused: boolean;
  elapsedMs: number;
  micVuLevel: number; // 0.0 to 1.0
  sysVuLevel: number; // 0.0 to 1.0
  activeSource: string;
}

export interface LicenseStatus {
  isActivated: boolean;
  licenseKey: string;
  instanceId: string;
  hardwareFingerprint: string;
  verifiedOffline: boolean;
  verificationLatencyMs: number;
  expiryDate: string;
}

export interface CameraFrame {
  timestamp_ms?: number;
  timestampMs?: number;
  x: number;
  y: number;
  zoom: number;
  frameIndex?: number;
  centerX?: number;
  centerY?: number;
}

export interface RecordingResult {
  videoPath: string;
  videoUrl: string;
  durationMs: number;
  frameCount: number;
  width: number;
  height: number;
  cameraKeyframes?: CameraFrame[];
  autoBlurMarkers: AutoBlurMarker[];
  rejectedTakes: RejectedTakeSegment[];
  rejectedTakeIntervals?: RejectedTakeSegment[];
  zoomKeyframes: ZoomKeyframe[];
}

export interface CaptureSource {
  id: string;
  name: string;
  isMonitor: boolean;
  width: number;
  height: number;
  x?: number;
  y?: number;
}
