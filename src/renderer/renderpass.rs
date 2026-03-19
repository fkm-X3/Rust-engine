/// Settings that control what the renderer draws.
#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub ambient_color: glam::Vec3,
    pub ambient_intensity: f32,
    pub enable_shadows: bool,
    pub enable_bloom: bool,
    pub bloom_threshold: f32,
    pub bloom_intensity: f32,
    pub exposure: f32,
    pub gamma: f32,
    pub wireframe: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            ambient_color: glam::Vec3::ONE,
            ambient_intensity: 0.1,
            enable_shadows: true,
            enable_bloom: false,
            bloom_threshold: 1.0,
            bloom_intensity: 0.5,
            exposure: 1.0,
            gamma: 2.2,
            wireframe: false,
        }
    }
}