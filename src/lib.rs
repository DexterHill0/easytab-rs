use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use thiserror::Error;

#[cfg(target_os = "windows")]
pub mod win32;
#[cfg(target_os = "windows")]
use win32::TabletData;
#[cfg(target_os = "windows")]
pub use win32::WindowsError;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacosError;
#[cfg(target_os = "macos")]
use macos::TabletData;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!("easytab is not yet implemented for this platform");

#[derive(Error, Clone, Debug)]
pub enum EasyTabError {
    #[cfg(target_os = "windows")]
    #[error("windows error: {0}")]
    WindowsError(#[from] WindowsError),

    #[cfg(target_os = "macos")]
    #[error("macos error: {0}")]
    MacosError(#[from] MacosError),

    #[cfg(feature = "raw-window-handle")]
    #[error("{0}")]
    HandleError(#[from] raw_window_handle::HandleError),

    #[cfg(feature = "raw-window-handle")]
    #[error("unsupported platform")]
    UnsupportedPlatform,
}

pub type EasyTabResult<T> = std::result::Result<T, EasyTabError>;

/// Initialisation options for the tablet.
#[derive(Clone)]
pub struct EasyTabOptions {
    /// Divisor used to normalise raw pressure to 0.0..=1.0. Default is platform specific:
    /// - Windows: 1024.0
    /// - macOS: 1.0 (pressure is already a float between 0.0 and 1.0)
    pub pressure_normalization: f32,
}

impl Default for EasyTabOptions {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "windows")]
            pressure_normalization: 1024.0,
            #[cfg(target_os = "macos")]
            pressure_normalization: 1.0,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum TabletEvent {
    /// Pen entered hover range above the tablet.
    StylusEnter,
    /// Pen left hover range (or pointer capture was released).
    StylusLeave,
    /// Pen tip made contact with the tablet surface. `raw_pressure` is the un-normalised value.
    StylusDown {
        x: i32,
        y: i32,
        pressure: f32,
        raw_pressure: f32,
    },
    /// Pen tip lifted from the tablet surface.
    StylusUp { x: i32, y: i32 },
    /// Pen moved while hovering or in contact. `pressure` is 0.0 when hovering.
    StylusMove {
        x: i32,
        y: i32,
        pressure: f32,
        raw_pressure: f32,
    },
}

pub(crate) struct TabletInner {
    pub(crate) enabled: Cell<bool>,
    pub(crate) events: RefCell<VecDeque<TabletEvent>>,
    pub(crate) options: EasyTabOptions,
}

pub struct EasyTablet {
    pub(crate) inner: Rc<TabletInner>,
    /// Platform specific data
    pub(crate) data: TabletData,
}

impl EasyTablet {
    pub fn enable(&self) {
        self.inner.enabled.set(true);
    }

    pub fn disable(&self) {
        self.inner.enabled.set(false);
    }
}

#[cfg(feature = "raw-window-handle")]
impl EasyTablet {
    pub fn from_window<W: raw_window_handle::HasWindowHandle>(window: &W) -> EasyTabResult<Self> {
        Self::from_window_options(window, EasyTabOptions::default())
    }

    pub fn from_window_options<W: raw_window_handle::HasWindowHandle>(
        window: &W,
        options: EasyTabOptions,
    ) -> EasyTabResult<Self> {
        use raw_window_handle::RawWindowHandle;

        let raw = window
            .window_handle()
            .map_err(EasyTabError::HandleError)?
            .as_raw();

        match raw {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Win32(h) => Self::init_rwh(h.hwnd.get() as usize, options),
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(h) => Self::init_appkit_rwh(h.ns_view, options),
            #[cfg(target_os = "macos")]
            RawWindowHandle::UiKit(..) => todo!("UiKit support"),
            _ => Err(EasyTabError::UnsupportedPlatform),
        }
    }
}
