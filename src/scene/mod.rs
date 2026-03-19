use std::sync::Arc;

use hecs::World;
use parking_lot::RwLock;

use crate::renderer::{Camera, Light, Material, Mesh, Transform};

/// A handle to a scene usable from multiple threads.
pub type SceneHandle = Arc<RwLock<Scene>>;

/// Re-export Entity from hecs
pub use hecs::Entity;

/// The scene owns an ECS world and the active camera.
pub struct Scene {
    world: World,
    main_camera: Option<Camera>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            main_camera: Some(Camera::default()),
        }
    }

    // ─── Camera ───────────────────────────────────────────────────────────────

    pub fn set_main_camera(&mut self, camera: Camera) {
        self.main_camera = Some(camera);
    }

    pub fn main_camera(&self) -> Option<&Camera> {
        self.main_camera.as_ref()
    }

    pub fn main_camera_mut(&mut self) -> Option<&mut Camera> {
        self.main_camera.as_mut()
    }

    // ─── ECS world ────────────────────────────────────────────────────────────

    pub fn world(&self) -> &World { &self.world }
    pub fn world_mut(&mut self) -> &mut World { &mut self.world }

    // ─── Entity helpers ───────────────────────────────────────────────────────

    /// Spawn an entity with a transform, mesh, and material.
    pub fn spawn_mesh(
        &mut self,
        transform: Transform,
        mesh: Mesh,
        material: Material,
    ) -> Entity {
        self.world.spawn((transform, mesh, material))
    }

    /// Spawn an entity with a transform and light.
    pub fn spawn_light(&mut self, transform: Transform, light: Light) -> Entity {
        self.world.spawn((transform, light))
    }

    /// Spawn an arbitrary bundle.
    pub fn spawn<B: hecs::DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.world.spawn(bundle)
    }

    /// Despawn an entity.
    pub fn despawn(&mut self, entity: Entity) -> Result<(), hecs::NoSuchEntity> {
        self.world.despawn(entity)
    }

    /// Query all entities with a transform.
    pub fn query_transforms(&self) -> hecs::QueryBorrow<'_, &Transform> {
        self.world.query::<&Transform>()
    }

    /// Get a component reference.
    pub fn get<C: hecs::Component>(&self, entity: Entity) -> Option<hecs::Ref<'_, C>> {
        self.world.get::<&C>(entity).ok()
    }

    /// Get a mutable component reference.
    pub fn get_mut<C: hecs::Component>(&self, entity: Entity) -> Option<hecs::RefMut<'_, C>> {
        self.world.get::<&mut C>(entity).ok()
    }

    /// Get total entity count in the scene.
    pub fn entity_count(&self) -> usize {
        self.world.len() as usize
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}