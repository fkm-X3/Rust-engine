use anyhow::Result;
use vkengine::prelude::*;

fn main() -> Result<()> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║          VKEngine - Vulkan Game Engine Demo                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n🎮 SHOWCASING ENGINE CAPABILITIES:");
    println!("   ✓ PBR Rendering (Cook-Torrance BRDF)");
    println!("   ✓ Dynamic Physics (Rapier3D)");
    println!("   ✓ Multi-Light Setup (Directional + Point Lights)");
    println!("   ✓ Material Variety (Metal/Plastic with varied roughness)");
    println!("   ✓ Real-time Camera Orbit");
    println!("   ✓ ECS Architecture (hecs)");
    println!("\n📝 CONTROLS:");
    println!("   • ESC - Exit");
    println!("   • Camera automatically orbits the scene");
    println!("\n⚡ Starting engine...\n");

    let config = EngineConfig {
        window_title: "VKEngine — Bleeding Edge Demo".to_string(),
        window_width: 1920,
        window_height: 1080,
        vsync: true,
        msaa_samples: 4,
        enable_validation_layers: cfg!(debug_assertions),
        enable_physics: true,
        gravity: Vec3::new(0.0, -9.81, 0.0),
        ..Default::default()
    };

    let mut engine = Engine::new(config)?;

    // ─── Scene Setup ──────────────────────────────────────────────────────────
    {
        let mut scene = engine.scene().write();

        // Camera with wide FOV for dramatic effect
        scene.set_main_camera(Camera::perspective(75.0, 1920.0 / 1080.0, 0.1, 500.0));

        // ═══ GROUND PLATFORM ═══
        scene.spawn_mesh(
            Transform {
                position: Vec3::new(0.0, -2.0, 0.0),
                scale: Vec3::new(25.0, 0.5, 25.0),
                ..Default::default()
            },
            Mesh::cube(),
            Material::plastic(Vec3::new(0.15, 0.15, 0.18)),
        );

        // ═══ PBR MATERIAL SHOWCASE ═══
        // Row 1: Metallic spheres with varying roughness
        for i in 0..7 {
            let roughness = (i as f32) / 6.0;
            let x = (i as f32 - 3.0) * 2.0;
            
            scene.spawn_mesh(
                Transform {
                    position: Vec3::new(x, 0.5, -5.0),
                    scale: Vec3::new(0.8, 0.8, 0.8),
                    ..Default::default()
                },
                Mesh::sphere(32, 16),
                Material::metal(Vec3::new(0.95, 0.85, 0.65), roughness), // Gold-like color
            );
        }

        // Row 2: Colored plastic spheres
        let colors = [
            Vec3::new(0.9, 0.2, 0.2),  // Red
            Vec3::new(0.2, 0.9, 0.2),  // Green
            Vec3::new(0.2, 0.2, 0.9),  // Blue
            Vec3::new(0.9, 0.5, 0.1),  // Orange
            Vec3::new(0.9, 0.2, 0.7),  // Magenta
        ];
        
        for (i, color) in colors.iter().enumerate() {
            let x = (i as f32 - 2.0) * 2.5;
            scene.spawn_mesh(
                Transform {
                    position: Vec3::new(x, 0.5, -2.0),
                    scale: Vec3::new(0.8, 0.8, 0.8),
                    ..Default::default()
                },
                Mesh::sphere(32, 16),
                Material::plastic(*color),
            );
        }

        // Row 3: Metallic cubes with different colors and roughness
        for i in 0..5 {
            let hue = (i as f32) / 5.0;
            let color = hsv_to_rgb(hue, 0.7, 0.9);
            let roughness = 0.2 + (i as f32) * 0.15;
            let x = (i as f32 - 2.0) * 2.0;
            
            scene.spawn_mesh(
                Transform {
                    position: Vec3::new(x, 0.5, 1.0),
                    rotation: Quat::from_rotation_y(0.5),
                    ..Default::default()
                },
                Mesh::cube(),
                Material::metal(color, roughness),
            );
        }

        // ═══ PHYSICS DEMONSTRATION ═══
        // Create a tower of physics-enabled cubes
        for layer in 0..5 {
            for i in 0..3 {
                let x = (i as f32 - 1.0) * 1.1;
                let y = 4.0 + layer as f32 * 1.05;
                let color_hue = (layer as f32 * 0.2 + i as f32 * 0.1) % 1.0;
                let color = hsv_to_rgb(color_hue, 0.6, 0.85);
                
                scene.spawn_mesh(
                    Transform {
                        position: Vec3::new(x, y, 4.0),
                        rotation: Quat::from_rotation_y((layer + i) as f32 * 0.3),
                        ..Default::default()
                },
                    Mesh::cube(),
                    Material::plastic(color),
                );
            }
        }

        // Scattered physics spheres falling into the scene
        for i in 0..8 {
            let angle = (i as f32) * 0.8;
            let radius = 3.0 + (i as f32) * 0.5;
            let x = radius * angle.cos();
            let z = radius * angle.sin();
            let y = 8.0 + i as f32 * 1.5;
            
            scene.spawn_mesh(
                Transform {
                    position: Vec3::new(x, y, z + 6.0),
                    scale: Vec3::new(0.6, 0.6, 0.6),
                    ..Default::default()
                },
                Mesh::sphere(24, 12),
                Material::metal(
                    hsv_to_rgb((i as f32) / 8.0, 0.8, 0.95),
                    0.3,
                ),
            );
        }

        // ═══ LIGHTING SETUP ═══
        // Primary sun light (warm)
        scene.spawn_light(
            Transform {
                position: Vec3::new(15.0, 25.0, 10.0),
                ..Default::default()
            },
            Light::directional(Vec3::new(1.0, 0.95, 0.85), 5.0),
        );

        // Fill light (cool blue, from opposite side)
        scene.spawn_light(
            Transform {
                position: Vec3::new(-10.0, 15.0, -8.0),
                ..Default::default()
            },
            Light::directional(Vec3::new(0.5, 0.6, 0.9), 1.5),
        );

        // Accent point lights for visual interest
        let point_light_colors = [
            (Vec3::new(5.0, 2.0, 0.0), Vec3::new(1.0, 0.3, 0.2), 15.0, 10.0),  // Red
            (Vec3::new(-5.0, 2.0, 0.0), Vec3::new(0.2, 0.3, 1.0), 15.0, 10.0), // Blue
            (Vec3::new(0.0, 2.0, 6.0), Vec3::new(0.2, 1.0, 0.3), 12.0, 8.0),   // Green
        ];

        for (pos, color, intensity, radius) in point_light_colors {
            scene.spawn_light(
                Transform { position: pos, ..Default::default() },
                Light::point(color, intensity, radius),
            );
        }

        println!("✓ Scene created with {} entities", scene.entity_count());
    }

    // ─── Update Loop ──────────────────────────────────────────────────────────
    let mut yaw = 0.0_f32;
    let mut last_fps_print = 0.0_f32;

    engine.on_update(move |scene, time, _events| {
        // Orbit camera around the scene
        yaw += time.delta * 0.15; // Slow rotation
        
        let distance = 18.0;
        let height = 8.0;
        
        if let Some(cam) = scene.main_camera_mut() {
            cam.position = Vec3::new(
                distance * yaw.cos(),
                height,
                distance * yaw.sin(),
            );
            cam.target = Vec3::new(0.0, 2.0, 0.0);
        }

        // Print FPS every 2 seconds
        if time.elapsed - last_fps_print > 2.0 {
            println!("📊 FPS: {:.1} | Entities: {} | Frame: {}", 
                time.fps, scene.entity_count(), time.frame);
            last_fps_print = time.elapsed;
        }
    });

    // ─── Run ──────────────────────────────────────────────────────────────────
    engine.run()
}

/// Convert HSV color to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Vec3 {
    let h6 = h * 6.0;
    let i = h6.floor() as u32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    
    match i {
        0 => Vec3::new(v, t, p),
        1 => Vec3::new(q, v, p),
        2 => Vec3::new(p, v, t),
        3 => Vec3::new(p, q, v),
        4 => Vec3::new(t, p, v),
        _ => Vec3::new(v, p, q),
    }
}
