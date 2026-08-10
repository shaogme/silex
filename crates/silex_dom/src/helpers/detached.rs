//! Browser callbacks that intentionally outlive a scoped view owner.
//!
//! Component code should use the owner-bound helpers from `crate::helpers`.

use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

/// A RAII handle for a window event listener.
pub struct WindowListenerHandle {
    cleanup: Option<Box<dyn FnOnce()>>,
}

impl WindowListenerHandle {
    pub fn new(cleanup: impl FnOnce() + 'static) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
        }
    }

    pub fn remove(mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }

    pub fn forget(mut self) {
        self.cleanup = None;
    }
}

impl Drop for WindowListenerHandle {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

pub fn window_event_listener_untyped(
    event_name: &str,
    cb: impl FnMut(web_sys::Event) + 'static,
) -> WindowListenerHandle {
    let mut cb = AssertUnwindSafe(cb);
    let cb: Closure<dyn FnMut(web_sys::Event)> = Closure::wrap(Box::new(move |event| (*cb)(event)));
    let cb = cb.into_js_value();

    let _ =
        super::window().add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());

    let event_name = event_name.to_string();
    let cb_clone = cb.clone();
    WindowListenerHandle::new(move || {
        let _ = super::window()
            .remove_event_listener_with_callback(&event_name, cb_clone.as_ref().unchecked_ref());
    })
}

pub fn try_window_event_listener_untyped(
    event_name: &str,
    cb: impl FnMut(web_sys::Event) + 'static,
) -> Result<WindowListenerHandle, JsValue> {
    let mut cb = AssertUnwindSafe(cb);
    let cb: Closure<dyn FnMut(web_sys::Event)> = Closure::wrap(Box::new(move |event| (*cb)(event)));
    let cb = cb.into_js_value();

    let window = super::try_window().ok_or_else(|| JsValue::from_str("Window not found"))?;
    window.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref())?;

    let event_name = event_name.to_string();
    let cb_clone = cb.clone();
    Ok(WindowListenerHandle::new(move || {
        if let Some(window) = super::try_window() {
            let _ = window.remove_event_listener_with_callback(
                &event_name,
                cb_clone.as_ref().unchecked_ref(),
            );
        }
    }))
}

pub fn window_event_listener<E, F>(event: E, mut cb: F) -> WindowListenerHandle
where
    E: crate::event::EventDescriptor + 'static,
    F: FnMut(E::EventType) + 'static,
{
    window_event_listener_untyped(&event.name(), move |event| {
        cb(event.unchecked_into());
    })
}

fn closure_once(cb: impl FnOnce() + 'static) -> JsValue {
    Closure::once_into_js(AssertUnwindSafe(cb))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimationFrameRequestHandle(i32);

impl AnimationFrameRequestHandle {
    pub fn cancel(&self) {
        let _ = super::window().cancel_animation_frame(self.0);
    }
}

pub fn request_animation_frame(cb: impl FnOnce() + 'static) {
    let _ = request_animation_frame_with_handle(cb);
}

pub fn request_animation_frame_with_handle(
    cb: impl FnOnce() + 'static,
) -> Result<AnimationFrameRequestHandle, JsValue> {
    super::window()
        .request_animation_frame(closure_once(cb).as_ref().unchecked_ref())
        .map(AnimationFrameRequestHandle)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdleCallbackHandle(u32);

impl IdleCallbackHandle {
    pub fn cancel(&self) {
        super::window().cancel_idle_callback(self.0);
    }
}

pub fn request_idle_callback(cb: impl FnOnce() + 'static) {
    let _ = request_idle_callback_with_handle(cb);
}

pub fn request_idle_callback_with_handle(
    cb: impl FnOnce() + 'static,
) -> Result<IdleCallbackHandle, JsValue> {
    super::window()
        .request_idle_callback(closure_once(cb).as_ref().unchecked_ref())
        .map(IdleCallbackHandle)
}

pub fn queue_microtask(task: impl FnOnce() + 'static) {
    let task = Closure::once_into_js(AssertUnwindSafe(task));
    super::window().queue_microtask(&task.unchecked_into());
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimeoutHandle(i32);

impl TimeoutHandle {
    pub fn clear(&self) {
        super::window().clear_timeout_with_handle(self.0);
    }
}

pub fn set_timeout(cb: impl FnOnce() + 'static, duration: Duration) {
    let _ = set_timeout_with_handle(cb, duration);
}

pub fn set_timeout_with_handle(
    cb: impl FnOnce() + 'static,
    duration: Duration,
) -> Result<TimeoutHandle, JsValue> {
    super::window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            closure_once(cb).as_ref().unchecked_ref(),
            super::duration_millis(duration),
        )
        .map(TimeoutHandle)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalHandle(i32);

impl IntervalHandle {
    pub fn clear(&self) {
        super::window().clear_interval_with_handle(self.0);
    }
}

pub fn set_interval(cb: impl Fn() + 'static, duration: Duration) {
    let _ = set_interval_with_handle(cb, duration);
}

pub fn set_interval_with_handle(
    cb: impl Fn() + 'static,
    duration: Duration,
) -> Result<IntervalHandle, JsValue> {
    let cb = AssertUnwindSafe(cb);
    let cb: Closure<dyn FnMut()> = Closure::wrap(Box::new(move || (*cb)()));
    let cb = cb.into_js_value();
    super::window()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            super::duration_millis(duration),
        )
        .map(IntervalHandle)
}

pub fn debounce<T: 'static>(delay: Duration, cb: impl FnMut(T) + 'static) -> impl FnMut(T) {
    let cb = Rc::new(RefCell::new(cb));
    let timer = Rc::new(RefCell::new(None::<TimeoutHandle>));

    move |arg| {
        if let Some(timer) = timer.borrow_mut().take() {
            timer.clear();
        }
        let handle = set_timeout_with_handle(
            {
                let cb = Rc::clone(&cb);
                let timer = Rc::clone(&timer);
                move || {
                    let _ = timer.borrow_mut().take();
                    cb.borrow_mut()(arg);
                }
            },
            delay,
        );
        if let Ok(handle) = handle {
            *timer.borrow_mut() = Some(handle);
        }
    }
}

pub fn use_interval(
    duration: Duration,
    cb: impl Fn() + 'static,
) -> Result<IntervalHandle, JsValue> {
    set_interval_with_handle(cb, duration)
}

pub fn use_timeout(
    duration: Duration,
    cb: impl FnOnce() + 'static,
) -> Result<TimeoutHandle, JsValue> {
    set_timeout_with_handle(cb, duration)
}
