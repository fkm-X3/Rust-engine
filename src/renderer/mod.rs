pub mod camera;
pub mod context;
pub mod debug;
pub mod material;
pub mod memory;
pub mod mesh;
pub mod pipeline;
pub mod renderpass;
pub mod shadow;
pub mod swapchain;

pub use camera::Camera;
pub use material::Material;
pub use mesh::Mesh;
pub use renderpass::RenderSettings;

use std::sync::Arc;

use anyhow::{Context, Result};
use ash::vk;
use log::info;
use winit::window::Window;

use crate::{core::EngineConfig, scene::Scene};
use crate::core::Time;

use self::{
    context::VulkanContext,
    pipeline::PbrPipeline,
    swapchain::Swapchain,
};

// ─── Transform component ─────────────────────────────────────────────────────

/// Spatial transform component — position, rotation, scale.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
        }
    }

    pub fn from_position(pos: glam::Vec3) -> Self {
        Self { position: pos, ..Self::new() }
    }

    pub fn matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Light component ──────────────────────────────────────────────────────────

/// Type of light source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    Directional,
    Point { radius: f32 },
    Spot { inner_angle: f32, outer_angle: f32 },
}

/// Light component.
#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub kind: LightKind,
    pub color: glam::Vec3,
    pub intensity: f32,
    pub cast_shadows: bool,
}

impl Light {
    pub fn directional(color: glam::Vec3, intensity: f32) -> Self {
        Self { kind: LightKind::Directional, color, intensity, cast_shadows: true }
    }

    pub fn point(color: glam::Vec3, intensity: f32, radius: f32) -> Self {
        Self { kind: LightKind::Point { radius }, color, intensity, cast_shadows: false }
    }
}

// ─── Renderer ─────────────────────────────────────────────────────────────────

/// Push constants for the PBR vertex shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PushConstants {
    pub model: glam::Mat4,
    pub view_proj: glam::Mat4,
}

/// The main renderer — drives the Vulkan frame loop.
pub struct Renderer {
    context: Arc<VulkanContext>,
    swapchain: Swapchain,
    pipeline: PbrPipeline,
    current_frame: usize,
    max_frames_in_flight: usize,
    /// Per-frame sync primitives
    image_available: Vec<vk::Semaphore>,
    render_finished: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, config: &EngineConfig) -> Result<Self> {
        info!("Initializing Vulkan renderer...");

        let context = Arc::new(
            VulkanContext::new(&window, config)
                .context("Failed to create Vulkan context")?,
        );

        let swapchain = Swapchain::new(
            context.clone(),
            config.window_width,
            config.window_height,
            config.vsync,
        )
        .context("Failed to create swapchain")?;

        let pipeline = PbrPipeline::new(
            context.clone(),
            swapchain.render_pass(),
            config.msaa_samples,
        )
        .context("Failed to create PBR pipeline")?;

        let max_frames = config.max_frames_in_flight;
        let (image_available, render_finished, in_flight_fences) =
            Self::create_sync_objects(&context, max_frames)?;

        info!("Vulkan renderer ready.");

        Ok(Self {
            context,
            swapchain,
            pipeline,
            current_frame: 0,
            max_frames_in_flight: max_frames,
            image_available,
            render_finished,
            in_flight_fences,
        })
    }

    fn create_sync_objects(
        ctx: &VulkanContext,
        count: usize,
    ) -> Result<(Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>)> {
        let sem_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default()
            .flags(vk::FenceCreateFlags::SIGNALED);

        let mut img_sems = Vec::with_capacity(count);
        let mut render_sems = Vec::with_capacity(count);
        let mut fences = Vec::with_capacity(count);

        unsafe {
            for _ in 0..count {
                img_sems.push(ctx.device().create_semaphore(&sem_info, None)?);
                render_sems.push(ctx.device().create_semaphore(&sem_info, None)?);
                fences.push(ctx.device().create_fence(&fence_info, None)?);
            }
        }

        Ok((img_sems, render_sems, fences))
    }

    /// Render a frame from the scene.
    pub fn render(&mut self, scene: &Scene, _time: &Time) -> Result<()> {
        let frame = self.current_frame;

        unsafe {
            // Wait for the previous frame using this slot to finish.
            self.context.device().wait_for_fences(
                &[self.in_flight_fences[frame]],
                true,
                u64::MAX,
            )?;

            // Acquire next swapchain image.
            let (image_index, suboptimal) = match self.swapchain.acquire_next_image(
                self.image_available[frame],
            ) {
                Ok(v) => v,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.swapchain.recreate()?;
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            };

            self.context.device().reset_fences(&[self.in_flight_fences[frame]])?;

            // Record command buffer.
            let cmd = self.swapchain.command_buffer(image_index as usize);
            self.record_commands(cmd, image_index as usize, scene)?;

            // Submit.
            let wait_sems = [self.image_available[frame]];
            let signal_sems = [self.render_finished[frame]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cmds = [cmd];

            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait_sems)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signal_sems);

            self.context
                .device()
                .queue_submit(self.context.graphics_queue(), &[submit], self.in_flight_fences[frame])?;

            // Present.
            if suboptimal {
                self.swapchain.recreate()?;
            } else {
                self.swapchain.present(image_index, self.render_finished[frame])?;
            }
        }

        self.current_frame = (frame + 1) % self.max_frames_in_flight;
        Ok(())
    }

    fn record_commands(
        &mut self,
        cmd: vk::CommandBuffer,
        image_index: usize,
        scene: &Scene,
    ) -> Result<()> {
        let device = self.context.device();
        let extent = self.swapchain.extent();

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            device.begin_command_buffer(cmd, &begin_info)?;

            // Begin render pass
            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.05, 0.05, 0.08, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.swapchain.render_pass())
                .framebuffer(self.swapchain.framebuffer(image_index))
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                })
                .clear_values(&clear_values);

            device.cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);

            // Bind PBR pipeline
            self.pipeline.bind(cmd, extent);
        }

        // Draw scene entities (needs mutable borrow of self)
        self.draw_scene(cmd, scene)?;

        unsafe {
            let device = self.context.device();
            device.cmd_end_render_pass(cmd);
            device.end_command_buffer(cmd)?;
        }

        Ok(())
    }

    fn draw_scene(&mut self, cmd: vk::CommandBuffer, scene: &Scene) -> Result<()> {
        let world = scene.world();

        // Get the camera view-projection
        let view_proj = scene
            .main_camera()
            .map(|c| c.view_proj())
            .unwrap_or(glam::Mat4::IDENTITY);

        for (_, (transform, mesh, _material)) in
            world.query::<(&Transform, &Mesh, &Material)>().iter()
        {
            // Ensure mesh is uploaded to GPU (idempotent)
            mesh.ensure_uploaded(&self.context)?;
            
            let pc = PushConstants {
                model: transform.matrix(),
                view_proj,
            };

            self.pipeline.draw(cmd, mesh, &pc)?;
        }

        Ok(())
    }

    /// Handle a window resize event.
    pub fn handle_resize(&mut self, width: u32, height: u32) {
        self.swapchain.mark_dirty(width, height);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let device = self.context.device();
            let _ = device.device_wait_idle();

            for &sem in &self.image_available {
                device.destroy_semaphore(sem, None);
            }
            for &sem in &self.render_finished {
                device.destroy_semaphore(sem, None);
            }
            for &fence in &self.in_flight_fences {
                device.destroy_fence(fence, None);
            }
        }
    }
}