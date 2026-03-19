use glam::{Vec3, Vec4};

/// A debug line drawn for one frame.
#[derive(Debug, Clone, Copy)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec4,
    pub lifetime: f32,
}

/// Accumulates debug geometry for the current frame.
#[derive(Default)]
pub struct DebugDraw {
    lines: Vec<DebugLine>,
}

impl DebugDraw {
    pub fn line(&mut self, start: Vec3, end: Vec3, color: Vec4) {
        self.lines.push(DebugLine { start, end, color, lifetime: 0.0 });
    }

    pub fn aabb(&mut self, min: Vec3, max: Vec3, color: Vec4) {
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let edges = [
            (0,1),(1,2),(2,3),(3,0),
            (4,5),(5,6),(6,7),(7,4),
            (0,4),(1,5),(2,6),(3,7),
        ];
        for (a, b) in edges {
            self.line(corners[a], corners[b], color);
        }
    }

    pub fn sphere(&mut self, center: Vec3, radius: f32, color: Vec4, segments: u32) {
        let step = std::f32::consts::TAU / segments as f32;
        for i in 0..segments {
            let a = i as f32 * step;
            let b = (i + 1) as f32 * step;
            self.line(
                center + Vec3::new(a.cos(), 0.0, a.sin()) * radius,
                center + Vec3::new(b.cos(), 0.0, b.sin()) * radius,
                color,
            );
            self.line(
                center + Vec3::new(a.cos(), a.sin(), 0.0) * radius,
                center + Vec3::new(b.cos(), b.sin(), 0.0) * radius,
                color,
            );
        }
    }

    pub fn lines(&self) -> &[DebugLine] { &self.lines }
    pub fn clear(&mut self) { self.lines.clear(); }
}