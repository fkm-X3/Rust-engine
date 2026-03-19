use std::ffi::{CStr, CString};

use anyhow::{bail, Context, Result};
use ash::{vk, Device, Entry, Instance};
use log::{debug, info, warn};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::core::EngineConfig;

const VALIDATION_LAYERS: &[&str] = &["VK_LAYER_KHRONOS_validation"];

const DEVICE_EXTENSIONS: &[&CStr] = &[
    ash::khr::swapchain::NAME,
    ash::khr::dynamic_rendering::NAME,
];

/// Queue family indices found during physical device selection.
#[derive(Debug, Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub present: u32,
    pub transfer: u32,
}

/// Owns the Vulkan entry, instance, surface, device, queues, and allocator.
pub struct VulkanContext {
    _entry: Entry,
    instance: Instance,
    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device,
    queues: QueueFamilyIndices,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    transfer_queue: vk::Queue,
    debug_messenger: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}

impl VulkanContext {
    pub fn new(window: &Window, config: &EngineConfig) -> Result<Self> {
        let entry = unsafe { Entry::load()? };

        let instance = Self::create_instance(&entry, window, config)?;

        let debug_messenger = if config.enable_validation_layers {
            Some(Self::setup_debug_messenger(&entry, &instance)?)
        } else {
            None
        };

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };

        let physical_device =
            Self::pick_physical_device(&instance, &surface_loader, surface)?;

        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) };
        info!("Selected GPU: {:?}", name);

        let queue_indices =
            Self::find_queue_families(&instance, physical_device, &surface_loader, surface)?;

        let device = Self::create_logical_device(
            &instance,
            physical_device,
            queue_indices,
            config,
        )?;

        let graphics_queue =
            unsafe { device.get_device_queue(queue_indices.graphics, 0) };
        let present_queue =
            unsafe { device.get_device_queue(queue_indices.present, 0) };
        let transfer_queue =
            unsafe { device.get_device_queue(queue_indices.transfer, 0) };

        Ok(Self {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            queues: queue_indices,
            graphics_queue,
            present_queue,
            transfer_queue,
            debug_messenger,
        })
    }

    fn create_instance(
        entry: &Entry,
        window: &Window,
        config: &EngineConfig,
    ) -> Result<Instance> {
        let app_name = CString::new(config.window_title.as_str())?;
        let engine_name = CString::new("VKEngine")?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_3);

        // Surface extensions required by the platform
        let mut extensions =
            ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())
                .context("Failed to enumerate required extensions")?
                .to_vec();

        if config.enable_validation_layers {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        let layers: Vec<CString> = if config.enable_validation_layers {
            Self::check_validation_layers(entry)?;
            VALIDATION_LAYERS
                .iter()
                .map(|&s| CString::new(s).unwrap())
                .collect()
        } else {
            Vec::new()
        };
        let layer_ptrs: Vec<*const i8> = layers.iter().map(|s| s.as_ptr()).collect();

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layer_ptrs);

        Ok(unsafe { entry.create_instance(&create_info, None)? })
    }

    fn check_validation_layers(entry: &Entry) -> Result<()> {
        let available: Vec<String> = unsafe {
            entry
                .enumerate_instance_layer_properties()?
                .iter()
                .map(|l| {
                    CStr::from_ptr(l.layer_name.as_ptr())
                        .to_string_lossy()
                        .to_string()
                })
                .collect()
        };

        for &required in VALIDATION_LAYERS {
            if !available.iter().any(|a| a == required) {
                bail!("Validation layer not found: {}", required);
            }
        }
        Ok(())
    }

    fn setup_debug_messenger(
        entry: &Entry,
        instance: &Instance,
    ) -> Result<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)> {
        let loader = ash::ext::debug_utils::Instance::new(entry, instance);

        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(debug_callback));

        let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };

        Ok((loader, messenger))
    }

    fn pick_physical_device(
        instance: &Instance,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Result<vk::PhysicalDevice> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        if devices.is_empty() {
            bail!("No Vulkan-capable GPU found");
        }

        // Prefer a discrete GPU; fall back to integrated
        let mut best: Option<(vk::PhysicalDevice, u32)> = None;

        for dev in devices {
            let props = unsafe { instance.get_physical_device_properties(dev) };
            let score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 100,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 50,
                _ => 1,
            };

            // Check queue families and extension support
            if Self::find_queue_families(instance, dev, surface_loader, surface).is_err() {
                continue;
            }
            if !Self::check_device_extensions(instance, dev) {
                continue;
            }

            if best.is_none() || score > best.unwrap().1 {
                best = Some((dev, score));
            }
        }

        best.map(|(d, _)| d)
            .context("No suitable GPU found")
    }

    fn check_device_extensions(instance: &Instance, device: vk::PhysicalDevice) -> bool {
        let available = unsafe {
            instance
                .enumerate_device_extension_properties(device)
                .unwrap_or_default()
        };

        DEVICE_EXTENSIONS.iter().all(|&required| {
            available.iter().any(|ext| unsafe {
                CStr::from_ptr(ext.extension_name.as_ptr()) == required
            })
        })
    }

    fn find_queue_families(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface_loader: &ash::khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Result<QueueFamilyIndices> {
        let families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };

        let mut graphics = None;
        let mut present = None;
        let mut transfer = None;

        for (i, family) in families.iter().enumerate() {
            let i = i as u32;

            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics = Some(i);
            }

            // Prefer a dedicated transfer queue
            if family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            {
                transfer = Some(i);
            }

            let present_support = unsafe {
                surface_loader
                    .get_physical_device_surface_support(device, i, surface)
                    .unwrap_or(false)
            };
            if present_support {
                present = Some(i);
            }
        }

        // Fall back: use graphics queue for transfer if no dedicated one
        let transfer = transfer.or(graphics);

        match (graphics, present, transfer) {
            (Some(g), Some(p), Some(t)) => Ok(QueueFamilyIndices {
                graphics: g,
                present: p,
                transfer: t,
            }),
            _ => bail!("Required queue families not found"),
        }
    }

    fn create_logical_device(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        queues: QueueFamilyIndices,
        config: &EngineConfig,
    ) -> Result<Device> {
        let mut unique = std::collections::HashSet::new();
        unique.insert(queues.graphics);
        unique.insert(queues.present);
        unique.insert(queues.transfer);

        let priority = [1.0_f32];
        let queue_infos: Vec<vk::DeviceQueueCreateInfo> = unique
            .iter()
            .map(|&index| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(index)
                    .queue_priorities(&priority)
            })
            .collect();

        let ext_ptrs: Vec<*const i8> =
            DEVICE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();

        let layers: Vec<CString> = if config.enable_validation_layers {
            VALIDATION_LAYERS
                .iter()
                .map(|&s| CString::new(s).unwrap())
                .collect()
        } else {
            Vec::new()
        };
        let layer_ptrs: Vec<*const i8> = layers.iter().map(|s| s.as_ptr()).collect();

        let mut features13 = vk::PhysicalDeviceVulkan13Features::default()
            .dynamic_rendering(true)
            .synchronization2(true);

        let mut features12 = vk::PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(true)
            .descriptor_indexing(true);

        let features = vk::PhysicalDeviceFeatures::default()
            .sampler_anisotropy(true)
            .fill_mode_non_solid(true)
            .wide_lines(true);

        let mut create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&ext_ptrs)
            .enabled_features(&features)
            .push_next(&mut features12)
            .push_next(&mut features13);
        
        // Note: enabled_layer_names is deprecated in Vulkan 1.3+
        // Validation layers are now controlled at instance level only
        if !layer_ptrs.is_empty() {
            #[allow(deprecated)]
            { create_info = create_info.enabled_layer_names(&layer_ptrs); }
        }

        Ok(unsafe { instance.create_device(physical_device, &create_info, None)? })
    }

    // ─── Accessors ────────────────────────────────────────────────────────────

    pub fn instance(&self) -> &Instance { &self.instance }
    pub fn device(&self) -> &Device { &self.device }
    pub fn physical_device(&self) -> vk::PhysicalDevice { self.physical_device }
    pub fn surface(&self) -> vk::SurfaceKHR { self.surface }
    pub fn surface_loader(&self) -> &ash::khr::surface::Instance { &self.surface_loader }
    pub fn graphics_queue(&self) -> vk::Queue { self.graphics_queue }
    pub fn present_queue(&self) -> vk::Queue { self.present_queue }
    pub fn transfer_queue(&self) -> vk::Queue { self.transfer_queue }
    pub fn queue_indices(&self) -> QueueFamilyIndices { self.queues }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);

            self.surface_loader.destroy_surface(self.surface, None);

            if let Some((loader, messenger)) = self.debug_messenger.take() {
                loader.destroy_debug_utils_messenger(messenger, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let msg = CStr::from_ptr((*data).p_message).to_string_lossy();

    match severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("[Vulkan] {}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("[Vulkan] {}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => debug!("[Vulkan] {}", msg),
        _ => {}
    }

    vk::FALSE
}
