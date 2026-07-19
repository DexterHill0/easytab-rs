use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::OnceLock;

use ctor::ctor;
use dtor::dtor;
#[cfg(target_vendor = "apple")]
use objc2::ffi::class_addMethod;
use objc2::ffi::{
    OBJC_ASSOCIATION_ASSIGN, class_getInstanceMethod, class_replaceMethod, method_getTypeEncoding,
    objc_getAssociatedObject, objc_removeAssociatedObjects, objc_setAssociatedObject,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{ClassType, msg_send, sel};
use objc2_app_kit::{NSEvent, NSEventType, NSView, NSWindow};
use objc2_foundation::NSPoint;
use thiserror::Error;

use crate::{EasyTabOptions, EasyTabResult, EasyTablet, TabletEvent, TabletInner};

#[derive(Debug, Error, Clone)]
pub enum MacosError {
    #[error("no window found for given NSView")]
    NoWindowFound,
}

#[derive(Debug)]
struct SwizzleData {
    original_imp: unsafe extern "C-unwind" fn(),
    encoding: *const i8,
}

// TODO: safety comment
unsafe impl Send for SwizzleData {}
unsafe impl Sync for SwizzleData {}

static SWIZZLE_DATA: OnceLock<SwizzleData> = OnceLock::new();

fn get_assoc_object_selector() -> *const c_void {
    const {
        assert!(mem::size_of::<Sel>() == mem::size_of::<NonNull<c_void>>());
    };

    let sel = sel!(windowAssociatedObject);
    // SAFETY: we have checked the size of `Sel` equals `NonNull<c_void>` above.
    // `NonNull<T>` is guaranteed to have the same size as `*mut T`.
    unsafe { mem::transmute(sel) }
}

#[allow(non_snake_case)]
unsafe extern "C-unwind" fn sendEvent_swizzle(this: &NSWindow, _cmd: Sel, event: &NSEvent) {
    let sel = get_assoc_object_selector();

    let this_any: &AnyObject = this.as_ref();

    let obj = unsafe { objc_getAssociatedObject(this_any as *const _, sel) };

    if let Some(assoc) = NonNull::new(obj.cast_mut()) {
        let raw: *const TabletInner = unsafe { mem::transmute(assoc) };
        let tablet: &TabletInner = unsafe { &*raw };

        let raw_pressure = event.pressure();
        let pressure = raw_pressure / tablet.options.pressure_normalization;

        let NSPoint { x, y } = event.locationInWindow();

        match event.r#type() {
            NSEventType::MouseMoved => {
                tablet
                    .events
                    .borrow_mut()
                    .push_back(TabletEvent::StylusMove {
                        x: x as i32,
                        y: y as i32,
                        pressure,
                        raw_pressure,
                    })
            },
            NSEventType::LeftMouseDown
            | NSEventType::RightMouseDown
            | NSEventType::OtherMouseDown => {
                tablet
                    .events
                    .borrow_mut()
                    .push_back(TabletEvent::StylusDown {
                        x: x as i32,
                        y: y as i32,
                        pressure,
                        raw_pressure,
                    });
            },
            NSEventType::LeftMouseDragged
            | NSEventType::RightMouseDragged
            | NSEventType::OtherMouseDragged => {
                tablet
                    .events
                    .borrow_mut()
                    .push_back(TabletEvent::StylusMove {
                        x: x as i32,
                        y: y as i32,
                        pressure,
                        raw_pressure,
                    });
            },
            NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
                tablet.events.borrow_mut().push_back(TabletEvent::StylusUp {
                    x: x as i32,
                    y: y as i32,
                });
            },
            NSEventType::MouseEntered => {
                tablet
                    .events
                    .borrow_mut()
                    .push_back(TabletEvent::StylusEnter);
            },
            NSEventType::MouseExited => {
                tablet
                    .events
                    .borrow_mut()
                    .push_back(TabletEvent::StylusLeave);
            },
            _ => {},
        }
    }

    unsafe { msg_send!(this, sendEvent_original: event) }
}

#[ctor(unsafe)]
fn apply_swizzles() {
    let ns_window_class: *const AnyClass = NSWindow::class();

    let original_send_event = sel!(sendEvent:);
    let original_method = unsafe { class_getInstanceMethod(ns_window_class, original_send_event) };
    let encoding = unsafe { method_getTypeEncoding(original_method) };

    let original_imp = unsafe {
        class_replaceMethod(
            ns_window_class.cast_mut(),
            original_send_event,
            mem::transmute::<unsafe extern "C-unwind" fn(_, _, _), Imp>(sendEvent_swizzle as _),
            encoding,
        )
        .expect("ctor class_replaceMethod swizzle failed")
    };

    unsafe {
        class_addMethod(
            ns_window_class.cast_mut(),
            sel!(sendEvent_original:),
            original_imp,
            encoding,
        )
    };

    SWIZZLE_DATA
        .set(SwizzleData {
            original_imp,
            encoding,
        })
        .expect("swizzle data already initialized");
}

#[dtor(unsafe)]
fn cleanup() {
    let ns_window_class: *const AnyClass = NSWindow::class();

    let original_send_event = sel!(sendEvent:);

    let SwizzleData {
        original_imp,
        encoding,
    } = SWIZZLE_DATA.get().expect("swizzle data not set in dtor");

    unsafe {
        class_replaceMethod(
            ns_window_class.cast_mut(),
            original_send_event,
            *original_imp,
            *encoding,
        )
        .expect("dtor class_replaceMethod failed");
    }
}

pub struct TabletData {
    window: Retained<NSWindow>,
}

impl EasyTablet {
    pub fn init_appkit(handle: NonNull<NSView>, options: EasyTabOptions) -> EasyTabResult<Self> {
        let window = unsafe { &*handle.as_ptr() }
            .window()
            .ok_or(MacosError::NoWindowFound)?;

        let inner = Rc::new(TabletInner {
            enabled: Cell::new(false),
            events: RefCell::new(VecDeque::new()),
            options,
        });

        let assoc_sel = get_assoc_object_selector();
        let assoc_data = Rc::into_raw(Rc::clone(&inner));

        unsafe {
            objc_setAssociatedObject(
                Retained::as_ptr(&window).cast_mut().cast(),
                assoc_sel,
                assoc_data.cast_mut().cast(),
                OBJC_ASSOCIATION_ASSIGN,
            );
        }

        Ok(Self {
            inner,
            data: TabletData { window },
        })
    }

    #[cfg(feature = "raw-window-handle")]
    pub(crate) fn init_appkit_rwh(
        handle: NonNull<c_void>,
        options: EasyTabOptions,
    ) -> EasyTabResult<Self> {
        // SAFETY: raw_window_handle states the handle is:
        // > A pointer to an NSView object.
        let handle: NonNull<NSView> = unsafe { std::mem::transmute(handle) };

        Self::init_appkit(handle, options)
    }

    /// Drains all pending tablet events. Call this once per frame.
    pub fn poll_events(&self) -> impl Iterator<Item = TabletEvent> {
        std::mem::take(&mut *self.inner.events.borrow_mut()).into_iter()
    }
}

impl Drop for EasyTablet {
    fn drop(&mut self) {
        let sel = get_assoc_object_selector();

        let this_any: *mut AnyObject = Retained::as_ptr(&self.data.window).cast_mut().cast();

        let obj = unsafe { objc_getAssociatedObject(this_any as *const _, sel) };

        let Some(assoc) = NonNull::new(obj.cast_mut()) else {
            return;
        };

        unsafe { objc_removeAssociatedObjects(this_any as *mut _) };

        let raw: *const TabletInner = unsafe { mem::transmute(assoc) };
        // SAFETY: we created the pointer with `Rc::into_raw`
        let _ = unsafe { Rc::from_raw(raw) };
    }
}
