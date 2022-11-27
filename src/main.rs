use std::{cell::Cell, pin::Pin, rc::Rc};

use thiserror::Error;

#[cfg(target_os = "windows")]
pub mod win32;
use win32::WinTabEvent;
#[cfg(target_os = "windows")]
pub use win32::WinTabletIndex;
use windows::Win32::Foundation::HANDLE_PTR;
#[cfg(target_os = "windows")]
use windows::Win32::UI::TabletPC::IRealTimeStylus;

#[cfg(target_os = "windows")]
type Message = windows::core::HSTRING;

///
#[derive(Error, Clone, Debug)]
pub enum EasyTabError {
    #[cfg(target_os = "windows")]
    #[error("win error: {0}")]
    WinError(Message),
}

pub type EasyTabResult<T> = std::result::Result<T, EasyTabError>;

/// The initialisation options for the tablet.
#[derive(Default)]
pub struct EasyTabOptions {
    /// When a tablet is disconnected and either reconnected, or a new tablet is connected, it will try to re-initialise the new tablet.
    pub retry_on_change: bool,
    #[cfg(target_os = "windows")]
    pub index: WinTabletIndex,
}

// transparent, private wrapper struct since `EasyTablet` needs to wrapped in an `Rc`, but I don't want to expose the `Rc` to the user.
// especially since it would require them to write `Rc<EasyTablet>` everywhere, rather than `EasyTablet`.
/// Private inner struct, do not use. (Use [`EasyTablet`] instead)
#[doc(hidden)]
pub struct __InnerTablet {
    active: Cell<bool>,
    x: Cell<i32>,
    y: Cell<i32>,
    pressure: Cell<f32>,

    opts: EasyTabOptions,

    #[cfg(target_os = "windows")]
    on: Cell<Option<Box<dyn Fn(WinTabEvent)>>>,

    #[cfg(target_os = "windows")]
    stylus: IRealTimeStylus,
}

/// TODO
pub struct EasyTablet(Rc<__InnerTablet>);

impl std::ops::Deref for EasyTablet {
    type Target = __InnerTablet;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

fn main() {
    // unsafe {
    //     CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED).expect("failed to initalise COM")
    // };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let handle = window.raw_window_handle();

    let hwnd = match handle {
        RawWindowHandle::Win32(r) => r,
        _ => panic!(""),
    };

    let tablet = EasyTablet::init(hwnd.hwnd as usize).expect("tablet failed to initialize");

    tablet.enable().expect("enable");

    tablet.on(Box::new(|event| println!("new event {:#?}", event)));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
