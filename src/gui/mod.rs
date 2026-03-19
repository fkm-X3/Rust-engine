use std::sync::Arc;

use anyhow::Result;
use log::info;
use winit::window::Window;

use crate::core::EngineConfig;

/// Message type for the editor panel.
#[derive(Debug, Clone)]
pub enum EditorMessage {
    /// User selected an entity in the hierarchy panel.
    EntitySelected(u64),
    /// Toggle the physics debug overlay.
    TogglePhysicsDebug,
    /// Toggle wireframe rendering.
    ToggleWireframe,
    /// Slider changed a float value (field name, new value).
    FloatChanged(String, f32),
    /// Play / Pause the simulation.
    PlayPause,
    /// Step one frame.
    Step,
}

/// State for the Iced editor UI.
#[derive(Debug, Default, Clone)]
pub struct EditorState {
    pub selected_entity: Option<u64>,
    pub physics_debug: bool,
    pub wireframe: bool,
    pub playing: bool,
    pub fps: f32,
    pub frame: u64,
    pub entity_count: usize,
}

/// Drives the Iced runtime on top of the engine window.
pub struct GuiManager {
    window: Arc<Window>,
    state: EditorState,
    // In a full integration this holds the Iced runtime, renderer,
    // and wgpu device/queue references shared with the Vulkan renderer
    // via an interop fence. Wired as a skeleton here.
}

impl GuiManager {
    pub fn new(window: Arc<Window>, _config: &EngineConfig) -> Result<Self> {
        info!("Initializing Iced GUI manager...");

        // Full integration:
        //   1. Create wgpu Instance/Adapter/Device/Queue sharing the physical device
        //      with Vulkan (via ash-wgpu interop crate or separate wgpu device).
        //   2. Build iced_wgpu::Renderer and iced_winit::program::State.
        //   3. Store them here.

        Ok(Self {
            window,
            state: EditorState::default(),
        })
    }

    /// Forward winit window events to Iced.
    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) {
        // iced_winit::program::State::update takes winit events.
        // let _ = self.iced_state.update(...);
        let _ = event;
    }

    /// Render the GUI overlay for this frame.
    pub fn render(&mut self) -> Result<()> {
        // Full render:
        //   1. Compute Iced layout/primitives from current EditorState.
        //   2. iced_wgpu::Renderer::present() → composite over the 3-D frame.
        Ok(())
    }

    /// Update stats displayed in the HUD.
    pub fn set_stats(&mut self, fps: f32, frame: u64, entities: usize) {
        self.state.fps = fps;
        self.state.frame = frame;
        self.state.entity_count = entities;
    }

    /// Process a message from the UI.
    pub fn process(&mut self, msg: EditorMessage) -> Vec<EditorMessage> {
        match msg {
            EditorMessage::EntitySelected(id) => {
                self.state.selected_entity = Some(id);
            }
            EditorMessage::TogglePhysicsDebug => {
                self.state.physics_debug = !self.state.physics_debug;
            }
            EditorMessage::ToggleWireframe => {
                self.state.wireframe = !self.state.wireframe;
            }
            EditorMessage::PlayPause => {
                self.state.playing = !self.state.playing;
            }
            _ => {}
        }
        Vec::new()
    }

    pub fn state(&self) -> &EditorState { &self.state }
}

// ─── Iced Application definition ─────────────────────────────────────────────
//
//todo: In a full integration, this would be the struct implementing `iced::Application`,

mod editor_ui {
    use super::EditorMessage;
    use iced::{
        widget::{button, column, container, row, text, slider, horizontal_rule},
        Element, Length, Theme,
    };

    /// Render the hierarchy panel.
    pub fn hierarchy_panel<'a>(
        entity_ids: &'a [u64],
        selected: Option<u64>,
    ) -> Element<'a, EditorMessage> {
        let items: Vec<Element<EditorMessage>> = entity_ids
            .iter()
            .map(|&id| {
                let label = format!("Entity {}", id);
                let is_selected = selected == Some(id);
                let btn = button(text(label))
                    .on_press(EditorMessage::EntitySelected(id));
                btn.into()
            })
            .collect();

        container(column(items).spacing(2))
            .padding(8)
            .width(Length::Fixed(220.0))
            .into()
    }

    /// Render the top toolbar.
    pub fn toolbar<'a>(playing: bool) -> Element<'a, EditorMessage> {
        let play_label = if playing { "⏸ Pause" } else { "▶ Play" };

        row![
            button(text(play_label)).on_press(EditorMessage::PlayPause),
            button(text("⏭ Step")).on_press(EditorMessage::Step),
            button(text("⬛ Wireframe")).on_press(EditorMessage::ToggleWireframe),
            button(text("⬤ Physics")).on_press(EditorMessage::TogglePhysicsDebug),
        ]
        .spacing(8)
        .padding(8)
        .into()
    }

    /// Render the stats HUD.
    pub fn stats_hud<'a>(fps: f32, frame: u64, entities: usize) -> Element<'a, EditorMessage> {
        column![
            text(format!("FPS: {:.1}", fps)),
            text(format!("Frame: {}", frame)),
            text(format!("Entities: {}", entities)),
        ]
        .spacing(4)
        .padding(8)
        .into()
    }
}

pub use editor_ui::{hierarchy_panel, stats_hud, toolbar};