use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use log::info;
use parking_lot::RwLock;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    assets::AssetManager,
    gui::GuiManager,
    physics::PhysicsWorld,
    renderer::Renderer,
    scene::Scene,
};

// ─── Configuration ────────────────────────────────────────────────────────────

/// Top-level engine configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub vsync: bool,
    pub max_frames_in_flight: usize,
    pub target_fps: Option<u32>,
    pub msaa_samples: u32,
    pub enable_validation_layers: bool,
    pub enable_physics: bool,
    pub gravity: glam::Vec3,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_title: "VKEngine".to_string(),
            window_width: 1280,
            window_height: 720,
            vsync: true,
            max_frames_in_flight: 2,
            target_fps: None,
            msaa_samples: 4,
            enable_validation_layers: cfg!(debug_assertions),
            enable_physics: true,
            gravity: glam::Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

// ─── Events ───────────────────────────────────────────────────────────────────

/// Events the engine emits to the application.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// A frame has started; dt is in seconds.
    Update(f32),
    /// Physics fixed step (dt is fixed, e.g. 1/60).
    FixedUpdate(f32),
    /// Rendering is about to happen.
    PreRender,
    /// Rendering is complete.
    PostRender,
    /// Engine is shutting down.
    Shutdown,
    /// Key pressed (scancode).
    KeyPressed(winit::keyboard::KeyCode),
    /// Key released (scancode).
    KeyReleased(winit::keyboard::KeyCode),
    /// Mouse moved (delta x, delta y).
    MouseMoved(f64, f64),
    /// Mouse button pressed.
    MousePressed(winit::event::MouseButton),
    /// Mouse button released.
    MouseReleased(winit::event::MouseButton),
    /// Window resized.
    Resized(u32, u32),
}

// ─── Time ─────────────────────────────────────────────────────────────────────

/// Tracks frame timing.
#[derive(Debug, Clone)]
pub struct Time {
    pub delta: f32,
    pub elapsed: f32,
    pub frame: u64,
    pub fps: f32,
    last_frame: Instant,
    fps_timer: f32,
    fps_frames: u32,
}

impl Time {
    pub fn new() -> Self {
        Self {
            delta: 0.0,
            elapsed: 0.0,
            frame: 0,
            fps: 0.0,
            last_frame: Instant::now(),
            fps_timer: 0.0,
            fps_frames: 0,
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        self.delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.elapsed += self.delta;
        self.frame += 1;

        self.fps_timer += self.delta;
        self.fps_frames += 1;
        if self.fps_timer >= 1.0 {
            self.fps = self.fps_frames as f32 / self.fps_timer;
            self.fps_frames = 0;
            self.fps_timer = 0.0;
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Engine ───────────────────────────────────────────────────────────────────

/// The central engine object. Owns all subsystems and drives the main loop.
pub struct Engine {
    config: EngineConfig,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui: Option<GuiManager>,
    physics: Option<PhysicsWorld>,
    assets: Arc<RwLock<AssetManager>>,
    scene: Arc<RwLock<Scene>>,
    time: Time,
    fixed_accumulator: f32,
    fixed_step: f32,
    running: bool,
    /// User-supplied callbacks
    on_update: Vec<Box<dyn FnMut(&mut Scene, &Time, &[EngineEvent]) + Send + Sync>>,
}

impl Engine {
    /// Create a new engine with the given configuration.
    pub fn new(config: EngineConfig) -> Result<Self> {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        info!("VKEngine initializing...");

        let assets = Arc::new(RwLock::new(AssetManager::new()));
        let scene = Arc::new(RwLock::new(Scene::new()));

        let physics = if config.enable_physics {
            Some(PhysicsWorld::new(config.gravity))
        } else {
            None
        };

        let fixed_step = 1.0 / 60.0;

        Ok(Self {
            config,
            window: None,
            renderer: None,
            gui: None,
            physics,
            assets,
            scene,
            time: Time::new(),
            fixed_accumulator: 0.0,
            fixed_step,
            running: true,
            on_update: Vec::new(),
        })
    }

    /// Register an update callback that runs every frame.
    pub fn on_update<F>(&mut self, f: F)
    where
        F: FnMut(&mut Scene, &Time, &[EngineEvent]) + Send + Sync + 'static,
    {
        self.on_update.push(Box::new(f));
    }

    /// Start the engine and enter the main loop.
    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = EngineApp {
            engine: self,
            pending_events: Vec::new(),
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }

    /// Access the asset manager.
    pub fn assets(&self) -> &Arc<RwLock<AssetManager>> {
        &self.assets
    }

    /// Access the scene.
    pub fn scene(&self) -> &Arc<RwLock<Scene>> {
        &self.scene
    }

    fn on_window_created(&mut self, window: Arc<Window>) -> Result<()> {
        info!("Window created: {}x{}", self.config.window_width, self.config.window_height);

        let renderer = Renderer::new(
            window.clone(),
            &self.config,
        )?;

        let gui = GuiManager::new(window.clone(), &self.config)?;

        self.renderer = Some(renderer);
        self.gui = Some(gui);
        self.window = Some(window);

        info!("Engine subsystems initialized.");
        Ok(())
    }

    fn update(&mut self, events: &[EngineEvent]) {
        self.time.tick();

        // Fixed physics step
        if let Some(physics) = &mut self.physics {
            self.fixed_accumulator += self.time.delta;
            while self.fixed_accumulator >= self.fixed_step {
                {
                    let mut scene = self.scene.write();
                    physics.step(self.fixed_step, &mut scene);
                }
                self.fixed_accumulator -= self.fixed_step;
            }
        }

        // User callbacks
        let time = self.time.clone();
        let mut scene = self.scene.write();
        for cb in &mut self.on_update {
            cb(&mut scene, &time, events);
        }
    }

    fn render(&mut self) -> Result<()> {
        if let Some(renderer) = &mut self.renderer {
            let scene = self.scene.read();
            renderer.render(&scene, &self.time)?;
        }
        if let Some(gui) = &mut self.gui {
            gui.render()?;
        }
        Ok(())
    }
}

// ─── winit App Handler ────────────────────────────────────────────────────────

struct EngineApp {
    engine: Engine,
    pending_events: Vec<EngineEvent>,
}

impl ApplicationHandler for EngineApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.engine.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title(&self.engine.config.window_title)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.engine.config.window_width,
                    self.engine.config.window_height,
                ));

            let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));

            self.engine
                .on_window_created(window)
                .expect("Engine init failed");
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Forward to GUI first
        if let Some(gui) = &mut self.engine.gui {
            gui.handle_window_event(&event);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.pending_events.push(EngineEvent::Shutdown);
                self.engine.running = false;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.pending_events
                    .push(EngineEvent::Resized(size.width, size.height));
                if let Some(renderer) = &mut self.engine.renderer {
                    renderer.handle_resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::event::ElementState;
                use winit::keyboard::PhysicalKey;
                if let PhysicalKey::Code(code) = event.physical_key {
                    let ev = match event.state {
                        ElementState::Pressed => EngineEvent::KeyPressed(code),
                        ElementState::Released => EngineEvent::KeyReleased(code),
                    };
                    self.pending_events.push(ev);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::ElementState;
                let ev = match state {
                    ElementState::Pressed => EngineEvent::MousePressed(button),
                    ElementState::Released => EngineEvent::MouseReleased(button),
                };
                self.pending_events.push(ev);
            }
            WindowEvent::RedrawRequested => {
                let events: Vec<EngineEvent> = self.pending_events.drain(..).collect();
                self.engine.update(&events);
                if let Err(e) = self.engine.render() {
                    log::error!("Render error: {}", e);
                }
                if let Some(w) = &self.engine.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.pending_events
                .push(EngineEvent::MouseMoved(delta.0, delta.1));
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.engine.window {
            window.request_redraw();
        }
    }
}