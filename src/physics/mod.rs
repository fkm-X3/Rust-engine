use glam::Vec3;
use rapier3d::prelude::*;

use crate::scene::Scene;
use crate::renderer::Transform;

// ─── ECS Components ──────────────────────────────────────────────────────────

/// Marks an entity as a Rapier rigid body.
#[derive(Debug, Clone)]
pub struct RigidBodyComponent {
    pub handle: RigidBodyHandle,
    pub body_type: RigidBodyType,
}

/// Marks an entity as having a Rapier collider.
#[derive(Debug, Clone)]
pub struct ColliderComponent {
    pub handle: ColliderHandle,
}

/// The shape of a collider.
#[derive(Debug, Clone)]
pub enum ColliderShape {
    Sphere { radius: f32 },
    Box    { half_extents: Vec3 },
    Capsule { half_height: f32, radius: f32 },
    ConvexHull { points: Vec<Vec3> },
}

// ─── PhysicsWorld ─────────────────────────────────────────────────────────────

/// Owns the Rapier physics world and all associated structures.
pub struct PhysicsWorld {
    gravity: rapier3d::math::Vector<f32>,
    rigid_body_set:  RigidBodySet,
    collider_set:    ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager:  IslandManager,
    broad_phase:     DefaultBroadPhase,
    narrow_phase:    NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver:      CCDSolver,
    query_pipeline:  QueryPipeline,
    event_collector: ChannelEventCollector,
    collision_recv:  crossbeam_channel::Receiver<CollisionEvent>,
    contact_force_recv: crossbeam_channel::Receiver<ContactForceEvent>,
}

impl PhysicsWorld {
    pub fn new(gravity: Vec3) -> Self {
        let (collision_send, collision_recv) = crossbeam_channel::unbounded();
        let (contact_force_send, contact_force_recv) = crossbeam_channel::unbounded();
        let event_collector = ChannelEventCollector::new(collision_send, contact_force_send);

        Self {
            gravity: rapier3d::math::Vector::new(gravity.x, gravity.y, gravity.z),
            rigid_body_set:  RigidBodySet::new(),
            collider_set:    ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager:  IslandManager::new(),
            broad_phase:     DefaultBroadPhase::new(),
            narrow_phase:    NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver:      CCDSolver::new(),
            query_pipeline:  QueryPipeline::new(),
            event_collector,
            collision_recv,
            contact_force_recv,
        }
    }

    // ─── Step ─────────────────────────────────────────────────────────────────

    /// Run one fixed physics step, then sync rigid-body positions back to ECS.
    pub fn step(&mut self, dt: f32, scene: &mut Scene) {
        self.integration_parameters.dt = dt;

        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &self.event_collector,
        );

        // Sync physics → ECS transforms
        for (_, (rb_comp, transform)) in scene
            .world_mut()
            .query_mut::<(&RigidBodyComponent, &mut Transform)>()
        {
            if let Some(rb) = self.rigid_body_set.get(rb_comp.handle) {
                let pos = rb.translation();
                let rot = rb.rotation();
                transform.position = Vec3::new(pos.x, pos.y, pos.z);
                transform.rotation = glam::Quat::from_xyzw(
                    rot.i, rot.j, rot.k, rot.w,
                );
            }
        }

        // Drain collision events
        while let Ok(event) = self.collision_recv.try_recv() {
            log::trace!("Collision: {:?}", event);
        }
    }

    // ─── Body creation helpers ───────────────────────────────────────────────

    /// Add a dynamic rigid body at the given position.
    pub fn add_dynamic_body(&mut self, position: Vec3) -> RigidBodyHandle {
        let rb = RigidBodyBuilder::dynamic()
            .translation(rapier3d::math::Vector::new(position.x, position.y, position.z))
            .build();
        self.rigid_body_set.insert(rb)
    }

    /// Add a static (fixed) rigid body.
    pub fn add_static_body(&mut self, position: Vec3) -> RigidBodyHandle {
        let rb = RigidBodyBuilder::fixed()
            .translation(rapier3d::math::Vector::new(position.x, position.y, position.z))
            .build();
        self.rigid_body_set.insert(rb)
    }

    /// Add a collider attached to a rigid body.
    pub fn add_collider(
        &mut self,
        shape: ColliderShape,
        body: RigidBodyHandle,
        restitution: f32,
        friction: f32,
    ) -> ColliderHandle {
        let shared = self.build_shape(&shape);
        let col = ColliderBuilder::new(shared)
            .restitution(restitution)
            .friction(friction)
            .build();
        self.collider_set.insert_with_parent(col, body, &mut self.rigid_body_set)
    }

    fn build_shape(&self, shape: &ColliderShape) -> SharedShape {
        match shape {
            ColliderShape::Sphere { radius } => SharedShape::ball(*radius),
            ColliderShape::Box { half_extents } => SharedShape::cuboid(
                half_extents.x,
                half_extents.y,
                half_extents.z,
            ),
            ColliderShape::Capsule { half_height, radius } => {
                SharedShape::capsule_y(*half_height, *radius)
            }
            ColliderShape::ConvexHull { points } => {
                let pts: Vec<rapier3d::math::Point<f32>> = points
                    .iter()
                    .map(|p| rapier3d::math::Point::new(p.x, p.y, p.z))
                    .collect();
                SharedShape::convex_hull(&pts).unwrap_or_else(|| SharedShape::ball(0.5))
            }
        }
    }

    // ─── Forces ───────────────────────────────────────────────────────────────

    /// Apply an impulse to a rigid body.
    pub fn apply_impulse(&mut self, handle: RigidBodyHandle, impulse: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.apply_impulse(
                rapier3d::math::Vector::new(impulse.x, impulse.y, impulse.z),
                true,
            );
        }
    }

    /// Apply a torque impulse to a rigid body.
    pub fn apply_torque_impulse(&mut self, handle: RigidBodyHandle, torque: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.apply_torque_impulse(
                rapier3d::math::Vector::new(torque.x, torque.y, torque.z),
                true,
            );
        }
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    /// Cast a ray and return the closest hit (handle, time-of-impact).
    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_toi: f32,
    ) -> Option<(ColliderHandle, f32)> {
        let ray = Ray::new(
            rapier3d::math::Point::new(origin.x, origin.y, origin.z),
            rapier3d::math::Vector::new(direction.x, direction.y, direction.z),
        );
        let filter = QueryFilter::default();

        self.query_pipeline
            .cast_ray(
                &self.rigid_body_set,
                &self.collider_set,
                &ray,
                max_toi,
                true,
                filter,
            )
            .map(|(handle, toi)| (handle, toi))
    }

    // ─── Accessors ────────────────────────────────────────────────────────────

    pub fn rigid_body_set(&self) -> &RigidBodySet { &self.rigid_body_set }
    pub fn collider_set(&self)   -> &ColliderSet  { &self.collider_set }

    pub fn rigid_body(&self, h: RigidBodyHandle) -> Option<&RigidBody> {
        self.rigid_body_set.get(h)
    }

    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.gravity = rapier3d::math::Vector::new(gravity.x, gravity.y, gravity.z);
    }
}