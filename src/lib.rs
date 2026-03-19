pub mod assets;
pub mod core;
pub mod gui;
pub mod physics;
pub mod renderer;
pub mod scene;
pub mod scripting;

pub use crate::core::{Engine, EngineConfig, EngineEvent};
pub use crate::scene::{Scene, SceneHandle};

/// Re-export commonly used types for convenience.
pub mod prelude {
    pub use crate::assets::{AssetHandle, AssetManager};
    pub use crate::core::{Engine, EngineConfig, EngineEvent, Time};
    pub use crate::physics::PhysicsWorld;
    pub use crate::renderer::{Camera, Light, LightKind, Material, Mesh, RenderSettings, Transform};
    pub use crate::scene::{Entity, Scene, SceneHandle};
    pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
    pub use hecs::World;
}
