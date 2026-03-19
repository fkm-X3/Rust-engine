use anyhow::{Context as _, Result};
use ash::vk;
use std::sync::Arc;

use super::context::VulkanContext;

/// Helper for GPU buffer creation and memory upload via staging buffers.
pub struct BufferUploader {
    context: Arc<VulkanContext>,
    command_pool: vk::CommandPool,
}

impl BufferUploader {
    /// Create a new buffer uploader with a dedicated transfer command pool.
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let device = context.device();
        let queue_indices = context.queue_indices();

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_indices.transfer)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);

        let command_pool = unsafe {
            device
                .create_command_pool(&pool_info, None)
                .context("Failed to create transfer command pool")?
        };

        Ok(Self {
            context,
            command_pool,
        })
    }

    /// Create a buffer with the specified usage and memory properties.
    pub fn create_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        memory_properties: vk::MemoryPropertyFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory)> {
        let device = self.context.device();

        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe {
            device
                .create_buffer(&buffer_info, None)
                .context("Failed to create buffer")?
        };

        let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let memory_type_index = self
            .find_memory_type(mem_requirements.memory_type_bits, memory_properties)
            .context("Failed to find suitable memory type")?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type_index);

        let memory = unsafe {
            device
                .allocate_memory(&alloc_info, None)
                .context("Failed to allocate buffer memory")?
        };

        unsafe {
            device
                .bind_buffer_memory(buffer, memory, 0)
                .context("Failed to bind buffer memory")?;
        }

        Ok((buffer, memory))
    }

    /// Upload data to a device-local buffer via a staging buffer.
    /// Returns (device_buffer, device_memory) ready for use on GPU.
    pub fn upload_buffer<T: Copy>(
        &self,
        data: &[T],
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory)> {
        let device = self.context.device();
        let size = (std::mem::size_of::<T>() * data.len()) as vk::DeviceSize;

        // Create staging buffer (CPU-visible)
        let (staging_buffer, staging_memory) = self.create_buffer(
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        // Map and copy data to staging buffer
        unsafe {
            let ptr = device
                .map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty())
                .context("Failed to map staging buffer memory")?;

            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut T, data.len());

            device.unmap_memory(staging_memory);
        }

        // Create device-local buffer
        let (device_buffer, device_memory) = self.create_buffer(
            size,
            vk::BufferUsageFlags::TRANSFER_DST | usage,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        // Copy staging → device buffer
        self.copy_buffer(staging_buffer, device_buffer, size)?;

        // Cleanup staging resources
        unsafe {
            device.destroy_buffer(staging_buffer, None);
            device.free_memory(staging_memory, None);
        }

        Ok((device_buffer, device_memory))
    }

    /// Copy data from one buffer to another using a transfer command.
    fn copy_buffer(
        &self,
        src: vk::Buffer,
        dst: vk::Buffer,
        size: vk::DeviceSize,
    ) -> Result<()> {
        let device = self.context.device();

        // Allocate command buffer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer = unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .context("Failed to allocate transfer command buffer")?[0]
        };

        // Record copy command
        let begin_info =
            vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            device
                .begin_command_buffer(command_buffer, &begin_info)
                .context("Failed to begin command buffer")?;

            let copy_region = vk::BufferCopy::default().size(size);
            device.cmd_copy_buffer(command_buffer, src, dst, &[copy_region]);

            device
                .end_command_buffer(command_buffer)
                .context("Failed to end command buffer")?;
        }

        // Submit and wait
        let cmd_buffers = [command_buffer];
        let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_buffers);

        unsafe {
            device
                .queue_submit(self.context.transfer_queue(), &[submit_info], vk::Fence::null())
                .context("Failed to submit transfer command")?;

            device
                .queue_wait_idle(self.context.transfer_queue())
                .context("Failed to wait for transfer queue")?;

            device.free_command_buffers(self.command_pool, &[command_buffer]);
        }

        Ok(())
    }

    /// Find a memory type that matches the requirements.
    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        let mem_properties = unsafe {
            self.context
                .instance()
                .get_physical_device_memory_properties(self.context.physical_device())
        };

        for i in 0..mem_properties.memory_type_count {
            let type_supported = (type_filter & (1 << i)) != 0;
            let properties_match =
                mem_properties.memory_types[i as usize].property_flags.contains(properties);

            if type_supported && properties_match {
                return Ok(i);
            }
        }

        anyhow::bail!("Failed to find suitable memory type")
    }
}

impl Drop for BufferUploader {
    fn drop(&mut self) {
        unsafe {
            self.context
                .device()
                .destroy_command_pool(self.command_pool, None);
        }
    }
}
