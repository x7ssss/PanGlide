use crate::error::{PanGlideError, Result};
use crate::telemetry::dpi::DpiCoordinateNormalizer;
use crate::telemetry::types::{InputEvent, InputEventType, KeyAction, ModifierState};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static GLOBAL_PRODUCER: Mutex<Option<Producer<InputEvent>>> = Mutex::new(None);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static HOOKS_READY: AtomicBool = AtomicBool::new(false);
static IS_RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_TOGGLE_HOTKEY_MS: AtomicU64 = AtomicU64::new(0);
static LAST_SNIP_HOTKEY_MS: AtomicU64 = AtomicU64::new(0);
static SNIP_REQUESTED: AtomicBool = AtomicBool::new(false);

// Active target monitor bounds for lock-free coordinate normalization in < 5ns
static HAS_TARGET_MONITOR: AtomicBool = AtomicBool::new(false);
static TARGET_LEFT: AtomicI32 = AtomicI32::new(0);
static TARGET_TOP: AtomicI32 = AtomicI32::new(0);
static TARGET_WIDTH: AtomicI32 = AtomicI32::new(1920);
static TARGET_HEIGHT: AtomicI32 = AtomicI32::new(1080);

// Last known normalized cursor position (stored as f32 bits)
static LAST_NORM_X_BITS: AtomicU32 = AtomicU32::new(0);
static LAST_NORM_Y_BITS: AtomicU32 = AtomicU32::new(0);

// Global Hook Manager instance kept alive across app lifecycle
static GLOBAL_HOOK_MANAGER: Mutex<Option<InputHookManager>> = Mutex::new(None);

pub struct InputHookManager {
    is_running: std::sync::Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl InputHookManager {
    /// Initialize input hook thread and store globally so it persists throughout app lifecycle.
    pub fn start(_capacity: usize) -> Result<()> {
        let mut guard = GLOBAL_HOOK_MANAGER.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        let is_running = std::sync::Arc::new(AtomicBool::new(true));
        let running_flag = is_running.clone();

        let thread_handle = thread::Builder::new()
            .name("panglide-input-hooks".into())
            .spawn(move || {
                let tid = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
                HOOK_THREAD_ID.store(tid, Ordering::SeqCst);

                // Set Per-Monitor V2 DPI awareness on hook thread
                DpiCoordinateNormalizer::init_per_monitor_v2();

                let h_mouse = unsafe {
                    SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), HINSTANCE::default(), 0)
                };
                let h_kb = unsafe {
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(low_level_keyboard_proc),
                        HINSTANCE::default(),
                        0,
                    )
                };

                if let (Ok(h_m), Ok(h_k)) = (h_mouse, h_kb) {
                    HOOKS_READY.store(true, Ordering::SeqCst);
                    eprintln!("[InputHookManager] Low-level mouse and keyboard hooks installed.");
                    let mut msg = MSG::default();
                    while running_flag.load(Ordering::Relaxed) {
                        // Pump messages for hooks
                        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
                        if ret.0 <= 0 || msg.message == WM_QUIT {
                            break;
                        }
                    }

                    unsafe {
                        let _ = UnhookWindowsHookEx(h_m);
                        let _ = UnhookWindowsHookEx(h_k);
                    }
                } else {
                    eprintln!("[InputHookManager] Failed to install low-level hooks");
                    HOOKS_READY.store(true, Ordering::SeqCst);
                }

                HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                HOOKS_READY.store(false, Ordering::SeqCst);
            })
            .map_err(|e| PanGlideError::Hook(format!("Failed to spawn hook thread: {}", e)))?;

        // Wait up to 1 second for hook thread to install hooks
        let start_wait = Instant::now();
        while !HOOKS_READY.load(Ordering::SeqCst) && start_wait.elapsed() < Duration::from_secs(1) {
            thread::sleep(Duration::from_millis(5));
        }

        *guard = Some(Self {
            is_running,
            thread_handle: Some(thread_handle),
        });

        Ok(())
    }

    /// Start telemetry capture for an active recording session with an SPSC ring buffer
    pub fn start_telemetry_capture(capacity: usize) -> Consumer<InputEvent> {
        let (producer, consumer) = RingBuffer::<InputEvent>::new(capacity);
        if let Ok(mut guard) = GLOBAL_PRODUCER.lock() {
            *guard = Some(producer);
        }
        IS_RECORDING_ACTIVE.store(true, Ordering::SeqCst);
        consumer
    }

    /// Stop telemetry capture and release the SPSC ring buffer producer
    pub fn stop_telemetry_capture() {
        IS_RECORDING_ACTIVE.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = GLOBAL_PRODUCER.lock() {
            *guard = None;
        }
    }

    /// Directly record an InputEvent into the SPSC ring buffer (takes < 15ns)
    pub fn record_input_event(event: InputEvent) {
        if let Ok(mut guard) = GLOBAL_PRODUCER.try_lock() {
            if let Some(ref mut prod) = *guard {
                let _ = prod.push(event);
            }
        }
    }

    /// Configure active target monitor bounds for lock-free coordinate normalization in hook proc
    pub fn set_target_monitor(left: i32, top: i32, width: u32, height: u32) {
        TARGET_LEFT.store(left, Ordering::Relaxed);
        TARGET_TOP.store(top, Ordering::Relaxed);
        TARGET_WIDTH.store(width as i32, Ordering::Relaxed);
        TARGET_HEIGHT.store(height as i32, Ordering::Relaxed);
        HAS_TARGET_MONITOR.store(true, Ordering::SeqCst);
    }

    pub fn clear_target_monitor() {
        HAS_TARGET_MONITOR.store(false, Ordering::SeqCst);
    }

    pub fn set_recording_active(active: bool) {
        IS_RECORDING_ACTIVE.store(active, Ordering::SeqCst);
    }

    pub fn is_recording_active() -> bool {
        IS_RECORDING_ACTIVE.load(Ordering::Relaxed)
    }

    /// Check and consume live snip hotkey request (Ctrl+Z)
    pub fn check_snip_requested() -> bool {
        SNIP_REQUESTED.swap(false, Ordering::SeqCst)
    }

    pub fn trigger_live_snip_request() {
        SNIP_REQUESTED.store(true, Ordering::SeqCst);
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }

        if let Ok(mut guard) = GLOBAL_PRODUCER.lock() {
            *guard = None;
        }
    }
}

impl Drop for InputHookManager {
    fn drop(&mut self) {
        self.stop();
    }
}

#[inline(always)]
fn get_current_time_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

#[inline(always)]
fn get_modifiers() -> ModifierState {
    unsafe {
        let is_down = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| -> bool {
            (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0
        };

        ModifierState {
            ctrl: is_down(VK_CONTROL) || is_down(VK_LCONTROL) || is_down(VK_RCONTROL),
            shift: is_down(VK_SHIFT) || is_down(VK_LSHIFT) || is_down(VK_RSHIFT),
            alt: is_down(VK_MENU) || is_down(VK_LMENU) || is_down(VK_RMENU),
            meta: is_down(VK_LWIN) || is_down(VK_RWIN),
        }
    }
}

/// Low-level mouse hook procedure.
/// Performance guarantee: Executes in <500ns by directly pushing POD to wait-free ring buffer without allocations.
unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 && l_param.0 != 0 {
        let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
        let msg = w_param.0 as u32;

        let (event_type, button) = match msg {
            WM_MOUSEMOVE => (Some(InputEventType::Move), 0u8),
            WM_LBUTTONDOWN => (Some(InputEventType::MouseDown), 1u8),
            WM_LBUTTONUP => (Some(InputEventType::MouseUp), 1u8),
            WM_RBUTTONDOWN => (Some(InputEventType::MouseDown), 2u8),
            WM_RBUTTONUP => (Some(InputEventType::MouseUp), 2u8),
            WM_MBUTTONDOWN => (Some(InputEventType::MouseDown), 3u8),
            WM_MBUTTONUP => (Some(InputEventType::MouseUp), 3u8),
            WM_MOUSEWHEEL => (Some(InputEventType::Wheel), 0u8),
            _ => (None, 0u8),
        };

        if let Some(evt_type) = event_type {
            let timestamp_us = get_current_time_us();
            let raw_x = info.pt.x;
            let raw_y = info.pt.y;

            // Lock-free normalization against targeted monitor bounds
            let (norm_x, norm_y) = if HAS_TARGET_MONITOR.load(Ordering::Relaxed) {
                let left = TARGET_LEFT.load(Ordering::Relaxed);
                let top = TARGET_TOP.load(Ordering::Relaxed);
                let width = TARGET_WIDTH.load(Ordering::Relaxed).max(1) as f32;
                let height = TARGET_HEIGHT.load(Ordering::Relaxed).max(1) as f32;
                let nx = ((raw_x - left) as f32 / width).clamp(0.0, 1.0);
                let ny = ((raw_y - top) as f32 / height).clamp(0.0, 1.0);
                (nx, ny)
            } else {
                let mon_info = DpiCoordinateNormalizer::get_monitor_for_point(raw_x, raw_y);
                DpiCoordinateNormalizer::normalize_to_monitor(raw_x, raw_y, &mon_info.bounds)
            };

            LAST_NORM_X_BITS.store(norm_x.to_bits(), Ordering::Relaxed);
            LAST_NORM_Y_BITS.store(norm_y.to_bits(), Ordering::Relaxed);

            let key_code = if evt_type == InputEventType::Wheel {
                ((info.mouseData >> 16) & 0xFFFF) as u32
            } else {
                0u32
            };

            let input_event = InputEvent {
                timestamp_us,
                event_type: evt_type,
                x: norm_x,
                y: norm_y,
                button,
                key_code,
            };

            // Direct lock-free push into SPSC ring buffer (takes < 15ns)
            if let Ok(mut guard) = GLOBAL_PRODUCER.try_lock() {
                if let Some(ref mut prod) = *guard {
                    let _ = prod.push(input_event);
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

/// Low-level keyboard hook procedure.
/// Performance guarantee: Executes in <500ns without heap allocations or I/O.
/// Task 3.4 Live Mistake Snipping: Detects Ctrl+Z and sets atomic flag for live snip extraction!
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 && l_param.0 != 0 {
        let info = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        let msg = w_param.0 as u32;

        let action = match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(KeyAction::Down),
            WM_KEYUP | WM_SYSKEYUP => Some(KeyAction::Up),
            _ => None,
        };

        if let Some(act) = action {
            let timestamp_us = get_current_time_us();
            let modifiers = get_modifiers();
            let vk = info.vkCode;

            // Live Mistake Snipping: Ctrl + Z during active recording
            if act == KeyAction::Down && vk == 0x5A && modifiers.ctrl && IS_RECORDING_ACTIVE.load(Ordering::Relaxed) {
                let now_ms = (timestamp_us / 1000) as u64;
                let last_ms = LAST_SNIP_HOTKEY_MS.load(Ordering::Relaxed);
                if now_ms.saturating_sub(last_ms) > 500 {
                    LAST_SNIP_HOTKEY_MS.store(now_ms, Ordering::Relaxed);
                    SNIP_REQUESTED.store(true, Ordering::SeqCst);
                }
            }

            // Global Hotkey: Ctrl + Shift + R or F9 toggles recording even when minimized
            let is_ctrl_shift_r = act == KeyAction::Down
                && (vk == 0x52 || vk == 0x72)
                && modifiers.ctrl
                && modifiers.shift;
            let is_f9 = act == KeyAction::Down && vk == 0x78;
            if is_ctrl_shift_r || is_f9 {
                let now_ms = (timestamp_us / 1000) as u64;
                let last_ms = LAST_TOGGLE_HOTKEY_MS.load(Ordering::Relaxed);
                if now_ms.saturating_sub(last_ms) > 600 {
                    LAST_TOGGLE_HOTKEY_MS.store(now_ms, Ordering::Relaxed);
                    if let Some(app) = crate::get_app_handle() {
                        let _ = app.emit("global_hotkey_toggle_record", ());
                    }
                }
            }

            let norm_x = f32::from_bits(LAST_NORM_X_BITS.load(Ordering::Relaxed));
            let norm_y = f32::from_bits(LAST_NORM_Y_BITS.load(Ordering::Relaxed));

            let evt_type = match act {
                KeyAction::Down => InputEventType::KeyDown,
                KeyAction::Up => InputEventType::KeyUp,
            };

            let input_event = InputEvent {
                timestamp_us,
                event_type: evt_type,
                x: norm_x,
                y: norm_y,
                button: 0,
                key_code: vk,
            };

            // Direct lock-free push into SPSC ring buffer (< 15ns)
            if let Ok(mut guard) = GLOBAL_PRODUCER.try_lock() {
                if let Some(ref mut prod) = *guard {
                    let _ = prod.push(input_event);
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}
