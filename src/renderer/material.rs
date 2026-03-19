use glam::Vec4;

/// PBR material parameters.
#[derive(Debug, Clone)]
pub struct Material {
    /// Base colour (RGBA, linear).
    pub albedo: Vec4,
    /// Metallic factor [0, 1].
    pub metallic: f32,
    /// Roughness factor [0, 1].
    pub roughness: f32,
    /// Emissive colour (RGB).
    pub emissive: glam::Vec3,
    /// Albedo texture handle (None = use solid colour).
    pub albedo_texture: Option<crate::assets::AssetHandle>,
    /// Normal map texture handle.
    pub normal_texture: Option<crate::assets::AssetHandle>,
    /// Metallic-roughness texture handle.
    pub metallic_roughness_texture: Option<crate::assets::AssetHandle>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            albedo: Vec4::new(0.8, 0.8, 0.8, 1.0),
            metallic: 0.0,
            roughness: 0.5,
            emissive: glam::Vec3::ZERO,
            albedo_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
        }
    }
}

impl Material {
    pub fn new(albedo: Vec4, metallic: f32, roughness: f32) -> Self {
        Self { albedo, metallic, roughness, ..Default::default() }
    }

    pub fn metal(color: glam::Vec3, roughness: f32) -> Self {
        Self::new(Vec4::from((color, 1.0)), 1.0, roughness)
    }

    pub fn plastic(color: glam::Vec3) -> Self {
        Self::new(Vec4::from((color, 1.0)), 0.0, 0.5)
    }

    pub fn emissive(color: glam::Vec3, strength: f32) -> Self {
        let mut m = Self::default();
        m.emissive = color * strength;
        m
    }
}
