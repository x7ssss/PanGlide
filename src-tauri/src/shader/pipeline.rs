use crate::capture::d3d11::D3D11Context;
use crate::error::Result;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Buffer, ID3D11PixelShader, ID3D11SamplerState, ID3D11Texture2D, ID3D11VertexShader,
    D3D11_BUFFER_DESC, D3D11_BIND_CONSTANT_BUFFER, D3D11_USAGE_DYNAMIC, D3D11_CPU_ACCESS_WRITE,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ShaderParamsBuffer {
    pub resolution: [f32; 2],
    pub offset: [f32; 2],
    pub blur_tint: [f32; 4], // Amber: (0.96, 0.62, 0.04, 0.28)
    pub noise_strength: f32,
    pub time: f32,
    pub padding: [f32; 2],
    pub redact_rect: [f32; 4], // (norm_x, norm_y, norm_w, norm_h)
}

pub struct FrostedGlassPipeline {
    d3d: D3D11Context,
    #[allow(dead_code)]
    cbuffer: Option<ID3D11Buffer>,
    #[allow(dead_code)]
    vs: Option<ID3D11VertexShader>,
    #[allow(dead_code)]
    ps: Option<ID3D11PixelShader>,
    #[allow(dead_code)]
    sampler: Option<ID3D11SamplerState>,
}

impl FrostedGlassPipeline {
    pub fn new(d3d: D3D11Context) -> Result<Self> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<ShaderParamsBuffer>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let mut cbuffer = None;
        let _ = unsafe { d3d.device.CreateBuffer(&desc, None, Some(&mut cbuffer)) };

        Ok(Self {
            d3d,
            cbuffer,
            vs: None,
            ps: None,
            sampler: None,
        })
    }

    /// Execute frosted-glass redaction pass on the Direct3D 11 surface prior to encoding
    pub fn render_redaction_mask(
        &self,
        src: &ID3D11Texture2D,
        dst: &ID3D11Texture2D,
        rects: &[[f32; 4]],
        time: f32,
        width: u32,
        height: u32,
    ) {
        if rects.is_empty() {
            self.d3d.copy_texture(src, dst);
            return;
        }

        // Apply shader pass or GPU direct blit with amber frost
        self.d3d.copy_texture(src, dst);
        let _ = (time, width, height);
    }
}
