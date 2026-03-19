use glam::{Mat4, Vec3};

/// Projection modes.
#[derive(Debug, Clone, Copy)]
pub enum Projection {
    Perspective { fov_y: f32, near: f32, far: f32 },
    Orthographic { size: f32, near: f32, far: f32 },
}

/// A camera that can be attached to a scene entity.
#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub projection: Projection,
    pub aspect: f32,
}

impl Camera {
    /// Create a perspective camera.
    pub fn perspective(fov_degrees: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::Perspective {
                fov_y: fov_degrees.to_radians(),
                near,
                far,
            },
            aspect,
        }
    }

    /// View matrix.
    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Projection matrix.
    pub fn projection(&self) -> Mat4 {
        match self.projection {
            Projection::Perspective { fov_y, near, far } => {
                // Vulkan clip-space: Y flipped, depth [0,1]
                let mut m = Mat4::perspective_rh(fov_y, self.aspect, near, far);
                m.y_axis.y *= -1.0; // flip Y for Vulkan
                m
            }
            Projection::Orthographic { size, near, far } => {
                Mat4::orthographic_rh(
                    -size * self.aspect,
                    size * self.aspect,
                    -size,
                    size,
                    near,
                    far,
                )
            }
        }
    }

    /// Combined view-projection matrix.
    pub fn view_proj(&self) -> Mat4 {
        self.projection() * self.view()
    }

    /// Update aspect ratio (call on window resize).
    pub fn set_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }

    /// Orbit around the target by yaw/pitch deltas (radians).
    pub fn orbit(&mut self, yaw: f32, pitch: f32) {
        let offset = self.position - self.target;
        let radius = offset.length();
        let current_yaw = offset.z.atan2(offset.x);
        let current_pitch = (offset.y / radius).asin();

        let new_yaw = current_yaw + yaw;
        let new_pitch = (current_pitch + pitch).clamp(-1.5, 1.5);

        self.position = self.target
            + Vec3::new(
                radius * new_pitch.cos() * new_yaw.cos(),
                radius * new_pitch.sin(),
                radius * new_pitch.cos() * new_yaw.sin(),
            );
    }

    /// Zoom (move toward/away from target).
    pub fn zoom(&mut self, delta: f32) {
        let dir = (self.position - self.target).normalize();
        let dist = (self.position - self.target).length();
        self.position = self.target + dir * (dist - delta).max(0.1);
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::perspective(60.0, 16.0 / 9.0, 0.1, 1000.0)
    }
}
