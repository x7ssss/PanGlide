use crate::capture::d3d11::D3D11Context;
use crate::error::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::Interface;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

pub struct WgcFrame {
    pub texture: ID3D11Texture2D,
    pub timestamp_100ns: i64,
    pub width: u32,
    pub height: u32,
}

pub type FrameCallback = Arc<dyn Fn(WgcFrame) + Send + Sync + 'static>;

pub struct WgcCaptureSession {
    session: GraphicsCaptureSession,
    frame_pool: Direct3D11CaptureFramePool,
    is_active: Arc<AtomicBool>,
}

impl WgcCaptureSession {
    /// Runtime capability check for Windows Graphics Capture (Windows 10 v1903+)
    pub fn is_supported() -> bool {
        GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }

    /// Create capture item targeting a specific window handle (HWND)
    pub fn create_item_for_window(hwnd: HWND) -> Result<GraphicsCaptureItem> {
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item = unsafe { interop.CreateForWindow(hwnd)? };
        Ok(item)
    }

    /// Create capture item targeting a specific monitor handle (HMONITOR)
    pub fn create_item_for_monitor(hmonitor: HMONITOR) -> Result<GraphicsCaptureItem> {
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item = unsafe { interop.CreateForMonitor(hmonitor)? };
        Ok(item)
    }

    /// Start high-performance WGC capture session delivering frames to Direct3D 11 GPU surfaces
    pub fn start(
        d3d: D3D11Context,
        item: GraphicsCaptureItem,
        on_frame: FrameCallback,
    ) -> Result<Self> {
        let size = item.Size()?;
        let width = size.Width as u32;
        let height = size.Height as u32;

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d.winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;

        let session = frame_pool.CreateCaptureSession(&item)?;

        // Disable yellow capture border if supported (Win 10 2004+)
        let _ = session.SetIsBorderRequired(false);
        // Ensure cursor is captured natively in stream
        let _ = session.SetIsCursorCaptureEnabled(true);

        let is_active = Arc::new(AtomicBool::new(true));
        let active_flag = is_active.clone();
        let d3d_clone = d3d.clone();

        frame_pool.FrameArrived(&TypedEventHandler::new(
            move |pool: &Option<Direct3D11CaptureFramePool>, _| {
                if !active_flag.load(Ordering::Relaxed) {
                    return Ok(());
                }

                if let Some(pool) = pool {
                    if let Ok(frame) = pool.TryGetNextFrame() {
                        let time = frame.SystemRelativeTime().map(|t| t.Duration).unwrap_or(0);
                        let surface = match frame.Surface() {
                            Ok(s) => s,
                            Err(_) => return Ok(()),
                        };

                        // Extract underlying Direct3D 11 texture
                        if let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() {
                            let src_texture: Result<ID3D11Texture2D> = unsafe {
                                access.GetInterface().map_err(Into::into)
                            };

                            if let Ok(src) = src_texture {
                                // Allocate or copy directly into GPU VRAM destination
                                if let Ok(dst) = d3d_clone.create_render_texture(
                                    width,
                                    height,
                                    DXGI_FORMAT_B8G8R8A8_UNORM,
                                ) {
                                    d3d_clone.copy_texture(&src, &dst);

                                    let captured = WgcFrame {
                                        texture: dst,
                                        timestamp_100ns: time,
                                        width,
                                        height,
                                    };
                                    on_frame(captured);
                                }
                            }
                        }
                    }
                }
                Ok(())
            },
        ))?;

        session.StartCapture()?;

        Ok(Self {
            session,
            frame_pool,
            is_active,
        })
    }

    pub fn stop(&self) {
        self.is_active.store(false, Ordering::Relaxed);
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
    }
}

impl Drop for WgcCaptureSession {
    fn drop(&mut self) {
        self.stop();
    }
}
