#version 450

// ── Inputs from vertex shader ─────────────────────────────────────────────────
layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in mat3 v_tbn;

// ── Output ────────────────────────────────────────────────────────────────────
layout(location = 0) out vec4 out_color;

// ── Uniform buffer (set 0, binding 0) ─────────────────────────────────────────
struct PointLight {
    vec4 position_radius; // xyz = position, w = radius
    vec4 color_intensity; // xyz = color,    w = intensity
};

struct DirLight {
    vec4 direction;       // xyz = direction (normalised toward light)
    vec4 color_intensity; // xyz = color,    w = intensity
};

layout(set = 0, binding = 0) uniform SceneUBO {
    vec4  camera_pos;         // xyz = camera world position
    vec4  ambient;            // xyz = color, w = intensity
    DirLight  dir_lights[4];
    PointLight point_lights[16];
    int   num_dir_lights;
    int   num_point_lights;
    float exposure;
    float gamma;
} scene;

// ── Textures ──────────────────────────────────────────────────────────────────
layout(set = 0, binding = 1) uniform sampler2D t_albedo;
layout(set = 0, binding = 2) uniform sampler2D t_normal;
layout(set = 0, binding = 3) uniform sampler2D t_metallic_roughness;

// ── Material push constants live in vertex PC; fragment uses a sub-struct ─────
// (Keeping it simple: material constants baked per draw into the same PC block)

// ── PBR helpers ───────────────────────────────────────────────────────────────
const float PI = 3.14159265358979;

// GGX Distribution
float D_GGX(float NdotH, float roughness) {
    float a  = roughness * roughness;
    float a2 = a * a;
    float d  = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Smith Geometry
float G_Smith(float NdotV, float NdotL, float roughness) {
    float k = (roughness + 1.0);
    k = k * k / 8.0;
    float gv = NdotV / (NdotV * (1.0 - k) + k);
    float gl = NdotL / (NdotL * (1.0 - k) + k);
    return gv * gl;
}

// Fresnel (Schlick)
vec3 F_Schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Cook-Torrance BRDF for one light
vec3 brdf(vec3 N, vec3 V, vec3 L, vec3 albedo, float metallic, float roughness) {
    vec3 H     = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.001);
    float NdotH = max(dot(N, H), 0.0);
    float VdotH = max(dot(V, H), 0.0);

    vec3 F0 = mix(vec3(0.04), albedo, metallic);

    float D = D_GGX(NdotH, roughness);
    float G = G_Smith(NdotV, NdotL, roughness);
    vec3  F = F_Schlick(VdotH, F0);

    vec3 specular = (D * G * F) / (4.0 * NdotV * NdotL + 0.001);
    vec3 kD = (1.0 - F) * (1.0 - metallic);
    return (kD * albedo / PI + specular) * NdotL;
}

// Reinhard tone mapping
vec3 tone_map(vec3 color, float exposure, float gamma) {
    color *= exposure;
    color = color / (color + vec3(1.0));           // Reinhard
    color = pow(color, vec3(1.0 / gamma));          // gamma correct
    return color;
}

// ── Main ──────────────────────────────────────────────────────────────────────
void main() {
    // Sample textures
    vec4 albedo_tex  = texture(t_albedo, v_uv);
    vec3 albedo      = pow(albedo_tex.rgb, vec3(2.2)); // sRGB → linear
    float alpha      = albedo_tex.a;

    vec3 mr          = texture(t_metallic_roughness, v_uv).rgb;
    float metallic   = mr.b;
    float roughness  = max(mr.g, 0.04);

    // Normal map
    vec3 n_sample    = texture(t_normal, v_uv).xyz * 2.0 - 1.0;
    vec3 N           = normalize(v_tbn * n_sample);

    vec3 V = normalize(scene.camera_pos.xyz - v_world_pos);

    // Ambient
    vec3 Lo = scene.ambient.xyz * scene.ambient.w * albedo;

    // Directional lights
    for (int i = 0; i < scene.num_dir_lights; i++) {
        vec3 L    = normalize(scene.dir_lights[i].direction.xyz);
        vec3 col  = scene.dir_lights[i].color_intensity.xyz;
        float I   = scene.dir_lights[i].color_intensity.w;
        Lo += brdf(N, V, L, albedo, metallic, roughness) * col * I;
    }

    // Point lights
    for (int i = 0; i < scene.num_point_lights; i++) {
        vec3 lpos   = scene.point_lights[i].position_radius.xyz;
        float radius = scene.point_lights[i].position_radius.w;
        vec3 col    = scene.point_lights[i].color_intensity.xyz;
        float I     = scene.point_lights[i].color_intensity.w;

        vec3  L     = lpos - v_world_pos;
        float dist  = length(L);
        L = normalize(L);

        // Smooth window attenuation (Epic Games formula)
        float atten = pow(clamp(1.0 - pow(dist / radius, 4.0), 0.0, 1.0), 2.0)
                    / (dist * dist + 1.0);

        Lo += brdf(N, V, L, albedo, metallic, roughness) * col * I * atten;
    }

    // Tone map + gamma
    vec3 result = tone_map(Lo, scene.exposure, scene.gamma);

    out_color = vec4(result, alpha);
}
