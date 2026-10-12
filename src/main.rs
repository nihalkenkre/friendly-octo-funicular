mod application;
mod events;
mod renderer;
mod scene_graph;

fn main() {
    println!("Hello, world!");
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::<events::UserEvent>::with_user_event()
        .build()
        .unwrap();
    let mut app = application::Application::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();

    println!("Bye, world!");
}
