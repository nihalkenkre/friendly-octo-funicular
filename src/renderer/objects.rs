use std::ffi::CStr;

use ash::{
    ext::{self},
    khr,
    vk::{self},
};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct Instance {
    pub instance: ash::Instance,
}

impl Instance {
    pub fn new(extensions: &[*const i8]) -> Self {
        let ash_entry = ash::Entry::linked();

        let mut all_exts = vec![];

        if cfg!(debug_assertions) {
            all_exts.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        unsafe {
            for ext in extensions {
                all_exts.push(CStr::from_ptr(*ext).as_ptr());
            }

            let instance = ash_entry
                .create_instance(
                    &vk::InstanceCreateInfo::default()
                        .application_info(
                            &vk::ApplicationInfo::default()
                                .api_version(vk::make_api_version(0, 1, 3, 0)),
                        )
                        .enabled_extension_names(&all_exts),
                    None,
                )
                .unwrap();

            Self { instance }
        }
    }

    pub fn create_device_and_queues(
        &self,
        physical_device_data: &PhysicalDeviceData,
    ) -> (ash::Device, vk::Queue, vk::Queue, vk::Queue) {
        let req_exts = [
            vk::KHR_SWAPCHAIN_NAME.as_ptr() as *const i8,
            vk::KHR_DEFERRED_HOST_OPERATIONS_NAME.as_ptr() as *const i8,
            vk::EXT_ROBUSTNESS2_NAME.as_ptr() as *const i8,
            vk::EXT_EXTENDED_DYNAMIC_STATE3_NAME.as_ptr() as *const i8,
        ];

        let mut dyn_rend_feats =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
        let mut sync2_feats =
            vk::PhysicalDeviceSynchronization2Features::default().synchronization2(true);
        let mut feats11 =
            vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
        let mut feats12 = vk::PhysicalDeviceVulkan12Features::default()
            .uniform_and_storage_buffer8_bit_access(true)
            .shader_int8(true)
            .timeline_semaphore(true)
            .buffer_device_address(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .runtime_descriptor_array(true)
            .storage_push_constant8(true);
        let mut feats2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut dyn_rend_feats)
            .push_next(&mut sync2_feats)
            .push_next(&mut feats11)
            .push_next(&mut feats12);

        // Go through the retrieved queue family indices and create the device queue create infos.
        // If there are more than one queue family share the same index, then more than one queue is requested in the queue create info
        let mut d_q_cis: Vec<vk::DeviceQueueCreateInfo> = Vec::new();

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.graphics_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.graphics_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.compute_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.compute_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.transfer_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.transfer_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        for d_q_ci in &mut d_q_cis {
            let priorities: Vec<f32> = vec![1f32; d_q_ci.queue_count as usize];

            d_q_ci.p_queue_priorities = priorities.as_ptr();
        }

        let create_info = vk::DeviceCreateInfo::default()
            .enabled_extension_names(&req_exts)
            .queue_create_infos(&d_q_cis)
            .push_next(&mut feats2);

        unsafe {
            let device = self
                .instance
                .create_device(physical_device_data.physical_device, &create_info, None)
                .unwrap();

            // Loop through the device queue create infos and retrieve the queues.
            let graphics_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.graphics_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.graphics_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            let compute_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.compute_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.compute_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            let transfer_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.transfer_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.transfer_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            (device, graphics_queue, compute_queue, transfer_queue)
        }
    }

    pub fn destroy(self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

pub struct PhysicalDeviceData<'instance_lifetime> {
    pub physical_device: vk::PhysicalDevice,

    pub graphics_queue_family_index: u32,
    pub transfer_queue_family_index: u32,
    pub compute_queue_family_index: u32,

    pub queue_family_indices: Vec<u32>,
    pub properties: vk::PhysicalDeviceProperties2<'instance_lifetime>,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties2<'instance_lifetime>,
}

impl<'instance_lifetime> PhysicalDeviceData<'instance_lifetime> {
    pub fn new(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: &vk::SurfaceKHR,
    ) -> Self {
        unsafe {
            let mut properties = vk::PhysicalDeviceProperties2::default();

            // find the first discrete or intergrated GPU, simple use case
            let physical_device = match instance
                .enumerate_physical_devices()
                .unwrap()
                .into_iter()
                .find(|physical_device| {
                    instance.get_physical_device_properties2(*physical_device, &mut properties);
                    match properties.properties.device_type {
                        vk::PhysicalDeviceType::DISCRETE_GPU => true,
                        vk::PhysicalDeviceType::INTEGRATED_GPU => true,
                        _ => false,
                    }
                }) {
                Some(x) => x,
                None => {
                    println!("No GPUs on the system. Exiting...");
                    vk::PhysicalDevice::null()
                }
            };

            let mut queue_family_properties =
                vec![
                    vk::QueueFamilyProperties2::default();
                    instance.get_physical_device_queue_family_properties2_len(physical_device)
                ];

            instance.get_physical_device_queue_family_properties2(
                physical_device,
                &mut queue_family_properties,
            );

            // look for queue family with GRAPHICS_BIT set
            let graphics_queue_family_index = queue_family_properties
                .iter()
                .enumerate()
                .find(|(index, qfp)| {
                    surface_loader
                        .get_physical_device_surface_support(
                            physical_device,
                            *index as u32,
                            *surface,
                        )
                        .unwrap()
                        && qfp
                            .queue_family_properties
                            .queue_flags
                            .contains(vk::QueueFlags::GRAPHICS)
                })
                .unwrap()
                .0 as u32;

            // look for queue family with only TRANSFER_BIT and !GRAPHICS_BIT set
            // if no queue found then look for any queue with TRANSFER_BIT set.
            let transfer_queue_family_index =
                match queue_family_properties.iter().enumerate().find(|(_, qfp)| {
                    qfp.queue_family_properties
                        .queue_flags
                        .contains(vk::QueueFlags::TRANSFER)
                        && !qfp
                            .queue_family_properties
                            .queue_flags
                            .contains(vk::QueueFlags::GRAPHICS)
                }) {
                    Some(x) => x.0 as u32,
                    None => {
                        queue_family_properties
                            .iter()
                            .enumerate()
                            .find(|(_, qfp)| {
                                qfp.queue_family_properties
                                    .queue_flags
                                    .contains(vk::QueueFlags::TRANSFER)
                            })
                            .unwrap()
                            .0 as u32
                    }
                };

            // look for queue family with only COMPUTE_BIT and !GRAPHICS_BIT set
            // if no queue found then look for any queue with COMPUTE_BIT set.
            let compute_queue_family_index =
                match queue_family_properties.iter().enumerate().find(|(_, qfp)| {
                    qfp.queue_family_properties
                        .queue_flags
                        .contains(vk::QueueFlags::COMPUTE)
                        && !qfp
                            .queue_family_properties
                            .queue_flags
                            .contains(vk::QueueFlags::GRAPHICS)
                }) {
                    Some(x) => x.0 as u32,
                    None => {
                        queue_family_properties
                            .iter()
                            .enumerate()
                            .find(|(_, qfp)| {
                                qfp.queue_family_properties
                                    .queue_flags
                                    .contains(vk::QueueFlags::COMPUTE)
                            })
                            .unwrap()
                            .0 as u32
                    }
                };

            let mut memory_properties = vk::PhysicalDeviceMemoryProperties2::default();
            instance
                .get_physical_device_memory_properties2(physical_device, &mut memory_properties);

            Self {
                physical_device,
                graphics_queue_family_index,
                transfer_queue_family_index,
                compute_queue_family_index,
                queue_family_indices: vec![
                    graphics_queue_family_index,
                    compute_queue_family_index,
                    transfer_queue_family_index,
                ],
                properties,
                memory_properties,
            }
        }
    }
}

pub struct SurfaceData<'instance_lifetime> {
    pub capabilities: vk::SurfaceCapabilities2KHR<'instance_lifetime>,
    pub surface_format: vk::SurfaceFormat2KHR<'instance_lifetime>,
    pub present_mode: vk::PresentModeKHR,
    pub surface: vk::SurfaceKHR,
    pub surface_loader: khr::surface::Instance,
}

impl<'instance_lifetime> SurfaceData<'instance_lifetime> {
    pub fn new(instance: &ash::Instance, window: &winit::window::Window) -> Self {
        unsafe {
            let surface = ash_window::create_surface(
                &ash::Entry::linked(),
                instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )
            .unwrap();

            Self {
                surface,
                capabilities: vk::SurfaceCapabilities2KHR::default(),
                surface_format: vk::SurfaceFormat2KHR::default(),
                present_mode: vk::PresentModeKHR::default(),
                surface_loader: khr::surface::Instance::new(&ash::Entry::linked(), instance),
            }
        }
    }

    pub fn populate_data(&mut self, physical_device: &vk::PhysicalDevice) {
        unsafe {
            self.capabilities = vk::SurfaceCapabilities2KHR::default().surface_capabilities(
                self.surface_loader
                    .get_physical_device_surface_capabilities(*physical_device, self.surface)
                    .unwrap(),
            );

            self.surface_format = match self
                .surface_loader
                .get_physical_device_surface_formats(*physical_device, self.surface)
                .unwrap()
                .iter()
                .find(|&surface_format| {
                    surface_format.format == vk::Format::R8G8B8A8_SRGB
                        && surface_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                }) {
                Some(x) => vk::SurfaceFormat2KHR::default().surface_format(*x),
                None => vk::SurfaceFormat2KHR::default(),
            };

            self.present_mode = match self
                .surface_loader
                .get_physical_device_surface_present_modes(*physical_device, self.surface)
                .unwrap()
                .iter()
                .find(|&present_mode| *present_mode == vk::PresentModeKHR::MAILBOX)
            {
                Some(x) => *x,
                None => vk::PresentModeKHR::FIFO,
            };
        }
    }

    pub fn destroy(&self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}

pub struct DebugUtils {
    device: ext::debug_utils::Device,
}

impl DebugUtils {
    pub fn new(instance: &ash::Instance, device: &ash::Device) -> Self {
        Self {
            device: ext::debug_utils::Device::new(&instance, &device),
        }
    }

    pub fn set_object_name<T: vk::Handle>(&self, handle: T, name: String) {
        unsafe {
            let name_info = vk::DebugUtilsObjectNameInfoEXT::default()
                .object_handle(handle)
                .object_name(CStr::from_ptr((name + "\0").as_ptr() as *const i8));

            self.device.set_debug_utils_object_name(&name_info).unwrap();
        }
    }
}

pub struct DeviceData {
    pub device: ash::Device,

    pub graphics_queue: vk::Queue,
    pub compute_queue: vk::Queue,
    pub transfer_queue: vk::Queue,
}

impl DeviceData {
    pub fn new(instance: &ash::Instance, physical_device_data: &PhysicalDeviceData) -> Self {
        let req_exts = [
            vk::KHR_SWAPCHAIN_NAME.as_ptr() as *const i8,
            vk::KHR_DEFERRED_HOST_OPERATIONS_NAME.as_ptr() as *const i8,
            vk::EXT_ROBUSTNESS2_NAME.as_ptr() as *const i8,
            vk::EXT_EXTENDED_DYNAMIC_STATE3_NAME.as_ptr() as *const i8,
        ];

        let mut dyn_rend_feats =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
        let mut sync2_feats =
            vk::PhysicalDeviceSynchronization2Features::default().synchronization2(true);
        let mut feats11 =
            vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
        let mut feats12 = vk::PhysicalDeviceVulkan12Features::default()
            .uniform_and_storage_buffer8_bit_access(true)
            .shader_int8(true)
            .timeline_semaphore(true)
            .buffer_device_address(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .runtime_descriptor_array(true)
            .storage_push_constant8(true);
        let mut feats2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut dyn_rend_feats)
            .push_next(&mut sync2_feats)
            .push_next(&mut feats11)
            .push_next(&mut feats12);

        // Go through the retrieved queue family indices and create the device queue create infos.
        // If there are more than one queue family share the same index, then more than one queue is requested in the queue create info
        let mut d_q_cis: Vec<vk::DeviceQueueCreateInfo> = Vec::new();

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.graphics_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.graphics_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.compute_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.compute_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        match d_q_cis.iter_mut().find(|d_q_ci| {
            d_q_ci.queue_family_index == physical_device_data.transfer_queue_family_index
        }) {
            Some(x) => {
                x.queue_count += 1;
            }
            None => {
                d_q_cis.push(vk::DeviceQueueCreateInfo {
                    queue_family_index: physical_device_data.transfer_queue_family_index,
                    queue_count: 1,
                    ..Default::default()
                });
            }
        }

        for d_q_ci in &mut d_q_cis {
            let priorities: Vec<f32> = vec![1f32; d_q_ci.queue_count as usize];

            d_q_ci.p_queue_priorities = priorities.as_ptr();
        }

        let create_info = vk::DeviceCreateInfo::default()
            .enabled_extension_names(&req_exts)
            .queue_create_infos(&d_q_cis)
            .push_next(&mut feats2);

        unsafe {
            let device = instance
                .create_device(physical_device_data.physical_device, &create_info, None)
                .unwrap();

            // Loop through the device queue create infos and retrieve the queues.
            let graphics_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.graphics_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.graphics_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            let compute_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.compute_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.compute_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            let transfer_queue = match d_q_cis.iter().enumerate().find(|&d_q_ci| {
                d_q_ci.1.queue_family_index == physical_device_data.transfer_queue_family_index
            }) {
                Some(x) => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: physical_device_data.transfer_queue_family_index,
                    queue_index: x.1.queue_count - 1, // hardcoded since we need just one queue
                    ..Default::default()
                }),
                None => device.get_device_queue2(&vk::DeviceQueueInfo2 {
                    queue_family_index: 0,
                    queue_index: 0,
                    ..Default::default()
                }),
            };

            Self {
                device,
                graphics_queue,
                compute_queue,
                transfer_queue,
            }
        }
    }

    pub fn destroy(&self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

pub struct SwapchainData {
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_loader: khr::swapchain::Device,

    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,

    pub acq_sig_sems: Vec<vk::Semaphore>,
    pub prsnt_wait_sems: Vec<vk::Semaphore>,

    pub frames_in_flight: u32,

    image_index: u32,
}

impl SwapchainData {
    pub fn new(
        instance: &ash::Instance,
        device: &ash::Device,
        surface: &SurfaceData,
        queue_family_indices: &[u32],
        debug_utils: &DebugUtils,
        extra_frames_in_flight: u32,
    ) -> Self {
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.surface)
            .min_image_count(surface.capabilities.surface_capabilities.min_image_count)
            .image_format(surface.surface_format.surface_format.format)
            .image_color_space(surface.surface_format.surface_format.color_space)
            .image_extent(surface.capabilities.surface_capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .queue_family_indices(queue_family_indices)
            .pre_transform(surface.capabilities.surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(surface.present_mode);

        let mut iv_ci = vk::ImageViewCreateInfo::default()
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(surface.surface_format.surface_format.format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .level_count(1),
            );

        let mut image_views = Vec::new();

        let swapchain_loader = ash::khr::swapchain::Device::new(instance, device);
        let swapchain;
        let mut images = vec![];
        let mut acq_sig_sems = vec![];
        let mut prsnt_wait_sems = vec![];

        let frames_in_flight =
            surface.capabilities.surface_capabilities.min_image_count + extra_frames_in_flight;

        unsafe {
            swapchain = swapchain_loader
                .create_swapchain(&create_info, None)
                .unwrap();
            images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

            for image in images.iter().as_ref() {
                iv_ci = iv_ci.image(*image);

                image_views.push(device.create_image_view(&iv_ci, None).unwrap());
            }

            for _ in 0..frames_in_flight {
                acq_sig_sems.push(
                    device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .unwrap(),
                );
                prsnt_wait_sems.push(
                    device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .unwrap(),
                );
            }

            if cfg!(debug_assertions) {
                debug_utils.set_object_name(swapchain, "swapchain".to_string());

                for (index, image) in images.iter().enumerate() {
                    let name = "swapchain image ".to_owned() + index.to_string().as_str();
                    debug_utils.set_object_name(*image, name);
                }

                for (index, image_view) in image_views.iter().enumerate() {
                    let name = "swapchain image view ".to_owned() + index.to_string().as_str();
                    debug_utils.set_object_name(*image_view, name);
                }

                for (index, semaphore) in acq_sig_sems.iter().enumerate() {
                    let name = "swapchain acq sig sem ".to_owned() + index.to_string().as_str();
                    debug_utils.set_object_name(*semaphore, name);
                }

                for (index, semaphore) in acq_sig_sems.iter().enumerate() {
                    let name = "swapchain acq sig sem ".to_owned() + index.to_string().as_str();
                    debug_utils.set_object_name(*semaphore, name);
                }

                for (index, semaphore) in prsnt_wait_sems.iter().enumerate() {
                    let name = "swapchain prsnt wait sem ".to_owned() + index.to_string().as_str();
                    debug_utils.set_object_name(*semaphore, name);
                }
            }
        }

        let image_index = 0;

        Self {
            swapchain_loader,
            swapchain,
            images,
            image_views,
            acq_sig_sems,
            prsnt_wait_sems,
            image_index,
            frames_in_flight,
        }
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            for image_view in &self.image_views {
                device.destroy_image_view(*image_view, None);
            }

            for semaphore in &self.acq_sig_sems {
                device.destroy_semaphore(*semaphore, None);
            }

            for semaphore in &self.prsnt_wait_sems {
                device.destroy_semaphore(*semaphore, None);
            }
        }
    }

    pub fn get_acq_sig_sem(&self) -> vk::Semaphore {
        *self.acq_sig_sems.get(self.image_index as usize).unwrap()
    }

    pub fn get_prsnt_wait_sem(&self) -> vk::Semaphore {
        *self.prsnt_wait_sems.get(self.image_index as usize).unwrap()
    }

    pub fn get_image(&self, image_index: u32) -> vk::Image {
        let image = *self.images.get(image_index as usize).unwrap();

        image
    }

    pub fn get_image_view(&self, image_index: u32) -> vk::ImageView {
        let image_view = *self.image_views.get(image_index as usize).unwrap();

        image_view
    }

    pub fn next_index(&mut self) {
        self.image_index = (self.image_index + 1) % self.frames_in_flight;
    }
}

pub struct Task {
    pub command_pool: vk::CommandPool,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub semaphores: Vec<vk::Semaphore>,
    pub semaphore_values: Vec<u64>,
    pub queue: vk::Queue,
    pub queue_family_index: u32,
    pub count: usize,
    pub current_index: usize,
}

impl Task {
    pub fn create(
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        count: usize,
        debug_utils: &DebugUtils,
        name: String,
    ) -> Self {
        let semaphore_values = vec![1u64; count];

        unsafe {
            let mut sem_tl_ci = vk::SemaphoreTypeCreateInfo::default()
                .initial_value(0)
                .semaphore_type(vk::SemaphoreType::TIMELINE);

            let mut semaphores = vec![];
            for i in 0..count {
                let semaphore = device
                    .create_semaphore(
                        &vk::SemaphoreCreateInfo::default().push_next(&mut sem_tl_ci),
                        None,
                    )
                    .unwrap();

                device
                    .signal_semaphore(
                        &vk::SemaphoreSignalInfo::default()
                            .semaphore(semaphore)
                            .value(1),
                    )
                    .unwrap();

                semaphores.push(semaphore);

                if cfg!(debug_assertions) {
                    let name_str = name.clone() + " semaphore " + &i.to_string();
                    debug_utils.set_object_name(semaphore, name_str);
                }
            }

            let command_pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .unwrap();

            if cfg!(debug_assertions) {
                let mut n = name.clone();
                n.push_str(" command pool");
                debug_utils.set_object_name(command_pool, n);
            }

            let command_buffers = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_buffer_count(count as u32)
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY),
                )
                .unwrap();
            if cfg!(debug_assertions) {
                for (i, command_buffer) in command_buffers.iter().enumerate() {
                    let mut n = name.clone();
                    n.push_str(&(" command buffer ".to_owned() + i.to_string().as_str()));
                    debug_utils.set_object_name(*command_buffer, n);
                }
            }

            Self {
                command_pool,
                command_buffers,
                semaphores,
                semaphore_values,
                queue,
                queue_family_index,
                count,
                current_index: 0,
            }
        }
    }

    pub fn start_task(&self, device: &ash::Device, command_buffer: vk::CommandBuffer) {
        unsafe {
            device
                .begin_command_buffer(
                    command_buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .unwrap();
        }
    }

    pub fn end_task(&self, device: &ash::Device) {
        unsafe {
            device
                .end_command_buffer(*self.command_buffers.get(self.current_index).unwrap())
                .unwrap();
        }
    }

    pub fn submit_task(
        &self,
        device: &ash::Device,
        wait_stage_mask: vk::PipelineStageFlags2,
        signal_stage_mask: vk::PipelineStageFlags2,
        wait_sem_infos: &[vk::SemaphoreSubmitInfo],
        sig_sem_infos: &[vk::SemaphoreSubmitInfo],
    ) {
        let (semaphore, value) = self.get_semaphore_and_value();

        let wsi = wait_sem_infos.to_vec();

        let mut ssi = sig_sem_infos.to_vec();
        ssi.push(
            vk::SemaphoreSubmitInfo::default()
                .semaphore(semaphore)
                .value(value + 1)
                .stage_mask(signal_stage_mask),
        );

        unsafe {
            device
                .queue_submit2(
                    self.queue,
                    &[vk::SubmitInfo2::default()
                        .command_buffer_infos(&[vk::CommandBufferSubmitInfo::default()
                            .command_buffer(self.get_command_buffer())])
                        .wait_semaphore_infos(&wsi)
                        .signal_semaphore_infos(&ssi)],
                    vk::Fence::null(),
                )
                .unwrap();
        }
    }

    pub fn get_command_buffer(&self) -> vk::CommandBuffer {
        *self.command_buffers.get(self.current_index).unwrap()
    }

    pub fn get_semaphore_and_value(&self) -> (vk::Semaphore, u64) {
        (
            *self.semaphores.get(self.current_index).unwrap(),
            *self.semaphore_values.get(self.current_index).unwrap(),
        )
    }

    pub fn next_frame(&mut self) {
        self.semaphore_values[self.current_index] += 1;
        self.current_index = (self.current_index + 1) % self.count;
    }

    pub fn destroy(&self, device: &ash::Device) {
        unsafe {
            device.destroy_command_pool(self.command_pool, None);

            for sem in self.semaphores.iter() {
                device.destroy_semaphore(*sem, None);
            }
        }
    }
}

pub struct TransferTask {
    pub task: Task,
}

impl TransferTask {
    pub fn create(
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        count: usize,
        debug_utils: &DebugUtils,
        name: String,
    ) -> Self {
        Self {
            task: Task::create(device, queue, queue_family_index, count, debug_utils, name),
        }
    }

    pub fn start_task(&self, device: &ash::Device, command_buffer: vk::CommandBuffer) {
        self.task.start_task(device, command_buffer);
    }

    pub fn change_image_layout(
        &self,
        device: &ash::Device,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage_mask: vk::PipelineStageFlags2,
        dst_stage_mask: vk::PipelineStageFlags2,
        src_access_mask: vk::AccessFlags2,
        dst_access_mask: vk::AccessFlags2,
    ) {
        unsafe {
            device.cmd_pipeline_barrier2(
                *self
                    .task
                    .command_buffers
                    .get(self.task.current_index)
                    .unwrap(),
                &vk::DependencyInfo::default().image_memory_barriers(&[
                    vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(src_stage_mask)
                        .dst_stage_mask(dst_stage_mask)
                        .src_access_mask(src_access_mask)
                        .dst_access_mask(dst_access_mask)
                        .old_layout(old_layout)
                        .new_layout(new_layout)
                        .image(image)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .layer_count(1)
                                .level_count(1)
                                .aspect_mask(vk::ImageAspectFlags::COLOR),
                        ),
                ]),
            );
        }
    }

    pub fn end_task(&self, device: &ash::Device) {
        self.task.end_task(device);
    }

    pub fn submit_task(
        &self,
        device: &ash::Device,
        wait_stage_mask: vk::PipelineStageFlags2,
        sig_stage_mask: vk::PipelineStageFlags2,
        wait_sem_infos: &[vk::SemaphoreSubmitInfo],
        sig_sem_infos: &[vk::SemaphoreSubmitInfo],
    ) {
        self.task.submit_task(
            device,
            wait_stage_mask,
            sig_stage_mask,
            wait_sem_infos,
            sig_sem_infos,
        );
    }

    pub fn destroy(&self, device: &ash::Device) {
        self.task.destroy(device);
    }
}

pub struct GraphicsTask {
    pub task: Task,
}

impl GraphicsTask {
    pub fn create(
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        count: usize,
        debug_utils: &DebugUtils,
        name: String,
    ) -> Self {
        Self {
            task: Task::create(device, queue, queue_family_index, count, debug_utils, name),
        }
    }

    pub fn start_task(&self, device: &ash::Device, command_buffer: vk::CommandBuffer) {
        self.task.start_task(device, command_buffer);
    }

    pub fn begin_render_pass(
        &self,
        device: &ash::Device,
        color_attachments: &[vk::RenderingAttachmentInfo],
        color_targets: &[vk::Image],
        depth_attachment: vk::RenderingAttachmentInfo,
        depth_target: vk::Image,
        viewports: &[vk::Viewport],
        scissors: &[vk::Rect2D],
    ) {
        let command_buffer = self.task.get_command_buffer();
        let rendering_info = vk::RenderingInfo::default()
            .color_attachments(color_attachments)
            .depth_attachment(&depth_attachment)
            .layer_count(1)
            .render_area(scissors[0]);

        let mut img_mem_barrs = vec![];

        for color_target in color_targets {
            img_mem_barrs.push(
                vk::ImageMemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .src_access_mask(vk::AccessFlags2::empty())
                    .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_queue_family_index(self.task.queue_family_index)
                    .dst_queue_family_index(self.task.queue_family_index)
                    .image(*color_target)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .layer_count(1)
                            .level_count(1)
                            .aspect_mask(vk::ImageAspectFlags::COLOR),
                    ),
            )
        }

        unsafe {
            device.cmd_pipeline_barrier2(
                self.task.get_command_buffer(),
                &vk::DependencyInfo::default().image_memory_barriers(&img_mem_barrs),
            );

            device.cmd_begin_rendering(command_buffer, &rendering_info);
            device.cmd_set_viewport_with_count(command_buffer, viewports);
            device.cmd_set_scissor_with_count(command_buffer, scissors);
        }
    }

    pub fn end_render_pass(&self, device: &ash::Device, color_targets: &[vk::Image]) {
        let command_buffer = self.task.get_command_buffer();

        let mut img_mem_barrs = vec![];

        for color_target in color_targets {
            img_mem_barrs.push(
                vk::ImageMemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags2::empty())
                    .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .src_queue_family_index(self.task.queue_family_index)
                    .dst_queue_family_index(self.task.queue_family_index)
                    .image(*color_target)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .layer_count(1)
                            .level_count(1)
                            .aspect_mask(vk::ImageAspectFlags::COLOR),
                    ),
            );
        }

        unsafe {
            device.cmd_end_rendering(command_buffer);
            device.cmd_pipeline_barrier2(
                self.task.get_command_buffer(),
                &vk::DependencyInfo::default().image_memory_barriers(&img_mem_barrs),
            );
        }
    }

    pub fn end_task(&self, device: &ash::Device) {
        self.task.end_task(device);
    }

    pub fn submit_task(
        &self,
        device: &ash::Device,
        wait_stage_mask: vk::PipelineStageFlags2,
        sig_stage_mask: vk::PipelineStageFlags2,
        wait_sem_infos: &[vk::SemaphoreSubmitInfo],
        sig_sem_infos: &[vk::SemaphoreSubmitInfo],
    ) {
        self.task.submit_task(
            device,
            wait_stage_mask,
            sig_stage_mask,
            wait_sem_infos,
            sig_sem_infos,
        );
    }

    pub fn present(&self, swapchain_data: &SwapchainData, image_indices: &[u32]) {
        unsafe {
            match swapchain_data.swapchain_loader.queue_present(
                self.task.queue,
                &vk::PresentInfoKHR::default()
                    .image_indices(image_indices)
                    .swapchains(&[swapchain_data.swapchain])
                    .wait_semaphores(&[swapchain_data.get_prsnt_wait_sem()]),
            ) {
                Ok(_) => {}
                Err(err) => eprintln!("{err}"),
            }
        }
    }
    pub fn destroy(&self, device: &ash::Device) {
        self.task.destroy(device);
    }
}
