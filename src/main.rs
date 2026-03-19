use anyhow::Result;
use vkengine::prelude::*;

fn main() -> Result<()> {
    let config = EngineConfig {
        window_title: "VKEngine — Sandbox".to_string(),
        window_width: 1280,
        window_height: 720,
        vsync: true,
        msaa_samples: 4,
        enable_validation_layers: cfg!(debug_assertions),
        enable_physics: true,
        gravity: Vec3::new(0.0, -9.81, 0.0),
        ..Default::default()
    };

    let mut engine = Engine::new(config)?;

    // ─── Scene setup ──────────────────────────────────────────────────────────
    {
        let mut scene = engine.scene().write();

        // Main camera
        scene.set_main_camera(Camera::perspective(60.0, 1280.0 / 720.0, 0.1, 1000.0));

        // Floor plane
        scene.spawn_mesh(
            Transform {
                position: Vec3::new(0.0, -1.0, 0.0),
                scale:    Vec3::new(20.0, 0.1, 20.0),
                ..Default::default()
            },
            Mesh::cube(),
            Material::plastic(Vec3::new(0.3, 0.3, 0.3)),
        );

        // A row of metallic cubes
        for i in 0..5_i32 {
            scene.spawn_mesh(
                Transform {
                    position: Vec3::new((i - 2) as f32 * 1.5, 0.5, 0.0),
                    ..Default::default()
                },
                Mesh::cube(),
                Material::metal(
                    Vec3::new(0.2 + i as f32 * 0.15, 0.4, 0.8 - i as f32 * 0.1),
                    0.1 + i as f32 * 0.2,
                ),
            );
        }

        // Directional (sun) light
        scene.spawn_light(
            Transform {
                position: Vec3::new(10.0, 20.0, 10.0),
                ..Default::default()
            },
            Light::directional(Vec3::ONE, 3.0),
        );
    }

    // ─── Update callback ──────────────────────────────────────────────────────
    let mut t: f32 = 0.0;
    engine.on_update(move |scene, time, _events| {
        t = time.elapsed;

        // Slowly orbit camera
        if let Some(cam) = scene.main_camera_mut() {
            cam.position = Vec3::new(
                6.0 * (t * 0.2).cos(),
                3.5,
                6.0 * (t * 0.2).sin(),
            );
        }
    });

    // ─── Run ──────────────────────────────────────────────────────────────────
    engine.run()
}
