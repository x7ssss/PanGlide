use crate::capture::d3d11::D3D11Context;
use crate::capture::wgc::{FrameCallback, WgcFrame};
use crate::error::{PanGlideError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_OUTDUPL_FRAME_INFO,
};

pub struct DxgiDesktopDuplication {
    is_running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl DxgiDesktopDuplication {
    pub fn start(
        d3d: D3D11Context,
        output_index: u32,
        on_frame: FrameCallback,
    ) -> Result<Self> {
        let dxgi_device: IDXGIDevice = d3d.device.cast()?;
        let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };
        let output: IDXGIOutput = unsafe { adapter.EnumOutputs(output_index)? };
        let output1: IDXGIOutput1 = output.cast()?;

        let desc = unsafe { output.GetDesc()? };
        let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).unsigned_abs();
        let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).unsigned_abs();

        let duplication: IDXGIOutputDuplication = unsafe {
            output1.DuplicateOutput(&d3d.device)?
        };

        let is_running = Arc::new(AtomicBool::new(true));
        let running_flag = is_running.clone();

        let thread_handle = thread::Builder::new()
            .name("panglide-dxgi-dup".into())
            .spawn(move || {
                let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();

                while running_flag.load(Ordering::Relaxed) {
                    let mut resource = None;
                    let hr = unsafe {
                        duplication.AcquireNextFrame(20, &mut frame_info, &mut resource)
                    };

                    if hr.is_ok() {
                        if let Some(res) = resource {
                            if let Ok(src_texture) = res.cast::<ID3D11Texture2D>() {
                                if let Ok(dst_texture) = d3d.create_render_texture(
                                    width,
                                    height,
                                    DXGI_FORMAT_B8G8R8A8_UNORM,
                                ) {
                                    d3d.copy_texture(&src_texture, &dst_texture);

                                    let time_100ns = frame_info.LastPresentTime;
                                    let frame = WgcFrame {
                                        texture: dst_texture,
                                        timestamp_100ns: time_100ns,
                                        width,
                                        height,
                                    };
                                    on_frame(frame);
                                }
                            }
                        }
                        let _ = unsafe { duplication.ReleaseFrame() };
                    } else {
                        // Timeout or occluded - sleep briefly before next probe
                        thread::sleep(Duration::from_millis(1));
                    }
                }
            })
            .map_err(|e| PanGlideError::Capture(format!("Failed to spawn DXGI duplication thread: {}", e)))?;

        Ok(Self {
            is_running,
            thread_handle: Some(thread_handle),
        })
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DxgiDesktopDuplication {
    fn drop(&mut self) {
        self.stop();
    }
}
