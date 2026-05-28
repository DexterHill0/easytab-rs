#![allow(clippy::single_match)]

use easytab::EasyTablet;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    window: Option<Window>,
    tablet: Option<EasyTablet>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();

        let tablet = EasyTablet::from_window(&window).unwrap();

        tablet.enable();

        self.window = Some(window);
        self.tablet = Some(tablet);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(tablet) = &self.tablet {
            for event in tablet.poll_events() {
                println!("{event:?}");
            }
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        if let Some(t) = &self.tablet {
            t.disable();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
