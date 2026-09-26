use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};

#[derive(Clone, Debug)]
pub struct MonitorDpiInfo {
    pub hmonitor: isize,
    pub bounds: RECT,
    pub dpi_x: u32,
    pub dpi_y: u32,
    pub scale_factor: f32,
}

pub struct DpiCoordinateNormalizer;

impl DpiCoordinateNormalizer {
    /// Initialize process to Per-Monitor V2 DPI awareness
    pub fn init_per_monitor_v2() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    /// Retrieve monitor and DPI configuration for given cursor coordinates
    pub fn get_monitor_for_point(raw_x: i32, raw_y: i32) -> MonitorDpiInfo {
        unsafe {
            let pt = POINT { x: raw_x, y: raw_y };
            let hmonitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);

            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            };

            let bounds = if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
                mi.rcMonitor
            } else {
                RECT {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                }
            };

            let mut dpi_x: u32 = 96;
            let mut dpi_y: u32 = 96;
            let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

            let scale_factor = (dpi_x as f32 / 96.0).max(1.0);

            MonitorDpiInfo {
                hmonitor: hmonitor.0 as isize,
                bounds,
                dpi_x,
                dpi_y,
                scale_factor,
            }
        }
    }

    /// Normalize raw desktop mouse coordinates into [0.0, 1.0] relative to target monitor bounds
    pub fn normalize_to_monitor(raw_x: i32, raw_y: i32, monitor_bounds: &RECT) -> (f32, f32) {
        let width = (monitor_bounds.right - monitor_bounds.left).max(1) as f32;
        let height = (monitor_bounds.bottom - monitor_bounds.top).max(1) as f32;

        let rel_x = (raw_x - monitor_bounds.left) as f32;
        let rel_y = (raw_y - monitor_bounds.top) as f32;

        let norm_x = (rel_x / width).clamp(0.0, 1.0);
        let norm_y = (rel_y / height).clamp(0.0, 1.0);

        (norm_x, norm_y)
    }
}
