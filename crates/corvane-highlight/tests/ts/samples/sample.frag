#version 330 core
// Blinn-Phong fragment shader with distance fog.

#define MAX_LIGHTS 4

struct Light {
    vec3 position;
    vec3 color;
    float intensity;
};

in vec3 vNormal;
in vec3 vWorldPos;
in vec2 vUv;

uniform sampler2D uAlbedo;
uniform Light uLights[MAX_LIGHTS];
uniform int uLightCount;
uniform vec3 uCameraPos;
uniform bool uFog;

layout(location = 0) out vec4 fragColor;
const float SHININESS = 32.0;

/* Contribution of a single light. */
vec3 shade(in Light light, vec3 n, vec3 v, vec3 albedo) {
    vec3 l = normalize(light.position - vWorldPos);
    vec3 h = normalize(l + v);
    float diff = max(dot(n, l), 0.0);
    float spec = pow(max(dot(n, h), 0.0), SHININESS);
    return (albedo * diff + vec3(spec)) * light.color * light.intensity;
}

void main() {
    vec3 n = normalize(vNormal);
    vec3 v = normalize(uCameraPos - vWorldPos);
    vec3 albedo = texture(uAlbedo, vUv).rgb;
    vec3 color = albedo * 0.05;
    for (int i = 0; i < MAX_LIGHTS && i < uLightCount; ++i) {
        color += shade(uLights[i], n, v, albedo);
    }
    float fog = uFog ? clamp(length(uCameraPos - vWorldPos) / 50.0, 0.0, 1.0) : 0.0;
    fragColor = vec4(mix(color, vec3(0.6, 0.7, 0.8), fog), 1.0);
}
