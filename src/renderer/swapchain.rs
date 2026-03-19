use std::sync::Arc;

use anyhow::{Context, Result};
use ash::vk;
use log::info;

use super::context::VulkanContext;

pub struct Swapchain {
    ctx: Arc<VulkanContext>,
    loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    format: vk::Format,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    depth_image: vk::Image,
    depth_image_view: vk::ImageView,
    depth_memory: vk::DeviceMemory,
    dirty: bool,
    new_width: u32,
    new_height: u32,
    vsync: bool,
}

impl Swapchain {
    pub fn new(
        ctx: Arc<VulkanContext>,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self> {
        let loader = ash::khr::swapchain::Device::new(ctx.instance(), ctx.device());
        let mut this = Self {
            ctx,
            loader,
            swapchain: vk::SwapchainKHR::null(),
            images: Vec::new(),
            image_views: Vec::new(),
            format: vk::Format::UNDEFINED,
            extent: vk::Extent2D { width, height },
            render_pass: vk::RenderPass::null(),
            framebuffers: Vec::new(),
            command_pool: vk::CommandPool::null(),
            command_buffers: Vec::new(),
            depth_image: vk::Image::null(),
            depth_image_view: vk::ImageView::null(),
            depth_memory: vk::DeviceMemory::null(),
            dirty: false,
            new_width: width,
            new_height: height,
            vsync,
        };
        this.build(width, height)?;
        Ok(this)
    }

    fn build(&mut self, width: u32, height: u32) -> Result<()> {
        let capabilities = unsafe {
            self.ctx
                .surface_loader()
                .get_physical_device_surface_capabilities(
                    self.ctx.physical_device(),
                    self.ctx.surface(),
                )?
        };

        let formats = unsafe {
            self.ctx
                .surface_loader()
                .get_physical_device_surface_formats(
                    self.ctx.physical_device(),
                    self.ctx.surface(),
                )?
        };

        let present_modes = unsafe {
            self.ctx
                .surface_loader()
                .get_physical_device_surface_present_modes(
                    self.ctx.physical_device(),
                    self.ctx.surface(),
                )?
        };

        let format = Self::choose_format(&formats);
        let present_mode = Self::choose_present_mode(&present_modes, self.vsync);
        let extent = Self::choose_extent(&capabilities, width, height);

        let image_count = (capabilities.min_image_count + 1).min(
            if capabilities.max_image_count > 0 {
                capabilities.max_image_count
            } else {
                u32::MAX
            },
        );

        let indices = self.ctx.queue_indices();
        let (sharing, queue_indices): (vk::SharingMode, Vec<u32>) =
            if indices.graphics != indices.present {
                (
                    vk::SharingMode::CONCURRENT,
                    vec![indices.graphics, indices.present],
                )
            } else {
                (vk::SharingMode::EXCLUSIVE, vec![])
            };

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.ctx.surface())
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(sharing)
            .queue_family_indices(&queue_indices)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(self.swapchain);

        let new_swapchain =
            unsafe { self.loader.create_swapchain(&create_info, None)? };

        // Destroy old
        self.destroy_dependent();
        if self.swapchain != vk::SwapchainKHR::null() {
            unsafe { self.loader.destroy_swapchain(self.swapchain, None) };
        }

        self.swapchain = new_swapchain;
        self.format = format.format;
        self.extent = extent;

        // Get images
        self.images = unsafe { self.loader.get_swapchain_images(self.swapchain)? };

        // Create image views
        self.image_views = self
            .images
            .iter()
            .map(|&img| {
                let info = vk::ImageViewCreateInfo::default()
                    .image(img)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe { self.ctx.device().create_image_view(&info, None) }
            })
            .collect::<Result<_, _>>()?;

        // Create depth resources
        self.create_depth_resources()?;

        // Create render pass
        self.render_pass = self.create_render_pass()?;

        // Create framebuffers
        self.framebuffers = self
            .image_views
            .iter()
            .map(|&view| {
                let attachments = [view, self.depth_image_view];
                let info = vk::FramebufferCreateInfo::default()
                    .render_pass(self.render_pass)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                unsafe { self.ctx.device().create_framebuffer(&info, None) }
            })
            .collect::<Result<_, _>>()?;

        // Create command pool and buffers
        if self.command_pool == vk::CommandPool::null() {
            let pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(self.ctx.queue_indices().graphics);
            self.command_pool =
                unsafe { self.ctx.device().create_command_pool(&pool_info, None)? };
        }

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.images.len() as u32);
        self.command_buffers =
            unsafe { self.ctx.device().allocate_command_buffers(&alloc_info)? };

        info!(
            "Swapchain built: {}x{}, {} images, format {:?}",
            extent.width,
            extent.height,
            self.images.len(),
            format.format
        );

        Ok(())
    }

    fn create_depth_resources(&mut self) -> Result<()> {
        let depth_format = vk::Format::D32_SFLOAT;

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: self.extent.width,
                height: self.extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(depth_format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        unsafe {
            self.depth_image = self.ctx.device().create_image(&image_info, None)?;

            let mem_reqs = self
                .ctx
                .device()
                .get_image_memory_requirements(self.depth_image);

            let mem_index = self.find_memory_type(
                mem_reqs.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(mem_reqs.size)
                .memory_type_index(mem_index);

            self.depth_memory = self.ctx.device().allocate_memory(&alloc_info, None)?;
            self.ctx
                .device()
                .bind_image_memory(self.depth_image, self.depth_memory, 0)?;

            let view_info = vk::ImageViewCreateInfo::default()
                .image(self.depth_image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(depth_format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            self.depth_image_view = self.ctx.device().create_image_view(&view_info, None)?;
        }

        Ok(())
    }

    fn create_render_pass(&self) -> Result<vk::RenderPass> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(self.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let depth_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::D32_SFLOAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let color_ref = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_ref)
            .depth_stencil_attachment(&depth_ref);

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            );

        let attachments = [color_attachment, depth_attachment];
        let subpasses = [subpass];
        let dependencies = [dependency];

        let info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        Ok(unsafe { self.ctx.device().create_render_pass(&info, None)? })
    }

    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        let mem_props = unsafe {
            self.ctx
                .instance()
                .get_physical_device_memory_properties(self.ctx.physical_device())
        };

        for i in 0..mem_props.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(properties)
            {
                return Ok(i);
            }
        }

        anyhow::bail!("No suitable memory type found")
    }

    fn destroy_dependent(&mut self) {
        unsafe {
            let dev = self.ctx.device();

            for &fb in &self.framebuffers {
                dev.destroy_framebuffer(fb, None);
            }
            self.framebuffers.clear();

            if self.render_pass != vk::RenderPass::null() {
                dev.destroy_render_pass(self.render_pass, None);
                self.render_pass = vk::RenderPass::null();
            }

            for &view in &self.image_views {
                dev.destroy_image_view(view, None);
            }
            self.image_views.clear();

            if self.depth_image_view != vk::ImageView::null() {
                dev.destroy_image_view(self.depth_image_view, None);
                self.depth_image_view = vk::ImageView::null();
            }
            if self.depth_image != vk::Image::null() {
                dev.destroy_image(self.depth_image, None);
                self.depth_image = vk::Image::null();
            }
            if self.depth_memory != vk::DeviceMemory::null() {
                dev.free_memory(self.depth_memory, None);
                self.depth_memory = vk::DeviceMemory::null();
            }
        }
    }

    fn choose_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .copied()
            .unwrap_or(formats[0])
    }

    fn choose_present_mode(
        modes: &[vk::PresentModeKHR],
        vsync: bool,
    ) -> vk::PresentModeKHR {
        if vsync {
            return vk::PresentModeKHR::FIFO; // always available
        }
        if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        }
    }

    fn choose_extent(caps: &vk::SurfaceCapabilitiesKHR, w: u32, h: u32) -> vk::Extent2D {
        if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: w.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: h.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        }
    }

    // ─── Public API ───────────────────────────────────────────────────────────

    pub fn acquire_next_image(
        &mut self,
        semaphore: vk::Semaphore,
    ) -> Result<(u32, bool), vk::Result> {
        if self.dirty {
            unsafe { self.ctx.device().device_wait_idle().unwrap() };
            self.build(self.new_width, self.new_height).unwrap();
            self.dirty = false;
        }

        unsafe {
            self.loader
                .acquire_next_image(self.swapchain, u64::MAX, semaphore, vk::Fence::null())
        }
    }

    pub fn present(&self, image_index: u32, wait_sem: vk::Semaphore) -> Result<()> {
        let swapchains = [self.swapchain];
        let indices = [image_index];
        let wait_sems = [wait_sem];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_sems)
            .swapchains(&swapchains)
            .image_indices(&indices);

        unsafe {
            self.loader
                .queue_present(self.ctx.present_queue(), &present_info)?;
        }
        Ok(())
    }

    pub fn recreate(&mut self) -> Result<()> {
        unsafe { self.ctx.device().device_wait_idle()? };
        self.build(self.extent.width, self.extent.height)
    }

    pub fn mark_dirty(&mut self, width: u32, height: u32) {
        self.dirty = true;
        self.new_width = width;
        self.new_height = height;
    }

    pub fn render_pass(&self) -> vk::RenderPass { self.render_pass }
    pub fn extent(&self) -> vk::Extent2D { self.extent }
    pub fn command_buffer(&self, index: usize) -> vk::CommandBuffer {
        self.command_buffers[index]
    }
    pub fn framebuffer(&self, index: usize) -> vk::Framebuffer {
        self.framebuffers[index]
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            let _ = self.ctx.device().device_wait_idle();
            self.destroy_dependent();

            if self.command_pool != vk::CommandPool::null() {
                self.ctx
                    .device()
                    .destroy_command_pool(self.command_pool, None);
            }

            if self.swapchain != vk::SwapchainKHR::null() {
                self.loader.destroy_swapchain(self.swapchain, None);
            }
        }
    }
}
