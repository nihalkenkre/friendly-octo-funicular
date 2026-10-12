use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread::{self, JoinHandle},
    time::Duration,
};

use ash::vk;
use winit::raw_window_handle::HasDisplayHandle;

use crate::scene_graph::{self, SceneGraph};

mod objects;

pub struct Renderer<'instance_lifetime> {
    graphics_tasks: Arc<Mutex<objects::GraphicsTask>>,
    graphics_thread: Option<JoinHandle<()>>,
    is_shutting_down: Arc<AtomicBool>,

    scene_graph: scene_graph::SceneGraph,
    allocator: vk_mem::Allocator,
    swapchain_data: Arc<Mutex<objects::SwapchainData>>,
    device_data: Arc<Mutex<objects::DeviceData>>,
    physical_device_data: objects::PhysicalDeviceData<'instance_lifetime>,
    surface_data: objects::SurfaceData<'instance_lifetime>,
    instance: objects::Instance,
}

impl<'instance_lifetime> Renderer<'instance_lifetime> {
    pub fn new(window: &winit::window::Window) -> Self {
        let extensions =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
                .unwrap();

        let instance = objects::Instance::new(extensions);
        let mut surface_data = objects::SurfaceData::new(&instance.instance, &window);

        let physical_device_data = objects::PhysicalDeviceData::new(
            &instance.instance,
            &surface_data.surface_loader,
            &surface_data.surface,
        );
        surface_data.populate_data(&physical_device_data.physical_device);

        let dd = objects::DeviceData::new(&instance.instance, &physical_device_data);
        let debug_utils = &objects::DebugUtils::new(&instance.instance, &dd.device);

        if cfg!(debug_assertions) {
            debug_utils.set_object_name(instance.instance.handle(), "instance".to_string());
            debug_utils.set_object_name(surface_data.surface, "surface".to_string());
            debug_utils.set_object_name(
                physical_device_data.physical_device,
                "physical Device".to_string(),
            );
            debug_utils.set_object_name(dd.device.handle(), "device".to_string());
            debug_utils.set_object_name(dd.graphics_queue, "graphics queue".to_string());
            debug_utils.set_object_name(dd.compute_queue, "compute queue".to_string());
            debug_utils.set_object_name(dd.transfer_queue, "transfer queue".to_string());
        }

        let sd = objects::SwapchainData::new(
            &instance.instance,
            &dd.device,
            &surface_data,
            &physical_device_data.queue_family_indices,
            debug_utils,
        );

        let graphics_tasks = Arc::new(Mutex::new(objects::GraphicsTask::create(
            &dd.device,
            dd.graphics_queue,
            physical_device_data.graphics_queue_family_index,
            sd.image_count as usize,
            &debug_utils,
            "graphics tasks".to_string(),
        )));

        let swapchain_data = Arc::new(Mutex::new(sd));

        let allocator;
        unsafe {
            allocator = vk_mem::Allocator::new(vk_mem::AllocatorCreateInfo::new(
                &instance.instance,
                &dd.device,
                physical_device_data.physical_device,
            ))
            .unwrap();
        }

        let device_data = Arc::new(Mutex::new(dd));
        let scene_graph = SceneGraph::create();
        let is_shutting_down = Arc::new(AtomicBool::new(false));
        let graphics_thread = None;

        Self {
            instance,
            surface_data,
            physical_device_data,
            device_data,
            swapchain_data,
            graphics_tasks,
            allocator,
            scene_graph,
            is_shutting_down,
            graphics_thread,
        }
    }

    pub fn destroy(self) {
        self.is_shutting_down
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let device_data = self.device_data.lock().unwrap();
        unsafe {
            device_data.device.device_wait_idle().unwrap();
        }

        self.graphics_thread.unwrap().join().unwrap();
        self.graphics_tasks
            .lock()
            .unwrap()
            .destroy(&device_data.device);

        self.swapchain_data
            .lock()
            .unwrap()
            .destroy(&device_data.device);

        device_data.destroy();
        self.surface_data.destroy();
        self.instance.destroy();
    }

    pub fn start_jobs(&mut self) {
        {}
        {}
        {
            let gt = Arc::clone(&self.graphics_tasks);
            let is_shutting_down = Arc::clone(&self.is_shutting_down);
            let dd = Arc::clone(&self.device_data);
            let sd = Arc::clone(&self.swapchain_data);
            let extent = self
                .surface_data
                .capabilities
                .surface_capabilities
                .current_extent;

            self.graphics_thread = Some(thread::spawn(move || {
                while !is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                    let mut swapchain_data = sd.lock().unwrap();
                    let mut graphics_tasks = gt.lock().unwrap();
                    let device_data = dd.lock().unwrap();

                    let clear_color = vk::ClearColorValue {
                        float32: [
                            rand::random::<f32>(),
                            rand::random::<f32>(),
                            rand::random::<f32>(),
                            1f32,
                        ],
                    };

                    let clear_value = vk::ClearValue { color: clear_color };

                    let (semaphore, value) = graphics_tasks.task.get_semaphore_and_value();

                    unsafe {
                        device_data
                            .device
                            .wait_semaphores(
                                &vk::SemaphoreWaitInfo::default()
                                    .semaphores(&[semaphore])
                                    .values(&[value]),
                                u64::MAX,
                            )
                            .unwrap();
                    }

                    let acq_info = vk::AcquireNextImageInfoKHR::default()
                        .swapchain(swapchain_data.swapchain)
                        .semaphore(swapchain_data.get_acq_sig_sem())
                        .timeout(u64::MAX)
                        .device_mask(1);

                    let image_index: u32;

                    unsafe {
                        match swapchain_data
                            .swapchain_loader
                            .acquire_next_image2(&acq_info)
                        {
                            Ok(id) => {
                                if !id.1 {
                                    image_index = id.0;
                                } else {
                                    eprintln!("Swapchain is suboptimal");
                                    return;
                                }
                            }

                            Err(err) => {
                                eprintln!("{err}");
                                return;
                            }
                        }
                    }

                    let color_attachments: [vk::RenderingAttachmentInfo; 1] =
                        [vk::RenderingAttachmentInfo::default()
                            .image_view(swapchain_data.get_image_view(image_index))
                            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                            .load_op(vk::AttachmentLoadOp::CLEAR)
                            .store_op(vk::AttachmentStoreOp::STORE)
                            .clear_value(clear_value)];

                    let color_targets: [vk::Image; 1] = [swapchain_data.get_image(image_index)];

                    graphics_tasks.start_task(
                        &device_data.device,
                        graphics_tasks.task.get_command_buffer(),
                    );

                    graphics_tasks.begin_render_pass(
                        &device_data.device,
                        &color_attachments,
                        &color_targets,
                        vk::RenderingAttachmentInfo::default(),
                        vk::Image::null(),
                        &[vk::Viewport::default()
                            .width(1920 as f32)
                            .height(1080 as f32)
                            .min_depth(0f32)
                            .max_depth(1f32)],
                        &[vk::Rect2D::default().extent(extent)],
                    );

                    graphics_tasks.end_render_pass(&device_data.device, &color_targets);
                    graphics_tasks.end_task(&device_data.device);

                    graphics_tasks.submit_task(
                        &device_data.device,
                        vk::PipelineStageFlags2::TOP_OF_PIPE,
                        vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                        &[vk::SemaphoreSubmitInfo::default()
                            .semaphore(swapchain_data.get_acq_sig_sem())
                            .stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)],
                        &[vk::SemaphoreSubmitInfo::default()
                            .semaphore(swapchain_data.get_prsnt_wait_sem())
                            .stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)],
                    );

                    graphics_tasks.present(&swapchain_data, &[image_index]);

                    swapchain_data.next_index();
                    graphics_tasks.task.next_frame();

                    // thread::sleep(Duration::from_millis(100));
                }
            }));
        }
    }

    pub fn render_scene_graph(&self, scene_graph: &scene_graph::SceneGraph) {}

    pub fn render_node_editor(&self, scene_graph: &scene_graph::SceneGraph) {}

    // pub fn render_blank(&mut self) {
    //     let (sem, sem_val) = self.graphics_tasks.get_semaphore_and_value();

    //     unsafe {
    //         self.device_data
    //             .device
    //             .wait_semaphores(
    //                 &vk::SemaphoreWaitInfo::default()
    //                     .semaphores(&[sem])
    //                     .values(&[sem_val]),
    //                 u64::MAX,
    //             )
    //             .unwrap();
    //     }

    //     let acq_info = vk::AcquireNextImageInfoKHR::default()
    //         .swapchain(self.swapchain_data.swapchain)
    //         .semaphore(self.swapchain_data.get_acq_sig_sem())
    //         .timeout(u64::MAX)
    //         .device_mask(1);

    //     let image_index: u32;

    //     unsafe {
    //         match self
    //             .swapchain_data
    //             .swapchain_loader
    //             .acquire_next_image2(&acq_info)
    //         {
    //             Ok(id) => {
    //                 if !id.1 {
    //                     image_index = id.0;
    //                 } else {
    //                     eprintln!("Swapchain is suboptimal");
    //                     return;
    //                 }
    //             }

    //             Err(err) => {
    //                 eprintln!("{err}");
    //                 return;
    //             }
    //         }
    //     }

    //     self.graphics_tasks.start_task(&self.device_data.device);
    //     self.graphics_tasks.change_image_layout(
    //         &self.device_data.device,
    //         *self
    //             .swapchain_data
    //             .images
    //             .get(image_index as usize)
    //             .unwrap(),
    //         vk::ImageLayout::UNDEFINED,
    //         vk::ImageLayout::PRESENT_SRC_KHR,
    //         vk::PipelineStageFlags2::TOP_OF_PIPE,
    //         vk::PipelineStageFlags2::TRANSFER,
    //         vk::AccessFlags2::empty(),
    //         vk::AccessFlags2::MEMORY_WRITE,
    //     );
    //     self.graphics_tasks.end_task(&self.device_data.device);

    //     self.graphics_tasks.submit_task(
    //         &self.device_data.device,
    //         vk::PipelineStageFlags2::TOP_OF_PIPE,
    //         vk::PipelineStageFlags2::TRANSFER,
    //         vec![
    //             vk::SemaphoreSubmitInfo::default()
    //                 .semaphore(self.swapchain_data.get_acq_sig_sem())
    //                 .stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE),
    //         ],
    //         vec![
    //             vk::SemaphoreSubmitInfo::default()
    //                 .semaphore(self.swapchain_data.get_prsnt_wait_sem())
    //                 .stage_mask(vk::PipelineStageFlags2::TRANSFER),
    //         ],
    //     );

    //     unsafe {
    //         match self.swapchain_data.swapchain_loader.queue_present(
    //             self.device_data.graphics_queue,
    //             &vk::PresentInfoKHR::default()
    //                 .image_indices(&[image_index])
    //                 .swapchains(&[self.swapchain_data.swapchain])
    //                 .wait_semaphores(&[self.swapchain_data.get_prsnt_wait_sem()]),
    //         ) {
    //             Ok(_) => {}
    //             Err(err) => eprintln!("{err}"),
    //         }
    //     }

    //     self.graphics_tasks.task.next_frame();
    //     self.swapchain_data.next_frame();
    // }
}
