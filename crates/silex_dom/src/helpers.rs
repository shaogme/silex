use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::Window;

use silex_core::{SilexError, SilexResult};

use crate::view::{HostCallback, HostResourceHandle, ViewOwnerToken};

pub mod detached;

fn host_resource_error(error: SilexError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

// --- Window & Document Access ---

// --- Window & Document Access ---

/// Try returning the current [`Window`] if available.
pub fn try_window() -> Option<Window> {
    web_sys::window()
}

/// Returns the current [`Window`].
pub fn window() -> Window {
    web_sys::window().expect("Window not found")
}

/// Try returning the current [`Document`] if available.
pub fn try_document() -> Option<Document> {
    web_sys::window().and_then(|w| w.document())
}

/// Returns the current [`Document`].
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

pub fn window_event_listener_untyped<'scope>(
    owner: &ViewOwnerToken<'scope>,
    event_name: &str,
    mut cb: impl FnMut(web_sys::Event) -> SilexResult<()> + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let destination = owner.host_callback(
        move |payload| cb(payload.unchecked_into::<web_sys::Event>()),
        owner.error_handler(),
    );
    let destination_for_closure = AssertUnwindSafe(destination.clone());
    let closure: Closure<dyn FnMut(web_sys::Event)> =
        Closure::wrap(Box::new(move |event: web_sys::Event| {
            let _ = destination_for_closure.dispatch(event.into());
        }));
    let closure = Rc::new(RefCell::new(Some(closure.into_js_value())));
    let js_fn = closure
        .borrow()
        .as_ref()
        .expect("owned window listener callback is present")
        .unchecked_ref::<js_sys::Function>()
        .clone();
    let window = window();
    if let Err(error) = window.add_event_listener_with_callback(event_name, &js_fn) {
        destination.cancel();
        let _ = closure.borrow_mut().take();
        return Err(error);
    }

    let closure_for_cleanup = closure.clone();
    let event_name = event_name.to_string();
    let event_name_for_cleanup = event_name.clone();
    let js_fn_for_cleanup = js_fn.clone();
    let window_for_cleanup = window.clone();
    match owner.try_host_resource_for_callback(&destination, move || {
        let _ = window_for_cleanup
            .remove_event_listener_with_callback(&event_name_for_cleanup, &js_fn_for_cleanup);
        let _ = closure_for_cleanup.borrow_mut().take();
    }) {
        Ok(resource) => Ok(resource),
        Err(error) => Err(host_resource_error(error)),
    }
}

pub fn window_event_listener<'scope, E, F>(
    owner: &ViewOwnerToken<'scope>,
    event: E,
    mut cb: F,
) -> Result<HostResourceHandle<'scope>, JsValue>
where
    E: crate::event::EventDescriptor,
    F: FnMut(E::EventType) -> SilexResult<()> + 'scope,
{
    window_event_listener_untyped(
        owner,
        &event.name(),
        move |event| cb(event.unchecked_into()),
    )
}

// --- Timer & Animation Frame Helpers ---

pub(super) fn duration_millis(duration: Duration) -> i32 {
    duration.as_millis().try_into().unwrap_or(i32::MAX)
}

type JsClosureSlot = Rc<RefCell<Option<JsValue>>>;

struct OnceCallbackGuard {
    destination: HostCallback,
    closure: JsClosureSlot,
}

impl Drop for OnceCallbackGuard {
    fn drop(&mut self) {
        self.destination.finish();
        let _ = self.closure.borrow_mut().take();
    }
}

fn owned_once_callback<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() -> SilexResult<()> + 'scope,
    closure: &JsClosureSlot,
) -> HostCallback {
    let mut cb = Some(cb);
    let destination = owner.host_callback_once(
        move |_| {
            if let Some(cb) = cb.take() {
                cb()
            } else {
                Ok(())
            }
        },
        owner.error_handler(),
    );
    let destination_for_closure = destination.clone();
    let closure_for_callback = closure.clone();
    let callback = Closure::once_into_js(AssertUnwindSafe(move || {
        let _guard = OnceCallbackGuard {
            destination: destination_for_closure.clone(),
            closure: closure_for_callback.clone(),
        };
        let _ = destination_for_closure.dispatch(JsValue::UNDEFINED);
    }));
    *closure.borrow_mut() = Some(callback);
    destination
}

pub fn request_animation_frame<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() -> SilexResult<()> + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(None));
    let destination = owned_once_callback(owner, cb, &callback);
    let frame = {
        let callback_ref = callback.borrow();
        window().request_animation_frame(
            callback_ref
                .as_ref()
                .expect("owned animation callback is present")
                .unchecked_ref(),
        )
    };
    let frame = match frame {
        Ok(frame) => frame,
        Err(error) => {
            destination.cancel();
            let _ = callback.borrow_mut().take();
            return Err(error);
        }
    };
    let callback_for_cleanup = callback.clone();
    match owner.try_host_resource_for_callback(&destination, move || {
        let _ = window().cancel_animation_frame(frame);
        let _ = callback_for_cleanup.borrow_mut().take();
    }) {
        Ok(resource) => Ok(resource),
        Err(error) => Err(host_resource_error(error)),
    }
}

pub fn request_idle_callback<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() -> SilexResult<()> + 'scope,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(None));
    let destination = owned_once_callback(owner, cb, &callback);
    let idle = {
        let callback_ref = callback.borrow();
        window().request_idle_callback(
            callback_ref
                .as_ref()
                .expect("owned idle callback is present")
                .unchecked_ref(),
        )
    };
    let idle = match idle {
        Ok(idle) => idle,
        Err(error) => {
            destination.cancel();
            let _ = callback.borrow_mut().take();
            return Err(error);
        }
    };
    let callback_for_cleanup = callback.clone();
    match owner.try_host_resource_for_callback(&destination, move || {
        window().cancel_idle_callback(idle);
        let _ = callback_for_cleanup.borrow_mut().take();
    }) {
        Ok(resource) => Ok(resource),
        Err(error) => Err(host_resource_error(error)),
    }
}

pub fn queue_microtask<'scope>(
    owner: &ViewOwnerToken<'scope>,
    task: impl FnOnce() -> SilexResult<()> + 'scope,
) -> HostResourceHandle<'scope> {
    if !owner.is_active() {
        return HostResourceHandle::inactive();
    }
    let callback = Rc::new(RefCell::new(None));
    let destination = owned_once_callback(owner, task, &callback);
    let callback_for_cleanup = callback.clone();
    {
        let callback_ref = callback.borrow();
        let task = callback_ref
            .as_ref()
            .expect("owned microtask callback is present")
            .unchecked_ref::<js_sys::Function>();
        window().queue_microtask(task);
    }
    // Microtasks cannot be physically removed. The destination gate still
    // prevents user code after owner disposal.
    owner.host_resource_for_callback(&destination, move || {
        let _ = callback_for_cleanup.borrow_mut().take();
    })
}

pub fn set_timeout<'scope>(
    owner: &ViewOwnerToken<'scope>,
    cb: impl FnOnce() -> SilexResult<()> + 'scope,
    duration: Duration,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let callback = Rc::new(RefCell::new(None));
    let destination = owned_once_callback(owner, cb, &callback);
    let timeout = {
        let callback_ref = callback.borrow();
        window().set_timeout_with_callback_and_timeout_and_arguments_0(
            callback_ref
                .as_ref()
                .expect("owned timeout callback is present")
                .unchecked_ref(),
            duration_millis(duration),
        )
    };
    let timeout = match timeout {
        Ok(timeout) => timeout,
        Err(error) => {
            destination.cancel();
            let _ = callback.borrow_mut().take();
            return Err(error);
        }
    };
    let callback_for_cleanup = callback.clone();
    let window_for_cleanup = window();
    match owner.try_host_resource_for_callback(&destination, move || {
        window_for_cleanup.clear_timeout_with_handle(timeout);
        let _ = callback_for_cleanup.borrow_mut().take();
    }) {
        Ok(resource) => Ok(resource),
        Err(error) => Err(host_resource_error(error)),
    }
}

pub fn set_interval<'scope>(
    owner: &ViewOwnerToken<'scope>,
    mut cb: impl FnMut() -> SilexResult<()> + 'scope,
    duration: Duration,
) -> Result<HostResourceHandle<'scope>, JsValue> {
    if !owner.is_active() {
        return Err(JsValue::from_str("view owner is inactive"));
    }
    let destination = owner.host_callback(move |_| cb(), owner.error_handler());
    let destination_for_closure = AssertUnwindSafe(destination.clone());
    let closure: Closure<dyn FnMut()> = Closure::wrap(Box::new(move || {
        let _ = destination_for_closure.dispatch(JsValue::UNDEFINED);
    }));
    let closure = Rc::new(RefCell::new(Some(closure.into_js_value())));
    let js_fn = closure
        .borrow()
        .as_ref()
        .expect("owned interval callback is present")
        .unchecked_ref::<js_sys::Function>()
        .clone();
    let window = window();
    let interval = match window
        .set_interval_with_callback_and_timeout_and_arguments_0(&js_fn, duration_millis(duration))
    {
        Ok(interval) => interval,
        Err(error) => {
            destination.cancel();
            let _ = closure.borrow_mut().take();
            return Err(error);
        }
    };
    let closure_for_cleanup = closure.clone();
    let window_for_cleanup = window.clone();
    match owner.try_host_resource_for_callback(&destination, move || {
        window_for_cleanup.clear_interval_with_handle(interval);
        let _ = closure_for_cleanup.borrow_mut().take();
    }) {
        Ok(resource) => Ok(resource),
        Err(error) => Err(host_resource_error(error)),
    }
}

// --- Debounce ---

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

pub fn debounce<'scope, T, F>(
    owner: &ViewOwnerToken<'scope>,
    delay: Duration,
    mut cb: F,
) -> impl FnMut(T) + 'scope + use<'scope, T, F>
where
    T: 'scope,
    F: FnMut(T) -> SilexResult<()> + 'scope,
{
    let state = Rc::new(RefCell::new(DebounceState {
        pending: None,
        timer: None,
        generation: 0,
    }));
    let state_for_callback = state.clone();
    let error_handler = owner.error_handler();
    let destination = owner.host_callback(
        move |payload| {
            let Some(generation) = payload.as_f64() else {
                return Ok(());
            };
            let value = {
                let mut state = state_for_callback.borrow_mut();
                if state.generation as f64 != generation {
                    return Ok(());
                }
                if let Some(timer) = state.timer.take() {
                    drop(timer.callback);
                }
                state.pending.take()
            };
            if let Some(value) = value {
                cb(value)?;
            }
            Ok(())
        },
        owner.error_handler(),
    );

    let state_for_cleanup = state.clone();
    let resource = owner.host_resource_for_callback(&destination, move || {
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
        let callback = Closure::once_into_js(AssertUnwindSafe(move || {
            let _ = destination.dispatch(JsValue::from_f64(generation as f64));
        }));
        let timeout = window().set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            duration_millis(delay),
        );
        match timeout {
            Ok(id) => {
                state.borrow_mut().timer = Some(DebounceTimer { id, callback });
            }
            Err(error) => {
                let _ = state.borrow_mut().pending.take();
                error_handler.handle(error.into());
            }
        }
    }
}
