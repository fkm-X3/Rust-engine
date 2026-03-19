use anyhow::Result;
use vkengine::prelude::*;
use vkengine::physics::{ColliderShape, RigidBodyComponent, ColliderComponent};

fn main() -> Result<()> {
    let config = EngineConfig {
        window_title: "VKEngine Physics Sandbox".into(),
        window_width: 1600,
        window_height: 900,
        vsync: false,                    // uncapped FPS for benchmarking
        enable_physics: true,
        enable_validation_layers: false, // off for release perf
        ..Default::default()
    };

    let mut engine = Engine::new(config)?;

    // ─── Build scene ──────────────────────────────────────────────────────────
    {
        let mut scene = engine.scene().write();

        scene.set_main_camera(Camera::perspective(70.0, 16.0 / 9.0, 0.05, 500.0));

        // Ground
        scene.spawn_mesh(
            Transform {
                position: Vec3::new(0.0, -0.5, 0.0),
                scale:    Vec3::new(30.0, 0.5, 30.0),
                ..Default::default()
            },
            Mesh::cube(),
            Material::plastic(Vec3::new(0.15, 0.15, 0.15)),
        );

        // Walls
        for (pos, scale) in [
            (Vec3::new( 15.0, 5.0,  0.0), Vec3::new(0.5, 10.0, 30.0)),
            (Vec3::new(-15.0, 5.0,  0.0), Vec3::new(0.5, 10.0, 30.0)),
            (Vec3::new(  0.0, 5.0,  15.0), Vec3::new(30.0, 10.0, 0.5)),
            (Vec3::new(  0.0, 5.0, -15.0), Vec3::new(30.0, 10.0, 0.5)),
        ] {
            scene.spawn_mesh(
                Transform { position: pos, scale, ..Default::default() },
                Mesh::cube(),
                Material::plastic(Vec3::new(0.2, 0.2, 0.25)),
            );
        }

        // Spawn 50 colourful cubes at random positions
        for i in 0..50_u32 {
            let angle = i as f32 * 0.4;
            let r = (i as f32 * 0.3 + 1.0) % 12.0;
            let pos = Vec3::new(r * angle.cos(), 2.0 + i as f32 * 0.3, r * angle.sin());

            let hue = i as f32 / 50.0;
            let color = hsv_to_rgb(hue, 0.8, 0.9);

            scene.spawn_mesh(
                Transform { position: pos, ..Default::default() },
                Mesh::cube(),
                Material::metal(color, 0.1 + (i as f32 / 50.0) * 0.8),
            );
        }

        // Sun light
        scene.spawn_light(
            Transform { position: Vec3::new(5.0, 15.0, 8.0), ..Default::default() },
            Light::directional(Vec3::new(1.0, 0.95, 0.85), 4.0),
        );

        // Fill light
        scene.spawn_light(
            Transform { position: Vec3::new(-8.0, 8.0, -5.0), ..Default::default() },
            Light::directional(Vec3::new(0.4, 0.5, 0.9), 0.8),
        );
    }

    // ─── Per-frame logic ──────────────────────────────────────────────────────
    let mut yaw   = 0.0_f32;
    let mut pitch = 0.3_f32;

    engine.on_update(move |scene, time, events| {
        // Handle mouse orbit
        for event in events {
            if let vkengine::EngineEvent::MouseMoved(dx, dy) = event {
                yaw   += *dx as f32 * 0.003;
                pitch  = (pitch - *dy as f32 * 0.003).clamp(-1.4, 1.4);
            }
        }

        let dist = 18.0_f32;
        if let Some(cam) = scene.main_camera_mut() {
            cam.position = Vec3::new(
                dist * pitch.cos() * yaw.cos(),
                dist * pitch.sin(),
                dist * pitch.cos() * yaw.sin(),
            );
            cam.target = Vec3::new(0.0, 3.0, 0.0);
        }
    });

    engine.run()
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Vec3 {
    let h6  = h * 6.0;
    let i   = h6.floor() as u32 % 6;
    let f   = h6 - h6.floor();
    let p   = v * (1.0 - s);
    let q   = v * (1.0 - f * s);
    let t   = v * (1.0 - (1.0 - f) * s);
    match i {
        0 => Vec3::new(v, t, p),
        1 => Vec3::new(q, v, p),
        2 => Vec3::new(p, v, t),
        3 => Vec3::new(p, q, v),
        4 => Vec3::new(t, p, v),
        _ => Vec3::new(v, p, q),
    }
}
