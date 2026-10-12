use crate::events;
use crate::renderer;
use crate::scene_graph;

use winit::event;

pub struct Application<'instance_lifetime> {
    event_proxy: Option<winit::event_loop::EventLoopProxy<events::UserEvent>>,
    window: Option<winit::window::Window>,
    renderer: Option<renderer::Renderer<'instance_lifetime>>,
    scene_graph: Option<scene_graph::SceneGraph>,
}

impl<'instance_lifetime> Application<'instance_lifetime> {
    pub fn new(event_loop: &winit::event_loop::EventLoop<events::UserEvent>) -> Self {
        Self {
            event_proxy: Some(event_loop.create_proxy()),
            window: None,
            renderer: None,
            scene_graph: None,
        }
    }
}

impl<'instance_lifetime> winit::application::ApplicationHandler<events::UserEvent>
    for Application<'instance_lifetime>
{
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if !self.window.is_none() {
            return;
        }

        let window = event_loop
            .create_window(winit::window::WindowAttributes::default().with_title("Octo"))
            .unwrap();

        let mut renderer = renderer::Renderer::new(&window);
        renderer.start_jobs();

        self.renderer = Some(renderer);
        self.window = Some(window);
        self.scene_graph = Some(scene_graph::SceneGraph::create());
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            event::WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => match event.physical_key {
                winit::keyboard::PhysicalKey::Code(key_code) => match key_code {
                    winit::keyboard::KeyCode::Escape => {
                        event_loop.exit();
                    }
                    _ => {}
                },
                winit::keyboard::PhysicalKey::Unidentified(key_code) => {}
            },
            event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            event::WindowEvent::Destroyed => {}
            event::WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        let _ = (event_loop, cause);
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: events::UserEvent,
    ) {
        let _ = (event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let _ = (event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;

        let renderer = self.renderer.take().unwrap();

        renderer.destroy();
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let _ = event_loop;
    }
}
