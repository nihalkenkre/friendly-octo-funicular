use std::{
    sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
    thread::{self, JoinHandle},
    time::Duration,
};

use ash::vk;
use winit::raw_window_handle::HasDisplayHandle;

use crate::scene_graph::{self, SceneGraph};

mod objects;

pub struct Renderer<'instance_lifetime> {
    transfer_tasks: Arc<RwLock<objects::TransferTask>>,
    transfer_thread: Option<JoinHandle<()>>,

    graphics_tasks: Arc<RwLock<objects::GraphicsTask>>,
    graphics_thread: Option<JoinHandle<()>>,
    is_shutting_down: Arc<AtomicBool>,

    scene_graph: scene_graph::SceneGraph,
    allocator: vk_mem::Allocator,
    swapchain_data: Arc<Mutex<objects::SwapchainData>>,
    transfer_queue: Arc<Mutex<vk::Queue>>,
    compute_queue: Arc<Mutex<vk::Queue>>,
    graphics_queue: Arc<Mutex<vk::Queue>>,
    device: Arc<ash::Device>,
    physical_device_data: objects::PhysicalDeviceData<'instance_lifetime>,
    surface_data: objects::SurfaceData<'instance_lifetime>,
    instance: objects::Instance,
    ash_entry: ash::Entry,
}

impl<'instance_lifetime> Renderer<'instance_lifetime> {
    pub fn new(window: &winit::window::Window) -> Self {
        let extensions =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())
                .unwrap();

        let ash_entry = unsafe { ash::Entry::load().unwrap() };
        let instance = objects::Instance::new(&ash_entry, extensions);
        let mut surface_data = objects::SurfaceData::new(&ash_entry, &instance.instance, &window);

        let physical_device_data = objects::PhysicalDeviceData::new(
            &instance.instance,
            &surface_data.surface_loader,
            &surface_data.surface,
        );
        surface_data.populate_data(&physical_device_data.physical_device);

        let (d, gq, cq, tq) = instance.create_device_and_queues(&physical_device_data);

        let device = Arc::new(d);
        let graphics_queue = Arc::new(Mutex::new(gq));
        let compute_queue = Arc::new(Mutex::new(cq));
        let transfer_queue = Arc::new(Mutex::new(tq));

        let debug_utils = &objects::DebugUtils::new(&instance.instance, &device);

        if cfg!(debug_assertions) {
            debug_utils.set_object_name(instance.instance.handle(), "instance".to_string());
            debug_utils.set_object_name(surface_data.surface, "surface".to_string());
            debug_utils.set_object_name(
                physical_device_data.physical_device,
                "physical device".to_string(),
            );
            debug_utils.set_object_name(device.handle(), "device".to_string());
            debug_utils.set_object_name(gq, "graphics queue".to_string());
            debug_utils.set_object_name(cq, "compute queue".to_string());
            debug_utils.set_object_name(tq, "transfer queue".to_string());
        }

        let sd = objects::SwapchainData::new(
            &instance.instance,
            &device,
            &surface_data,
            &physical_device_data.queue_family_indices,
            debug_utils,
            0,
        );

        let frames_in_flight = sd.frames_in_flight;

        let graphics_tasks = Arc::new(RwLock::new(objects::GraphicsTask::create(
            &device,
            gq,
            physical_device_data.graphics_queue_family_index,
            frames_in_flight as usize,
            &debug_utils,
            "graphics tasks".to_string(),
        )));

        let transfer_tasks = Arc::new(RwLock::new(objects::TransferTask::create(
            &device,
            tq,
            physical_device_data.transfer_queue_family_index,
            frames_in_flight as usize,
            &debug_utils,
            "transfer tasks".to_string(),
        )));

        let swapchain_data = Arc::new(Mutex::new(sd));

        let allocator;
        unsafe {
            allocator = vk_mem::Allocator::new(vk_mem::AllocatorCreateInfo::new(
                &instance.instance,
                &device,
                physical_device_data.physical_device,
            ))
            .unwrap();
        }

        let scene_graph = SceneGraph::create();
        let is_shutting_down = Arc::new(AtomicBool::new(false));
        let graphics_thread = None;
        let transfer_thread = None;

        Self {
            ash_entry,
            instance,
            surface_data,
            physical_device_data,
            swapchain_data,
            allocator,
            transfer_queue,
            compute_queue,
            graphics_queue,
            device,
            scene_graph,
            is_shutting_down,
            graphics_thread,
            graphics_tasks,
            transfer_tasks,
            transfer_thread,
        }
    }

    pub fn destroy(self) {
        self.is_shutting_down
            .store(true, std::sync::atomic::Ordering::Relaxed);

        unsafe {
            self.device.device_wait_idle().unwrap();
        }

        match self.graphics_thread {
            Some(th) => th.join().unwrap(),
            None => {}
        }

        match self.transfer_thread {
            Some(th) => th.join().unwrap(),
            None => {}
        }

        self.transfer_tasks.read().unwrap().destroy(&self.device);
        self.graphics_tasks.read().unwrap().destroy(&self.device);
        self.swapchain_data.lock().unwrap().destroy(&self.device);

        unsafe {
            self.device.destroy_device(None);
        }
        self.surface_data.destroy();
        self.instance.destroy();
    }

    pub fn start_jobs(&mut self) {
        {}
        {
            let gt = Arc::clone(&self.graphics_tasks);
            let is_shutting_down = Arc::clone(&self.is_shutting_down);
            let device = Arc::clone(&self.device);
            let sd = Arc::clone(&self.swapchain_data);
            let extent = self
                .surface_data
                .capabilities
                .surface_capabilities
                .current_extent;

            self.graphics_thread = Some(thread::spawn(move || {
                while !is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                    let mut swapchain_data = sd.lock().unwrap();
                    let mut graphics_tasks;
                    {
                        graphics_tasks = gt.write().unwrap();
                    }

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
                        device
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

                    graphics_tasks.start_task(&device, graphics_tasks.task.get_command_buffer());

                    graphics_tasks.begin_render_pass(
                        &device,
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

                    graphics_tasks.end_render_pass(&device, &color_targets);
                    graphics_tasks.end_task(&device);

                    graphics_tasks.submit_task(
                        &device,
                        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                        &[vk::SemaphoreSubmitInfo::default()
                            .semaphore(swapchain_data.get_acq_sig_sem())
                            .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)],
                        &[vk::SemaphoreSubmitInfo::default()
                            .semaphore(swapchain_data.get_prsnt_wait_sem())
                            .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)],
                    );

                    graphics_tasks.present(&swapchain_data, &[image_index]);

                    swapchain_data.next_index();
                    graphics_tasks.task.next_frame();

                    println!("gfx");
                    thread::sleep(Duration::from_millis(100));
                }
            }));
        }
        {
            let is_shutting_down = Arc::clone(&self.is_shutting_down);
            let tt = Arc::clone(&self.transfer_tasks);
            let device = Arc::clone(&self.device);
            let gt = Arc::clone(&self.graphics_tasks);

            self.transfer_thread = Some(thread::spawn(move || {
                while !is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                    let mut transfer_tasks;
                    {
                        transfer_tasks = tt.write().unwrap();
                    }
                    let graphics_tasks;
                    {
                        graphics_tasks = gt.read().unwrap();
                    }

                    let (semaphore, value) = transfer_tasks.task.get_semaphore_and_value();

                    unsafe {
                        device
                            .wait_semaphores(
                                &vk::SemaphoreWaitInfo::default()
                                    .semaphores(&[semaphore])
                                    .values(&[value]),
                                u64::MAX,
                            )
                            .unwrap();
                    }

                    transfer_tasks.start_task(&device, transfer_tasks.task.get_command_buffer());

                    transfer_tasks.end_task(&device);

                    let (gt_sem, gt_val) = graphics_tasks.task.get_semaphore_and_value();

                    transfer_tasks.submit_task(
                        &device,
                        vk::PipelineStageFlags2::empty(),
                        vk::PipelineStageFlags2::TRANSFER,
                        &[vk::SemaphoreSubmitInfo::default()
                            .semaphore(gt_sem)
                            .value(gt_val)
                            .stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)],
                        &[],
                    );

                    transfer_tasks.task.next_frame();
                    println!("xfer");
                    thread::sleep(Duration::from_millis(1000));
                }
            }));
        }
    }
}
