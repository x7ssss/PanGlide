use crate::error::{PanGlideError, Result};
use windows::core::Interface;
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;

#[derive(Clone)]
pub struct D3D11Context {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub winrt_device: IDirect3DDevice,
    pub feature_level: D3D_FEATURE_LEVEL,
}

// Safety: ID3D11Device and ID3D11DeviceContext with multi-thread protection can be safely shared
unsafe impl Send for D3D11Context {}
unsafe impl Sync for D3D11Context {}

impl D3D11Context {
    pub fn new() -> Result<Self> {
        let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL_11_0;

        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                flags,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )?;
        }

        let device = device.ok_or_else(|| PanGlideError::D3D11Init("Failed to create D3D11 Device".into()))?;
        let context = context.ok_or_else(|| PanGlideError::D3D11Init("Failed to create D3D11 Context".into()))?;

        // Query IDXGIDevice to create WinRT IDirect3DDevice for WGC interop
        let dxgi_device: IDXGIDevice = device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)? };
        let winrt_device: IDirect3DDevice = inspectable.cast()?;

        Ok(Self {
            device,
            context,
            winrt_device,
            feature_level,
        })
    }

    /// Allocate a high-performance Direct3D 11 surface in GPU VRAM (ZERO CPU RAM roundtrips)
    pub fn create_render_texture(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
        }
        texture.ok_or_else(|| PanGlideError::D3D11Init("Failed to allocate GPU texture".into()))
    }

    /// Direct GPU VRAM copy (0 CPU RAM round-trips)
    pub fn copy_texture(&self, src: &ID3D11Texture2D, dst: &ID3D11Texture2D) {
        unsafe {
            self.context.CopyResource(dst, src);
        }
    }

    /// Direct GPU subregion copy
    pub fn copy_subregion(
        &self,
        src: &ID3D11Texture2D,
        dst: &ID3D11Texture2D,
        dst_x: u32,
        dst_y: u32,
        src_box: Option<&windows::Win32::Graphics::Direct3D11::D3D11_BOX>,
    ) {
        unsafe {
            let p_box = src_box.map(|b| b as *const _);
            self.context
                .CopySubresourceRegion(dst, 0, dst_x, dst_y, 0, src, 0, p_box);
        }
    }

    /// Allocate staging texture for localized OCR reading / dirty-rect capture
    pub fn create_staging_texture(&self, width: u32, height: u32) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
        }
        texture.ok_or_else(|| PanGlideError::D3D11Init("Failed to allocate staging texture".into()))
    }
}
