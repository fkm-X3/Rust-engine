use anyhow::{Context as _, Result};
use ash::vk;
use std::sync::Arc;

use super::context::VulkanContext;

/// Shadow map resolution (square texture).
const SHADOW_MAP_SIZE: u32 = 2048;

/// Manages shadow map rendering (depth-only pass).
pub struct ShadowPass {
    context: Arc<VulkanContext>,
    
    // Shadow map resources
    depth_image: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
    sampler: vk::Sampler,
    
    // Render pass and framebuffer
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    
    // Pipeline
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl ShadowPass {
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let device = context.device();

        // Create depth image
        let (depth_image, depth_memory) = Self::create_depth_image(&context)?;
        let depth_view = Self::create_depth_view(&context, depth_image)?;
        let sampler = Self::create_sampler(&context)?;

        // Create render pass
        let render_pass = Self::create_render_pass(&context)?;

        // Create framebuffer
        let framebuffer = unsafe {
            let attachments = [depth_view];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(SHADOW_MAP_SIZE)
                .height(SHADOW_MAP_SIZE)
                .layers(1);

            device.create_framebuffer(&fb_info, None)?
        };

        // Create pipeline
        let (pipeline_layout, pipeline) = Self::create_pipeline(&context, render_pass)?;

        Ok(Self {
            context,
            depth_image,
            depth_memory,
            depth_view,
            sampler,
            render_pass,
            framebuffer,
            pipeline_layout,
            pipeline,
        })
    }

    fn create_depth_image(
        context: &Arc<VulkanContext>,
    ) -> Result<(vk::Image, vk::DeviceMemory)> {
        let device = context.device();

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D32_SFLOAT)
            .extent(vk::Extent3D {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let image = unsafe { device.create_image(&image_info, None)? };

        let mem_reqs = unsafe { device.get_image_memory_requirements(image) };

        let mem_props = unsafe {
            context
                .instance()
                .get_physical_device_memory_properties(context.physical_device())
        };

        let memory_type_index = (0..mem_props.memory_type_count)
            .find(|&i| {
                let type_supported = (mem_reqs.memory_type_bits & (1 << i)) != 0;
                let properties = mem_props.memory_types[i as usize].property_flags;
                let device_local = properties.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL);
                type_supported && device_local
            })
            .context("Failed to find suitable memory type for shadow map")?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(memory_type_index);

        let memory = unsafe { device.allocate_memory(&alloc_info, None)? };

        unsafe { device.bind_image_memory(image, memory, 0)? };

        Ok((image, memory))
    }

    fn create_depth_view(
        context: &Arc<VulkanContext>,
        image: vk::Image,
    ) -> Result<vk::ImageView> {
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::D32_SFLOAT)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        Ok(unsafe { context.device().create_image_view(&view_info, None)? })
    }

    fn create_sampler(context: &Arc<VulkanContext>) -> Result<vk::Sampler> {
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .compare_enable(true)
            .compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE);

        Ok(unsafe {
            context.device().create_sampler(&sampler_info, None)?
        })
    }

    fn create_render_pass(context: &Arc<VulkanContext>) -> Result<vk::RenderPass> {
        let depth_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::D32_SFLOAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);

        let depth_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .depth_stencil_attachment(&depth_ref);

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .dst_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);

        let attachments = [depth_attachment];
        let subpasses = [subpass];
        let dependencies = [dependency];

        let rp_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        Ok(unsafe {
            context.device().create_render_pass(&rp_info, None)?
        })
    }

    fn create_pipeline(
        context: &Arc<VulkanContext>,
        render_pass: vk::RenderPass,
    ) -> Result<(vk::PipelineLayout, vk::Pipeline)> {
        let device = context.device();

        // Load shadow vertex shader
        let vert_shader_code = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/shadow.vert.spv"));
        let vert_module = Self::create_shader_module(device, vert_shader_code)?;

        // Push constant for light-space matrix + model matrix
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .offset(0)
            .size(std::mem::size_of::<ShadowPushConstants>() as u32);

        let push_constants = [push_constant_range];

        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .push_constant_ranges(&push_constants);

        let pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };

        // Vertex input: only position (location 0)
        let binding_desc = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<super::mesh::Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);

        let attribute_desc = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(0);

        let binding_descs = [binding_desc];
        let attribute_descs = [attribute_desc];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&binding_descs)
            .vertex_attribute_descriptions(&attribute_descs);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: SHADOW_MAP_SIZE as f32,
            height: SHADOW_MAP_SIZE as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
            },
        };

        let viewports = [viewport];
        let scissors = [scissor];

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(true)
            .line_width(1.0);

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

        let vert_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_module)
            .name(c"main");

        let stages = [vert_stage];

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisample)
            .depth_stencil_state(&depth_stencil)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipelines = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .map_err(|(_, e)| e)?
        };

        unsafe {
            device.destroy_shader_module(vert_module, None);
        }

        Ok((pipeline_layout, pipelines[0]))
    }

    fn create_shader_module(device: &ash::Device, code: &[u8]) -> Result<vk::ShaderModule> {
        let code_aligned = unsafe {
            std::slice::from_raw_parts(code.as_ptr() as *const u32, code.len() / 4)
        };

        let create_info = vk::ShaderModuleCreateInfo::default().code(code_aligned);

        Ok(unsafe { device.create_shader_module(&create_info, None)? })
    }

    /// Get the shadow map image view for sampling in PBR shader.
    pub fn depth_view(&self) -> vk::ImageView {
        self.depth_view
    }

    /// Get the shadow map sampler.
    pub fn sampler(&self) -> vk::Sampler {
        self.sampler
    }

    /// Render shadow map from a directional light's perspective.
    /// Returns command buffer ready to be submitted.
    pub fn render(
        &self,
        cmd: vk::CommandBuffer,
        light_view_proj: glam::Mat4,
        meshes: &[(glam::Mat4, &super::Mesh)],
    ) -> Result<()> {
        let device = self.context.device();

        unsafe {
            // Begin render pass
            let clear_value = vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            };

            let clear_values = [clear_value];

            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: SHADOW_MAP_SIZE,
                        height: SHADOW_MAP_SIZE,
                    },
                })
                .clear_values(&clear_values);

            device.cmd_begin_render_pass(cmd, &rp_begin, vk::SubpassContents::INLINE);

            // Bind pipeline
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            // Set depth bias to reduce shadow acne
            device.cmd_set_depth_bias(cmd, 1.25, 0.0, 1.75);

            // Draw meshes
            for (model_matrix, mesh) in meshes {
                let push_constants = ShadowPushConstants {
                    light_space: light_view_proj,
                    model: *model_matrix,
                };

                let push_bytes = bytemuck::bytes_of(&push_constants);

                device.cmd_push_constants(
                    cmd,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    push_bytes,
                );

                let vertex_buffers = [mesh.vertex_buffer()];
                let offsets = [0];
                device.cmd_bind_vertex_buffers(cmd, 0, &vertex_buffers, &offsets);
                device.cmd_bind_index_buffer(cmd, mesh.index_buffer(), 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed(cmd, mesh.index_count(), 1, 0, 0, 0);
            }

            device.cmd_end_render_pass(cmd);
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowPushConstants {
    light_space: glam::Mat4,
    model: glam::Mat4,
}

impl Drop for ShadowPass {
    fn drop(&mut self) {
        unsafe {
            let device = self.context.device();
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_framebuffer(self.framebuffer, None);
            device.destroy_render_pass(self.render_pass, None);
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.depth_view, None);
            device.destroy_image(self.depth_image, None);
            device.free_memory(self.depth_memory, None);
        }
    }
}