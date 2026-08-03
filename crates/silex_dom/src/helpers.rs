use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::Window;

use silex_core::SilexError;

use crate::view::{HostResourceHandle, ViewOwnerToken};

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
pub fn window_event_listener_untyped_detached(
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
pub fn window_event_listener_detached<E, F>(event: E, mut cb: F) -> WindowListenerHandle
where
    E: crate::event::EventDescriptor + 'static,
    F: FnMut(E::EventType) + 'static,
{
    window_event_listener_untyped_detached(&event.name(), move |e| {
        cb(e.unchecked_into());
    })
}

pub fn window_event_listener_untyped_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    event_name: &str,
    mut cb: impl FnMut(web_sys::Event) + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let destination = owner.host_callback(move |payload| {
        cb(payload.unchecked_into::<web_sys::Event>());
    });
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let _ = destination.dispatch(event.into());
    }) as Box<dyn FnMut(web_sys::Event)>);
    let js_fn = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let window = window();
    window.add_event_listener_with_callback(event_name, &js_fn)?;

    let closure = Rc::new(RefCell::new(Some(closure)));
    let closure_for_cleanup = closure.clone();
    let event_name = event_name.to_string();
    Ok(owner.host_resource(move || {
        let _ = window.remove_event_listener_with_callback(&event_name, &js_fn);
        let _ = closure_for_cleanup.borrow_mut().take();
    }))
}

pub fn window_event_listener_owned<'scope, E, F>(
    owner: &ViewOwnerToken<'scope>,
    event: E,
    mut cb: F,
) -> Result<HostResourceHandle<'scope>, JsValue>
where
    E: crate::event::EventDescriptor + 'static,
    F: FnMut(E::EventType) + 'scope,
{
    window_event_listener_untyped_owned(owner, &event.name(), move |event| {
        cb(event.unchecked_into());
    })
}

/// Compatibility wrapper for the detached/manual listener API.
pub fn window_event_listener_untyped(
    event_name: &str,
    cb: impl FnMut(web_sys::Event) + 'static,
) -> WindowListenerHandle {
    window_event_listener_untyped_detached(event_name, cb)
}

/// Compatibility wrapper for the detached/manual listener API.
pub fn window_event_listener<E, F>(event: E, cb: F) -> WindowListenerHandle
where
    E: crate::event::EventDescriptor + 'static,
    F: FnMut(E::EventType) + 'static,
{
    window_event_listener_detached(event, cb)
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

fn duration_millis(duration: Duration) -> i32 {
    duration.as_millis().try_into().unwrap_or(i32::MAX)
}

fn owned_once_callback<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() + 'scope,
) -> JsValue {
    let mut cb = Some(cb);
    let destination = owner.host_callback(move |_| {
        if let Some(cb) = cb.take() {
            cb();
        }
    });
    Closure::once_into_js(move || {
        let _ = destination.dispatch(JsValue::UNDEFINED);
    })
}

pub fn request_animation_frame_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(Some(owned_once_callback(owner, cb))));
    let callback_ref = callback.borrow();
    let frame = window().request_animation_frame(
        callback_ref
            .as_ref()
            .expect("owned animation callback is present")
            .as_ref()
            .unchecked_ref(),
    );
    drop(callback_ref);
    let frame = frame?;
    let callback_for_cleanup = callback.clone();
    Ok(owner.host_resource(move || {
        let _ = window().cancel_animation_frame(frame);
        let _ = callback_for_cleanup.borrow_mut().take();
    }))
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

pub fn request_idle_callback_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(Some(owned_once_callback(owner, cb))));
    let callback_ref = callback.borrow();
    let idle = window().request_idle_callback(
        callback_ref
            .as_ref()
            .expect("owned idle callback is present")
            .as_ref()
            .unchecked_ref(),
    );
    drop(callback_ref);
    let idle = idle?;
    let callback_for_cleanup = callback.clone();
    Ok(owner.host_resource(move || {
        window().cancel_idle_callback(idle);
        let _ = callback_for_cleanup.borrow_mut().take();
    }))
}

pub fn queue_microtask(task: impl FnOnce() + 'static) {
    let task = Closure::once_into_js(task);
    window().queue_microtask(&task.unchecked_into());
}

pub fn queue_microtask_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    task: impl FnOnce() + 'scope,
) -> HostResourceHandle<'scope> {
    if !owner.is_active() {
        return HostResourceHandle::inactive();
    }
    let callback = Rc::new(RefCell::new(Some(owned_once_callback(owner, task))));
    let callback_for_cleanup = callback.clone();
    let task = callback
        .borrow()
        .as_ref()
        .expect("owned microtask callback is present")
        .clone();
    window().queue_microtask(task.unchecked_ref());
    // Microtasks cannot be physically removed. The destination gate still
    // prevents user code after owner disposal.
    owner.host_resource(move || {
        let _ = callback_for_cleanup.borrow_mut().take();
    })
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

pub fn set_timeout_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() + 'scope,
    duration: Duration,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(Some(owned_once_callback(owner, cb))));
    let callback_ref = callback.borrow();
    let timeout = window().set_timeout_with_callback_and_timeout_and_arguments_0(
        callback_ref
            .as_ref()
            .expect("owned timeout callback is present")
            .as_ref()
            .unchecked_ref(),
        duration_millis(duration),
    );
    drop(callback_ref);
    let timeout = timeout?;
    let callback_for_cleanup = callback.clone();
    Ok(owner.host_resource(move || {
        window().clear_timeout_with_handle(timeout);
        let _ = callback_for_cleanup.borrow_mut().take();
    }))
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

pub fn set_interval_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    mut cb: impl FnMut() + 'scope,
    duration: Duration,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let destination = owner.host_callback(move |_| cb());
    let closure = Closure::wrap(Box::new(move || {
        let _ = destination.dispatch(JsValue::UNDEFINED);
    }) as Box<dyn FnMut()>);
    let js_fn = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let window = window();
    let interval = window.set_interval_with_callback_and_timeout_and_arguments_0(
        &js_fn,
        duration_millis(duration),
    )?;
    let closure = Rc::new(RefCell::new(Some(closure)));
    let closure_for_cleanup = closure.clone();
    Ok(owner.host_resource(move || {
        window.clear_interval_with_handle(interval);
        let _ = closure_for_cleanup.borrow_mut().take();
    }))
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

struct DebounceTimer {
    id: i32,
    callback: JsValue,
}

struct DebounceState<T> {
    pending: Option<T>,
    timer: Option<DebounceTimer>,
    generation: u64,
}

impl<T> DebounceState<T> {
    fn clear_timer(&mut self) {
        if let Some(timer) = self.timer.take() {
            window().clear_timeout_with_handle(timer.id);
            drop(timer.callback);
        }
        self.generation = self.generation.wrapping_add(1);
    }
}

pub fn debounce_owned<'scope, T, F>(
    owner: &ViewOwnerToken<'scope>,
    delay: Duration,
    mut cb: F,
) -> impl FnMut(T) + 'scope
where
    T: 'scope,
    F: FnMut(T) + 'scope,
{
    let state = Rc::new(RefCell::new(DebounceState {
        pending: None,
        timer: None,
        generation: 0,
    }));
    let state_for_callback = state.clone();
    let destination = owner.host_callback(move |payload| {
        let Some(generation) = payload.as_f64() else {
            return;
        };
        let value = {
            let mut state = state_for_callback.borrow_mut();
            if state.generation as f64 != generation {
                return;
            }
            if let Some(timer) = state.timer.take() {
                drop(timer.callback);
            }
            state.pending.take()
        };
        if let Some(value) = value {
            cb(value);
        }
    });

    let state_for_cleanup = state.clone();
    let resource = owner.host_resource(move || {
        let mut state = state_for_cleanup.borrow_mut();
        state.clear_timer();
        let _ = state.pending.take();
    });

    move |arg| {
        if !resource.is_active() {
            return;
        }
        let generation = {
            let mut state = state.borrow_mut();
            state.clear_timer();
            state.pending = Some(arg);
            state.generation
        };
        let destination = destination.clone();
        let callback = Closure::once_into_js(move || {
            let _ = destination.dispatch(JsValue::from_f64(generation as f64));
        });
        let timeout = window().set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            duration_millis(delay),
        );
        match timeout {
            Ok(id) => {
                state.borrow_mut().timer = Some(DebounceTimer { id, callback });
            }
            Err(_) => {
                let _ = state.borrow_mut().pending.take();
            }
        }
    }
}

// --- Explicit owner and detached helpers ---

pub fn use_interval_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    duration: Duration,
    cb: impl FnMut() + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    set_interval_owned(owner, cb, duration)
}

pub fn use_timeout_owned<'scope>(
    owner: &ViewOwnerToken<'scope>,
    duration: Duration,
    cb: impl FnOnce() + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    set_timeout_owned(owner, cb, duration)
}

/// Detached/manual interval helper. It is not associated with a view owner.
pub fn use_interval(
    duration: Duration,
    cb: impl Fn() + 'static,
) -> Result<IntervalHandle, JsValue> {
    let handle = set_interval_with_handle(cb, duration)?;
    Ok(handle)
}

/// Detached/manual timeout helper. It is not associated with a view owner.
pub fn use_timeout(
    duration: Duration,
    cb: impl FnOnce() + 'static,
) -> Result<TimeoutHandle, JsValue> {
    let handle = set_timeout_with_handle(cb, duration)?;
    Ok(handle)
}
