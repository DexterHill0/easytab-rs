use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use thiserror::Error;
use windows::Win32::Foundation::{HANDLE_PTR, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Input::Pointer::{GetPointerPenInfo, GetPointerType, POINTER_PEN_INFO};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    POINTER_INPUT_TYPE, PT_PEN, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERENTER,
    WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
};

use crate::{EasyTabOptions, EasyTabResult, EasyTablet, TabletEvent, TabletInner};

#[derive(Debug, Error, Clone)]
pub enum WindowsError {
    #[error("failed to call CoInitializeEx ({0})")]
    CoInitializeFailed(String),

    #[error("failed to set window subclass")]
    FailedSubclass,
}

pub struct TabletData {
    hwnd: HWND,
    subclass_data_ptr: usize,
}

struct HwndData {
    tablet: Rc<TabletInner>,
}

fn cleanup_subclass(hwnd: HWND, subclass_data_ptr: usize) {
    unsafe {
        let _ = RemoveWindowSubclass(hwnd, Some(h_wndproc), subclass_data_ptr);

        drop(Box::from_raw(subclass_data_ptr as *mut HwndData));
    }
}

impl EasyTablet {
    pub fn init<W: Into<usize>>(hwnd: W) -> EasyTabResult<Self> {
        Self::init_options(HANDLE_PTR(hwnd.into()), EasyTabOptions::default())
    }

    pub fn init_options(hwnd: HANDLE_PTR, options: EasyTabOptions) -> EasyTabResult<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if result.is_err() {
            return Err(WindowsError::CoInitializeFailed(result.message()).into());
        }

        let hwnd_win = HWND(hwnd.0 as *mut _);

        let inner = Rc::new(TabletInner {
            enabled: Cell::new(false),
            events: RefCell::new(VecDeque::new()),
            options,
        });

        let ptr = Box::into_raw(Box::new(HwndData {
            tablet: Rc::clone(&inner),
        })) as usize;

        let result = unsafe { SetWindowSubclass(hwnd_win, Some(h_wndproc), ptr, ptr) };

        if !result.as_bool() {
            unsafe { drop(Box::from_raw(ptr as *mut HwndData)) };

            return Err(WindowsError::FailedSubclass.into());
        }

        Ok(Self {
            inner,
            data: TabletData {
                hwnd: hwnd_win,
                subclass_data_ptr: ptr,
            },
        })
    }

    pub fn enable(&self) {
        self.inner.enabled.set(true);
    }

    pub fn disable(&self) {
        self.inner.enabled.set(false);
    }

    /// Drains all pending tablet events. Call this once per frame.
    pub fn poll_events(&self) -> impl Iterator<Item = TabletEvent> {
        std::mem::take(&mut *self.inner.events.borrow_mut()).into_iter()
    }
}

impl Drop for EasyTablet {
    fn drop(&mut self) {
        cleanup_subclass(self.data.hwnd, self.data.subclass_data_ptr);
    }
}

const fn loword(l: usize) -> u32 {
    (l & 0xffff) as u32
}

unsafe extern "system" fn h_wndproc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT {
    let data = unsafe { &*(dwrefdata as *const HwndData) };

    if !data.tablet.enabled.get() {
        return unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) };
    }

    'outer: {
        match umsg {
            WM_POINTERENTER
            | WM_POINTERLEAVE
            | WM_POINTERCAPTURECHANGED
            | WM_POINTERDOWN
            | WM_POINTERUP
            | WM_POINTERUPDATE => {
                let pointer_id = loword(wparam.0);

                let mut pointer_type = POINTER_INPUT_TYPE(0);

                if unsafe { GetPointerType(pointer_id, &mut pointer_type).is_err() }
                    || pointer_type != PT_PEN
                {
                    break 'outer;
                }

                let mut pen_info: POINTER_PEN_INFO = unsafe { std::mem::zeroed() };

                if unsafe { GetPointerPenInfo(pointer_id, &mut pen_info).is_err() } {
                    break 'outer;
                }

                let x = pen_info.pointerInfo.ptPixelLocation.x;
                let y = pen_info.pointerInfo.ptPixelLocation.y;

                let raw_pressure = pen_info.pressure as f32;
                let pressure = raw_pressure / data.tablet.options.pressure_normalization;

                let event = match umsg {
                    WM_POINTERENTER => TabletEvent::StylusEnter,
                    WM_POINTERLEAVE | WM_POINTERCAPTURECHANGED => TabletEvent::StylusLeave,
                    WM_POINTERDOWN => TabletEvent::StylusDown {
                        x,
                        y,
                        pressure,
                        raw_pressure,
                    },
                    WM_POINTERUP => TabletEvent::StylusUp { x, y },
                    _ => TabletEvent::StylusMove {
                        x,
                        y,
                        pressure,
                        raw_pressure,
                    },
                };

                data.tablet.events.borrow_mut().push_back(event);
            },

            _ => {},
        }
    }

    unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) }
}
