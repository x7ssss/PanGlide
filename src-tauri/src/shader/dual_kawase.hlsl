// PanGlide Dual Kawase Frosted-Glass Blur & Amber Redaction HLSL Pixel Shader
// Direct3D 11 / Shader Model 5.0

cbuffer ShaderParams : register(b0)
{
    float2 u_resolution;       // Viewport resolution (e.g. 1920x1080)
    float2 u_offset;           // Iteration blur kernel offset
    float4 u_blur_tint;        // Amber frosted tint: (0.96, 0.62, 0.04, 0.28)
    float  u_noise_strength;   // Micro-surface frosted noise intensity (default: 0.0035)
    float  u_time;             // Timestamp for subtle dynamic refraction
    float2 u_padding;
    float4 u_redact_rect;      // (x_norm, y_norm, width_norm, height_norm)
};

Texture2D    g_texture : register(t0);
SamplerState g_sampler : register(s0);

struct VS_Output
{
    float4 position : SV_POSITION;
    float2 uv       : TEXCOORD0;
};

// Procedural pseudo-random hash for frosted acrylic micro-displacement
float hash21(float2 p)
{
    p = frac(p * float2(233.34, 851.73));
    p += dot(p, p + 23.45);
    return frac(p.x * p.y);
}

// Fullscreen quad vertex shader
VS_Output VS_Main(uint vertex_id : SV_VertexID)
{
    VS_Output output;
    output.uv = float2((vertex_id << 1) & 2, vertex_id & 2);
    output.position = float4(output.uv * float2(2.0f, -2.0f) + float2(-1.0f, 1.0f), 0.0f, 1.0f);
    return output;
}

// Dual Kawase Downsample Pass
float4 PS_DualKawaseDown(VS_Output input) : SV_Target
{
    float2 half_pixel = (1.0f / u_resolution) * 0.5f;
    float2 uv = input.uv;

    // Apply micro-noise displacement
    float noise = (hash21(uv * u_resolution + float2(u_time, u_time)) - 0.5f) * u_noise_strength;
    uv += noise;

    float4 sum = g_texture.Sample(g_sampler, uv) * 4.0f;
    sum += g_texture.Sample(g_sampler, uv - half_pixel * u_offset);
    sum += g_texture.Sample(g_sampler, uv + half_pixel * u_offset);
    sum += g_texture.Sample(g_sampler, uv + float2(half_pixel.x, -half_pixel.y) * u_offset);
    sum += g_texture.Sample(g_sampler, uv + float2(-half_pixel.x, half_pixel.y) * u_offset);

    return sum * 0.125f;
}

// Dual Kawase Upsample & Frosted Glass Masking Pass
float4 PS_DualKawaseUpAndComposite(VS_Output input) : SV_Target
{
    float2 uv = input.uv;
    float4 base_color = g_texture.Sample(g_sampler, uv);

    // Check if current UV falls within the amber privacy redaction bounding box
    bool in_redaction = (uv.x >= u_redact_rect.x && uv.x <= (u_redact_rect.x + u_redact_rect.z) &&
                         uv.y >= u_redact_rect.y && uv.y <= (u_redact_rect.y + u_redact_rect.w));

    if (!in_redaction)
    {
        return base_color;
    }

    // Micro-surface frosted noise displacement
    float noise_x = (hash21(uv * 1337.0f) - 0.5f) * u_noise_strength;
    float noise_y = (hash21(uv * 9876.0f) - 0.5f) * u_noise_strength;
    float2 frosted_uv = uv + float2(noise_x, noise_y);

    float2 half_pixel = (1.0f / u_resolution) * 0.5f;

    // 8-tap upsample filter for Dual Kawase
    float4 sum = 0.0f;
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(-half_pixel.x * 2.0f, 0.0f) * u_offset);
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(-half_pixel.x, half_pixel.y) * u_offset) * 2.0f;
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(0.0f, half_pixel.y * 2.0f) * u_offset);
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(half_pixel.x, half_pixel.y) * u_offset) * 2.0f;
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(half_pixel.x * 2.0f, 0.0f) * u_offset);
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(half_pixel.x, -half_pixel.y) * u_offset) * 2.0f;
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(0.0f, -half_pixel.y * 2.0f) * u_offset);
    sum += g_texture.Sample(g_sampler, frosted_uv + float2(-half_pixel.x, -half_pixel.y) * u_offset) * 2.0f;

    float4 blurred = sum * (1.0f / 12.0f);

    // Composite amber frosted-glass tint (#F59E0B)
    float3 amber_tint = u_blur_tint.rgb;
    float tint_alpha = u_blur_tint.a;

    float3 frosted_composite = lerp(blurred.rgb, amber_tint, tint_alpha);

    // Subtle 1px inner border glow on redaction box
    float2 edge_dist = min(uv - u_redact_rect.xy, (u_redact_rect.xy + u_redact_rect.zw) - uv);
    float min_edge = min(edge_dist.x * u_resolution.x, edge_dist.y * u_resolution.y);
    if (min_edge < 1.5f)
    {
        frosted_composite = lerp(frosted_composite, float3(0.96f, 0.62f, 0.04f), 0.75f);
    }

    return float4(frosted_composite, 1.0f);
}
