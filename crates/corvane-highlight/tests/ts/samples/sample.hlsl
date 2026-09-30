// Lit pixel shader with a constant buffer, structured buffer and texture.
#define MAX_LIGHTS 4
cbuffer PerFrame : register(b0) {
    float4x4 ViewProj;
    float3 CameraPos;
    float Time;
};

struct PointLight {
    float3 Position;
    float Radius;
    float3 Color;
    uint Enabled;
};

StructuredBuffer<PointLight> Lights : register(t1);
Texture2D<float4> Albedo : register(t0);
SamplerState LinearSampler : register(s0);
struct VSInput { float3 pos : POSITION; float3 normal : NORMAL; float2 uv : TEXCOORD0; };
struct PSInput { float4 pos : SV_Position; float3 world : WORLDPOS; float3 normal : NORMAL; float2 uv : TEXCOORD0; };

PSInput VSMain(VSInput input) {
    PSInput o;
    o.world = input.pos;
    o.pos = mul(float4(input.pos, 1.0f), ViewProj);
    o.normal = normalize(input.normal);
    o.uv = input.uv;
    return o;
}

/* Attenuated diffuse term for one light. */
float3 Shade(PointLight light, float3 world, float3 n) {
    float3 toLight = light.Position - world;
    float atten = saturate(1.0 - length(toLight) / light.Radius);
    return light.Color * max(dot(n, normalize(toLight)), 0.0) * atten;
}

[earlydepthstencil]
float4 PSMain(PSInput input) : SV_Target {
    float3 color = Albedo.Sample(LinearSampler, input.uv).rgb * 0.1;
    [unroll] for (uint i = 0; i < MAX_LIGHTS; ++i) {
        if (Lights[i].Enabled != 0u) color += Shade(Lights[i], input.world, input.normal);
    }
    return float4(color * (0.9 + 0.1 * sin(Time)), 1.0);
}
