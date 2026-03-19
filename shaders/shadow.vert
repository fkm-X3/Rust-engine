#version 450
// Shadow map pass — writes depth only, no colour output.

layout(location = 0) in vec3 a_position;

layout(push_constant) uniform PC {
    mat4 light_space; // light's view-projection
    mat4 model;
} pc;

void main() {
    gl_Position = pc.light_space * pc.model * vec4(a_position, 1.0);
}
