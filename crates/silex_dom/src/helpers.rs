use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::Window;

use silex_core::SilexError;

// --- Window & Document Access ---

// --- Window & Document Access ---

/// Try returning the current [`Window`](web_sys::Window) if available.
pub fn try_window() -> Option<Window> {
    web_sys::window()
}

/// Returns the current [`Window`](web_sys::Window).
pub fn window() -> Window {
    web_sys::window().expect("Window not found")
}

/// Try returning the current [`Document`](web_sys::Document) if available.
pub fn try_document() -> Option<Document> {
    web_sys::window().and_then(|w| w.document())
}

/// Returns the current [`Document`](web_sys::Document).
pub fn document() -> Document {
    try_document().expect("Document not found")
}

// --- Location Helpers ---

/// Returns the current [`window.location`](web_sys::Location).
pub fn location() -> web_sys::Location {
    window().location()
}

/// Current [`window.location.hash`](web_sys::Location::hash) without the beginning #.
pub fn location_hash() -> Option<String> {
    let hash = location().hash().ok()?;
    Some(hash.strip_prefix('#').unwrap_or(&hash).to_string())
}

/// Current [`window.location.pathname`](web_sys::Location::pathname).
pub fn location_pathname() -> Option<String> {
    location().pathname().ok()
}

// --- Event Helpers ---

/// Helper function to extract [`Event.target`](web_sys::Event::target) from any event.
pub fn event_target<T>(event: &web_sys::Event) -> T
where
    T: JsCast,
{
    event
        .target()
        .expect("Event target not found")
        .unchecked_into::<T>()
}

/// Helper function to extract `event.target.value` from an event.
/// Supports Input, TextArea, and Select elements.
pub fn event_target_value_result<E>(event: &E) -> Result<String, SilexError>
where
    E: AsRef<web_sys::Event>,
{
    let Some(target) = event.as_ref().target() else {
        return Err(SilexError::Dom("Event target not found".into()));
    };

    if let Some(element) = target.dyn_ref::<web_sys::HtmlInputElement>() {
        Ok(element.value())
    } else if let Some(element) = target.dyn_ref::<web_sys::HtmlTextAreaElement>() {
        Ok(element.value())
    } else if let Some(element) = target.dyn_ref::<web_sys::HtmlSelectElement>() {
        Ok(element.value())
    } else {
        Err(SilexError::Dom(
            "Event target does not expose a value".into(),
        ))
    }
}

pub fn event_target_value<E>(event: &E) -> String
where
    E: AsRef<web_sys::Event>,
{
    event_target_value_result(event).unwrap_or_default()
}

/// Helper function to extract `event.target.checked` from an event.
/// Useful for checkbox inputs.
pub fn event_target_checked<E>(event: &E) -> bool
where
    E: AsRef<web_sys::Event>,
{
    let Some(target) = event.as_ref().target() else {
        return false;
    };

    target
        .dyn_ref::<web_sys::HtmlInputElement>()
        .is_some_and(|input| input.checked())
}

/// Adds an event listener to the `Window`, returning a cancelable handle that automatically
/// unbinds the listener when dropped (RAII). Call `.forget()` to keep it alive indefinitely.
pub fn window_event_listener_untyped(
    event_name: &str,
    cb: impl FnMut(web_sys::Event) + 'static,
) -> WindowListenerHandle {
    let cb = Closure::wrap(Box::new(cb) as Box<dyn FnMut(web_sys::Event)>).into_js_value();

    let _ = window().add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());

    let event_name = event_name.to_string();
    let cb_clone = cb.clone();

    WindowListenerHandle::new(move || {
        let _ = window()
            .remove_event_listener_with_callback(&event_name, cb_clone.as_ref().unchecked_ref());
    })
}

/// Adds a typed event listener to the `Window`, returning a cancelable handle.
pub fn window_event_listener<E, F>(event: E, mut cb: F) -> WindowListenerHandle
where
    E: crate::event::EventDescriptor + 'static,
    F: FnMut(E::EventType) + 'static,
{
    window_event_listener_untyped(&event.name(), move |e| {
        cb(e.unchecked_into());
    })
}

/// A RAII handle for window event listeners. Automatically unbinds the listener on `Drop`
/// unless `.forget()` is explicitly called.
pub struct WindowListenerHandle {
    cleanup: Option<Box<dyn FnOnce()>>,
}

impl WindowListenerHandle {
    pub fn new(cleanup: impl FnOnce() + 'static) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
        }
    }

    /// Manually remove the event listener immediately.
    pub fn remove(mut self) {
        if let Some(f) = self.cleanup.take() {
            f();
        }
    }

    /// Disables automatic removal on Drop, keeping the listener active indefinitely.
    pub fn forget(mut self) {
        self.cleanup = None;
    }
}

impl Drop for WindowListenerHandle {
    fn drop(&mut self) {
        if let Some(f) = self.cleanup.take() {
            f();
        }
    }
}

// --- Timer & Animation Frame Helpers ---

fn closure_once(cb: impl FnOnce() + 'static) -> JsValue {
    Closure::once_into_js(cb)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimationFrameRequestHandle(i32);

impl AnimationFrameRequestHandle {
    pub fn cancel(&self) {
        let _ = window().cancel_animation_frame(self.0);
    }
}

pub fn request_animation_frame(cb: impl FnOnce() + 'static) {
    let _ = request_animation_frame_with_handle(cb);
}

pub fn request_animation_frame_with_handle(
    cb: impl FnOnce() + 'static,
) -> Result<AnimationFrameRequestHandle, JsValue> {
    window()
        .request_animation_frame(closure_once(cb).as_ref().unchecked_ref())
        .map(AnimationFrameRequestHandle)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdleCallbackHandle(u32);

impl IdleCallbackHandle {
    pub fn cancel(&self) {
        window().cancel_idle_callback(self.0);
    }
}

pub fn request_idle_callback(cb: impl FnOnce() + 'static) {
    let _ = request_idle_callback_with_handle(cb);
}

pub fn request_idle_callback_with_handle(
    cb: impl FnOnce() + 'static,
) -> Result<IdleCallbackHandle, JsValue> {
    window()
        .request_idle_callback(closure_once(cb).as_ref().unchecked_ref())
        .map(IdleCallbackHandle)
}

pub fn queue_microtask(task: impl FnOnce() + 'static) {
    let task = Closure::once_into_js(task);
    window().queue_microtask(&task.unchecked_into());
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimeoutHandle(i32);

impl TimeoutHandle {
    pub fn clear(&self) {
        window().clear_timeout_with_handle(self.0);
    }
}

pub fn set_timeout(cb: impl FnOnce() + 'static, duration: Duration) {
    let _ = set_timeout_with_handle(cb, duration);
}

pub fn set_timeout_with_handle(
    cb: impl FnOnce() + 'static,
    duration: Duration,
) -> Result<TimeoutHandle, JsValue> {
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            closure_once(cb).as_ref().unchecked_ref(),
            duration.as_millis().try_into().unwrap_or(0),
        )
        .map(TimeoutHandle)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalHandle(i32);

impl IntervalHandle {
    pub fn clear(&self) {
        window().clear_interval_with_handle(self.0);
    }
}

pub fn set_interval(cb: impl Fn() + 'static, duration: Duration) {
    let _ = set_interval_with_handle(cb, duration);
}

pub fn set_interval_with_handle(
    cb: impl Fn() + 'static,
    duration: Duration,
) -> Result<IntervalHandle, JsValue> {
    let cb = Closure::wrap(Box::new(cb) as Box<dyn FnMut()>).into_js_value();
    window()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            duration.as_millis().try_into().unwrap_or(0),
        )
        .map(IntervalHandle)
}

// --- Debounce ---

/// Debounce a callback function.
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

// --- Auto-cleanup Hooks ---

/// 类似于 `set_interval`，但在当前响应式作用域被清理时自动取消定时器。
///
/// 参数顺序设计为支持尾随闭包语法：
/// ```ignore
/// use_interval(Duration::from_millis(100), || {
///     /* 每 100ms 执行一次 */
/// });
/// ```
///
/// 返回 `Result<IntervalHandle, JsValue>`，允许在必要时手动提前清除。
pub fn use_interval(
    duration: Duration,
    cb: impl Fn() + 'static,
) -> Result<IntervalHandle, JsValue> {
    let handle = set_interval_with_handle(cb, duration)?;
    Ok(handle)
}

/// 类似于 `set_timeout`，但在当前响应式作用域被清理时自动取消定时器（如果尚未执行）。
///
/// 参数顺序设计为支持尾随闭包语法：
/// ```ignore
/// use_timeout(Duration::from_secs(1), || {
///     /* 1秒后执行一次 */
/// });
/// ```
pub fn use_timeout(
    duration: Duration,
    cb: impl FnOnce() + 'static,
) -> Result<TimeoutHandle, JsValue> {
    let handle = set_timeout_with_handle(cb, duration)?;
    Ok(handle)
}
