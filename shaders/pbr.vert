#version 450

// ── Vertex input ─────────────────────────────────────────────────────────────
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec4 a_tangent;

// ── Push constants (model + view-projection) ─────────────────────────────────
layout(push_constant) uniform PushConstants {
    mat4 model;
    mat4 view_proj;
} pc;

// ── Outputs to fragment shader ────────────────────────────────────────────────
layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out mat3 v_tbn;

void main() {
    vec4 world = pc.model * vec4(a_position, 1.0);
    v_world_pos = world.xyz;
    gl_Position = pc.view_proj * world;

    // Normal in world space (assumes uniform scale; use inverse transpose for non-uniform)
    mat3 m3 = mat3(pc.model);
    v_normal = normalize(m3 * a_normal);

    v_uv = a_uv;

    // TBN matrix for normal mapping
    vec3 T = normalize(m3 * a_tangent.xyz);
    vec3 N = v_normal;
    T = normalize(T - dot(T, N) * N); // re-orthogonalise
    vec3 B = cross(N, T) * a_tangent.w;
    v_tbn = mat3(T, B, N);
}
