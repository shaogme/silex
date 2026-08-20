#![cfg(target_arch = "wasm32")]

use silex_core::{
    ErrorHandlerToken, ErrorReporter, OwnerAccess, OwnerHandle, Runtime, SilexError, SilexErrorKind,
};
use silex_dom::{
    attribute::{AttrOp, AttributeBuilder},
    document,
    element::{Element, bind_event},
    event::click,
    helpers::{
        debounce, queue_microtask, request_animation_frame, request_idle_callback, set_interval,
        set_timeout, window_event_listener_untyped,
    },
    view::{AnyView, MountOwner, MountOwnerToken, StatefulKeyedListView, View, mount_text_node},
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
    time::Duration,
};
use wasm_bindgen::{JsValue, prelude::*};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{Element as WebElement, Event, MouseEvent, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

fn test_owner<'owner>(
    owner: OwnerAccess<'owner>,
) -> (MountOwnerToken<'owner>, ErrorHandlerToken<'owner>) {
    let error_handler = test_handler(owner);
    (MountOwnerToken::new(owner), error_handler)
}

#[wasm_bindgen(inline_js = r#"
export function installHostSpy() {
    const spy = {
        counts: Object.create(null),
        originals: Object.create(null),
        targets: [],
        pending: new Map(),
        callbacks: [],
        failNextTimeout: false,
    };

    const bump = (key) => {
        spy.counts[key] = (spy.counts[key] || 0) + 1;
    };

    const wrapTimer = (setName, clearName, setKey, clearKey, invokeKey, repeat) => {
        const setOriginal = window[setName];
        const clearOriginal = window[clearName];
        spy.originals[setName] = setOriginal;
        spy.originals[clearName] = clearOriginal;
        if (typeof setOriginal !== "function" || typeof clearOriginal !== "function") {
            return;
        }

        window[setName] = function(callback, ...args) {
            bump(setKey);
            if (setName === "setTimeout" && spy.failNextTimeout) {
                spy.failNextTimeout = false;
                throw new Error("forced timeout creation failure");
            }
            let id;
            const wrapped = (...callbackArgs) => {
                bump(invokeKey);
                if (!repeat) {
                    spy.pending.delete(id);
                }
                return callback(...callbackArgs);
            };
            id = setOriginal.call(this, wrapped, ...args);
            if (setName === "setTimeout") {
                spy.callbacks.push({ id, callback: wrapped });
            }
            spy.pending.set(id, true);
            return id;
        };

        window[clearName] = function(id) {
            bump(clearKey);
            spy.pending.delete(id);
            return clearOriginal.call(this, id);
        };
    };

    wrapTimer("setTimeout", "clearTimeout", "timeout_set", "timeout_clear", "timeout_invoke", false);
    wrapTimer("setInterval", "clearInterval", "interval_set", "interval_clear", "interval_invoke", true);
    wrapTimer("requestAnimationFrame", "cancelAnimationFrame", "frame_request", "frame_cancel", "frame_invoke", false);
    wrapTimer("requestIdleCallback", "cancelIdleCallback", "idle_request", "idle_cancel", "idle_invoke", false);

    const queueMicrotaskOriginal = window.queueMicrotask;
    spy.originals.queueMicrotask = queueMicrotaskOriginal;
    if (typeof queueMicrotaskOriginal === "function") {
        window.queueMicrotask = function(callback) {
            bump("microtask_queue");
            return queueMicrotaskOriginal.call(this, () => {
                bump("microtask_invoke");
                return callback();
            });
        };
    }

    const addOriginal = window.addEventListener;
    const removeOriginal = window.removeEventListener;
    spy.originals.addEventListener = addOriginal;
    spy.originals.removeEventListener = removeOriginal;
    window.addEventListener = function(name, callback, options) {
        bump(`event_add:${name}`);
        return addOriginal.call(this, name, callback, options);
    };
    window.removeEventListener = function(name, callback, options) {
        bump(`event_remove:${name}`);
        return removeOriginal.call(this, name, callback, options);
    };

    return spy;
}

export function spyEventTarget(spy, target) {
    const addOriginal = target.addEventListener;
    const removeOriginal = target.removeEventListener;
    spy.targets.push({ target, addOriginal, removeOriginal });
    target.addEventListener = function(name, callback, options) {
        spy.counts[`event_add:${name}`] = (spy.counts[`event_add:${name}`] || 0) + 1;
        return addOriginal.call(this, name, callback, options);
    };
    target.removeEventListener = function(name, callback, options) {
        spy.counts[`event_remove:${name}`] = (spy.counts[`event_remove:${name}`] || 0) + 1;
        return removeOriginal.call(this, name, callback, options);
    };
}

export function spyCount(spy, key) {
    return spy.counts[key] || 0;
}

export function spyWait(spy, milliseconds) {
    return new Promise((resolve) => spy.originals.setTimeout.call(window, resolve, milliseconds));
}

export function failNextTimeout(spy) {
    spy.failNextTimeout = true;
}

export function fireTimeout(spy, index) {
    const entry = spy.callbacks[index];
    if (entry === undefined) {
        throw new Error(`timeout callback ${index} does not exist`);
    }
    return entry.callback();
}

export function restoreHostSpy(spy) {
    for (const [name, original] of Object.entries(spy.originals)) {
        if (name === "setTimeout" || name === "clearTimeout" ||
            name === "setInterval" || name === "clearInterval" ||
            name === "requestAnimationFrame" || name === "cancelAnimationFrame" ||
            name === "requestIdleCallback" || name === "cancelIdleCallback" ||
            name === "queueMicrotask" || name === "addEventListener" ||
            name === "removeEventListener") {
            window[name] = original;
        }
    }
    for (const { target, addOriginal, removeOriginal } of spy.targets) {
        target.addEventListener = addOriginal;
        target.removeEventListener = removeOriginal;
    }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installHostSpy)]
    fn install_host_spy() -> JsValue;

    #[wasm_bindgen(js_name = spyEventTarget)]
    fn spy_event_target(spy: &JsValue, target: &JsValue);

    #[wasm_bindgen(js_name = spyCount)]
    fn spy_count(spy: &JsValue, key: &str) -> u32;

    #[wasm_bindgen(js_name = spyWait)]
    fn spy_wait(spy: &JsValue, milliseconds: u32) -> js_sys::Promise;

    #[wasm_bindgen(js_name = failNextTimeout)]
    fn fail_next_timeout(spy: &JsValue);

    #[wasm_bindgen(js_name = fireTimeout)]
    fn fire_timeout(spy: &JsValue, index: u32);

    #[wasm_bindgen(js_name = restoreHostSpy)]
    fn restore_host_spy(spy: &JsValue);
}

struct Spy {
    value: JsValue,
}

impl Spy {
    fn new() -> Self {
        Self {
            value: install_host_spy(),
        }
    }

    fn count(&self, key: &str) -> u32 {
        spy_count(&self.value, key)
    }

    fn fail_next_timeout(&self) {
        fail_next_timeout(&self.value);
    }

    fn fire_timeout(&self, index: u32) {
        fire_timeout(&self.value, index);
    }

    async fn wait(&self, milliseconds: u32) {
        JsFuture::from(spy_wait(&self.value, milliseconds))
            .await
            .expect("spy wait should resolve");
    }

    fn spy_target(&self, target: &JsValue) {
        spy_event_target(&self.value, target);
    }
}

impl Drop for Spy {
    fn drop(&mut self) {
        restore_host_spy(&self.value);
    }
}

struct DropProbe {
    drops: Rc<Cell<u32>>,
}

struct WindowResourceView {
    id: i32,
    calls: Rc<RefCell<Vec<i32>>>,
}

impl<'owner> silex_dom::view::ApplyAttributes<'owner> for WindowResourceView {}

impl<'owner> View<'owner> for WindowResourceView {
    fn mount(
        &self,
        owner: &dyn MountOwner<'owner>,
        parent: &Node,
        _attrs: Vec<silex_dom::attribute::AttrOp<'owner>>,
        error_handler: ErrorReporter<'owner>,
    ) -> silex_core::SilexResult<silex_dom::view::MountInstance<'owner>> {
        let calls = self.calls.clone();
        let id = self.id;
        window_event_listener_untyped(
            &owner.token(),
            "silex-window-resource",
            move |_| {
                calls.borrow_mut().push(id);
                Ok(())
            },
            error_handler,
        )
        .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
        mount_text_node(parent, &self.id.to_string())
    }
}

impl DropProbe {
    fn new(drops: Rc<Cell<u32>>) -> Self {
        Self { drops }
    }
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

fn mount_point() -> WebElement {
    let document = web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available");
    let host = document.create_element("div").expect("host can be created");
    document
        .body()
        .expect("body is available")
        .append_child(&host)
        .expect("host can be mounted");
    host
}

fn remove_mount_point(host: &WebElement) {
    if let Some(parent) = host.parent_node() {
        parent.remove_child(host).expect("host can be removed");
    }
}

fn dispatch(target: &Node, event: Event) {
    target
        .dispatch_event(&event)
        .expect("event dispatch should succeed");
}

#[wasm_bindgen_test]
fn fallible_dom_primitives_and_attribute_mount_failures_are_observable() {
    let host = mount_point();
    let text_parent: Node = document().create_text_node("not a parent").into();
    assert!(mount_text_node(&text_parent, "rejected").is_err());

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (owner, error_handler) = test_owner(owner);
            let token = owner.token();
            let element = document()
                .create_element("div")
                .expect("test element can be created");
            let invalid_class = AttrOp::static_class("invalid token".into());
            assert!(
                invalid_class
                    .apply(&element, &token, error_handler)
                    .is_err()
            );
        })
        .expect("child owner should initialize");

    let reported = Rc::new(Cell::new(false));
    let reported_for_owner = reported.clone();
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let error_handler = owner
                .error_handler(move |error| {
                    reported_for_owner.set(matches!(error, SilexError::Recoverable(SilexErrorKind::Framework(_))));
                })
                .expect("error handler should register");
            let owner = MountOwnerToken::new(owner);
            let view = Element::new("div").apply(AttrOp::new_scoped(|_, _, _| {
                Err(SilexError::recoverable(SilexErrorKind::Framework("attribute rejected".to_string())))
            }));
            assert!(matches!(
                view.mount(&owner, &host.clone().into(), Vec::new(), error_handler.view()),
                Err(SilexError::Recoverable(SilexErrorKind::Framework(message))) if message == "attribute rejected"
            ));
        })
        .expect("child owner should initialize");
    assert!(!reported.get());
    assert!(host.first_child().is_none());
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn element_listener_removes_physically_and_drops_on_root_dispose() {
    let spy = Spy::new();
    let host = mount_point();
    let element_slot = Rc::new(RefCell::new(None::<WebElement>));
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let access = root.access();
        let node_ref = access
            .node_ref::<WebElement>()
            .expect("node ref should initialize");
        let (owner, error_handler) = test_owner(access);
        let token = owner.token();
        let element = Element::new("button");
        let instance = element
            .mount(
                &owner,
                &host.clone().into(),
                Vec::new(),
                error_handler.view(),
            )
            .expect("element should mount");
        let mounted_element = instance
            .first_node()
            .expect("mount should contain the element")
            .clone()
            .unchecked_into::<WebElement>();
        let element_node: Node = mounted_element.clone().into();
        spy.spy_target(&mounted_element);
        *element_slot.borrow_mut() = Some(mounted_element.clone());

        let calls_for_handler = calls.clone();
        let probe = DropProbe::new(drops.clone());
        bind_event(
            &mounted_element,
            click,
            move |_| {
                calls_for_handler.set(calls_for_handler.get() + 1);
                let _ = &probe;
                Ok(())
            },
            &token,
            &error_handler,
        )
        .expect("element listener can be registered");

        node_ref
            .load(mounted_element.clone())
            .expect("node ref should load");
        assert!(
            node_ref
                .get()
                .expect("node ref should be readable")
                .is_some()
        );
        dispatch(&element_node, MouseEvent::new("click").unwrap().into());
        assert_eq!(calls.get(), 1);
    }

    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("event_add:click"), 1);
    assert_eq!(spy.count("event_remove:click"), 1);
    assert_eq!(drops.get(), 1);

    let element = element_slot
        .borrow()
        .as_ref()
        .expect("element is retained for assertion")
        .clone();
    dispatch(&element.into(), MouseEvent::new("click").unwrap().into());
    assert_eq!(calls.get(), 1);
    assert_eq!(spy.count("event_remove:click"), 1);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn element_listener_panic_closes_destination_before_owner_cleanup() {
    let spy = Spy::new();
    let host = mount_point();
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element_node: Node;

    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let token = owner.token();
        let element = document()
            .create_element("button")
            .expect("button should be creatable");
        spy.spy_target(&element);
        element_node = element.clone().into();
        let calls_for_callback = calls.clone();
        let probe = DropProbe::new(drops.clone());

        bind_event(
            &element,
            click,
            move |_| {
                calls_for_callback.set(calls_for_callback.get() + 1);
                let _ = &probe;
                panic!("element callback panic");
            },
            &token,
            &error_handler,
        )
        .expect("element listener should register");
    }

    // DOM dispatch reports whether the event was canceled; exceptions thrown by
    // listeners are reported separately and do not turn dispatch into an error.
    element_node
        .dispatch_event(&MouseEvent::new("click").unwrap())
        .expect("event dispatch should succeed");
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);

    dispatch(&element_node, MouseEvent::new("click").unwrap().into());
    assert_eq!(calls.get(), 1);

    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:click"), 1);
    assert_eq!(drops.get(), 1);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn render_rerun_replaces_old_window_listener() {
    let spy = Spy::new();
    let host = mount_point();
    let host_node: Node = host.clone().into();
    let calls = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        let (value, set_value) = owner.signal(0i32).expect("signal should initialize");
        let (owner, error_handler) = test_owner(owner);
        let calls_for_view = calls.clone();
        let view = move || WindowResourceView {
            id: value.get().expect("signal should be readable"),
            calls: calls_for_view.clone(),
        };
        let _ = view
            .mount(&owner, &host_node, Vec::new(), error_handler.view())
            .expect("view should mount");

        let window = web_sys::window().expect("window is available");
        window
            .dispatch_event(&Event::new("silex-window-resource").unwrap())
            .expect("window event dispatch should succeed");
        set_value.set(1).expect("signal should be writable");
        assert_eq!(spy.count("event_add:silex-window-resource"), 2);
        assert_eq!(spy.count("event_remove:silex-window-resource"), 1);
        window
            .dispatch_event(&Event::new("silex-window-resource").unwrap())
            .expect("window event dispatch should succeed");
    }

    assert_eq!(&*calls.borrow(), &[0, 1]);
    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn lexical_owner_disposes_window_listener_on_scope_exit() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let (owner, error_handler) = test_owner(owner);
            let calls_for_handler = calls.clone();
            window_event_listener_untyped(
                &owner.token(),
                "silex-lexical-resource",
                move |_| {
                    calls_for_handler.set(calls_for_handler.get() + 1);
                    Ok(())
                },
                error_handler,
            )
            .expect("lexical window listener should register");
            let window = web_sys::window().expect("window is available");
            window
                .dispatch_event(&Event::new("silex-lexical-resource").unwrap())
                .expect("window event dispatch should succeed");
        })
        .expect("child owner should initialize");

    let window = web_sys::window().expect("window is available");
    window
        .dispatch_event(&Event::new("silex-lexical-resource").unwrap())
        .expect("window event dispatch should succeed");
    assert_eq!(calls.get(), 1);
    assert_eq!(spy.count("event_add:silex-lexical-resource"), 1);
    assert_eq!(spy.count("event_remove:silex-lexical-resource"), 1);
}

#[wasm_bindgen_test]
fn keyed_reorder_keeps_window_resources_until_row_delete() {
    let spy = Spy::new();
    let host = mount_point();
    let host_node: Node = host.clone().into();
    let calls = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        owner
            .with_transient(|child| {
                let (items, set_items) = child
                    .signal(vec![1i32, 2])
                    .expect("signal should initialize");
                let calls_for_factory = calls.clone();
                let list = StatefulKeyedListView {
                    each: items,
                    key_fn: Rc::new(|item: &i32| *item),
                    view_fn: Rc::new(move |item: i32, _, updater| {
                        assert!(updater.bind(|_, _| {}));
                        AnyView::new(WindowResourceView {
                            id: item,
                            calls: calls_for_factory.clone(),
                        })
                    }),
                    error_handler: None,
                    _marker: PhantomData,
                };
                let (owner, error_handler) = test_owner(child);
                let _ = list
                    .mount(&owner, &host_node, Vec::new(), error_handler.view())
                    .expect("keyed list should mount");
                assert_eq!(spy.count("event_add:silex-window-resource"), 2);

                set_items
                    .set(vec![2, 1])
                    .expect("signal should be writable");
                assert_eq!(spy.count("event_remove:silex-window-resource"), 0);
                set_items.set(vec![2]).expect("signal should be writable");
                assert_eq!(spy.count("event_remove:silex-window-resource"), 1);
            })
            .expect("child owner should initialize");
    }

    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn window_listener_cancel_is_idempotent_and_owner_keeps_final_control() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let handle_slot = Rc::new(RefCell::new(None));
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let calls_for_handler = calls.clone();
        let probe = DropProbe::new(drops.clone());
        let handle = window_event_listener_untyped(
            &owner.token(),
            "silex-resize",
            move |_| {
                calls_for_handler.set(calls_for_handler.get() + 1);
                let _ = &probe;
                Ok(())
            },
            &error_handler,
        )
        .expect("window listener should register");
        *handle_slot.borrow_mut() = Some(handle);
    }

    let window = web_sys::window().expect("window is available");
    window
        .dispatch_event(&Event::new("silex-resize").unwrap())
        .expect("window event dispatch should succeed");
    assert_eq!(calls.get(), 1);

    handle_slot
        .borrow()
        .as_ref()
        .expect("listener handle is retained")
        .cancel()
        .expect("listener cancellation should succeed");
    handle_slot
        .borrow()
        .as_ref()
        .expect("listener handle is retained")
        .cancel()
        .expect("repeated listener cancellation should succeed");
    window
        .dispatch_event(&Event::new("silex-resize").unwrap())
        .expect("window event dispatch should succeed");
    assert_eq!(calls.get(), 1);
    assert_eq!(spy.count("event_add:silex-resize"), 1);
    assert_eq!(spy.count("event_remove:silex-resize"), 1);

    drop(handle_slot);
    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:silex-resize"), 1);
    assert_eq!(drops.get(), 1);
}

#[wasm_bindgen_test(async)]
async fn cancelable_host_tasks_are_cleared_before_dispatch() {
    let spy = Spy::new();
    let timeout_calls = Rc::new(Cell::new(0));
    let interval_calls = Rc::new(Cell::new(0));
    let frame_calls = Rc::new(Cell::new(0));
    let idle_calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);

        let timeout_calls_for_callback = timeout_calls.clone();
        let timeout_probe = DropProbe::new(drops.clone());
        set_timeout(
            &owner.token(),
            move || {
                timeout_calls_for_callback.set(timeout_calls_for_callback.get() + 1);
                let _ = &timeout_probe;
                Ok(())
            },
            Duration::from_millis(100),
            &error_handler,
        )
        .expect("timeout should register");

        let interval_calls_for_callback = interval_calls.clone();
        let interval_probe = DropProbe::new(drops.clone());
        set_interval(
            &owner.token(),
            move || {
                interval_calls_for_callback.set(interval_calls_for_callback.get() + 1);
                let _ = &interval_probe;
                Ok(())
            },
            Duration::from_millis(100),
            &error_handler,
        )
        .expect("interval should register");

        let frame_calls_for_callback = frame_calls.clone();
        let frame_probe = DropProbe::new(drops.clone());
        request_animation_frame(
            &owner.token(),
            move || {
                frame_calls_for_callback.set(frame_calls_for_callback.get() + 1);
                let _ = &frame_probe;
                Ok(())
            },
            &error_handler,
        )
        .expect("animation frame should register");

        let idle_calls_for_callback = idle_calls.clone();
        let idle_probe = DropProbe::new(drops.clone());
        request_idle_callback(
            &owner.token(),
            move || {
                idle_calls_for_callback.set(idle_calls_for_callback.get() + 1);
                let _ = &idle_probe;
                Ok(())
            },
            &error_handler,
        )
        .expect("idle callback should register");
    }

    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("timeout_set"), 1);
    assert_eq!(spy.count("timeout_clear"), 1);
    assert_eq!(spy.count("interval_set"), 1);
    assert_eq!(spy.count("interval_clear"), 1);
    assert_eq!(spy.count("frame_request"), 1);
    assert_eq!(spy.count("frame_cancel"), 1);
    assert_eq!(spy.count("idle_request"), 1);
    assert_eq!(spy.count("idle_cancel"), 1);

    spy.wait(150).await;
    assert_eq!(timeout_calls.get(), 0);
    assert_eq!(interval_calls.get(), 0);
    assert_eq!(frame_calls.get(), 0);
    assert_eq!(idle_calls.get(), 0);
    assert_eq!(spy.count("timeout_invoke"), 0);
    assert_eq!(spy.count("interval_invoke"), 0);
    assert_eq!(spy.count("frame_invoke"), 0);
    assert_eq!(spy.count("idle_invoke"), 0);
    assert_eq!(drops.get(), 4);
}

#[wasm_bindgen_test(async)]
async fn active_host_tasks_execute_and_interval_cancel_is_idempotent() {
    let spy = Spy::new();
    let timeout_calls = Rc::new(Cell::new(0));
    let interval_calls = Rc::new(Cell::new(0));
    let frame_calls = Rc::new(Cell::new(0));
    let idle_calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let interval_slot = Rc::new(RefCell::new(None));
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let timeout_calls_for_callback = timeout_calls.clone();
        set_timeout(
            &owner.token(),
            move || {
                timeout_calls_for_callback.set(timeout_calls_for_callback.get() + 1);
                Ok(())
            },
            Duration::from_millis(0),
            &error_handler,
        )
        .expect("timeout should register");

        let interval_calls_for_callback = interval_calls.clone();
        let interval = set_interval(
            &owner.token(),
            move || {
                interval_calls_for_callback.set(interval_calls_for_callback.get() + 1);
                Ok(())
            },
            Duration::from_millis(5),
            &error_handler,
        )
        .expect("interval should register");
        *interval_slot.borrow_mut() = Some(interval);

        let frame_calls_for_callback = frame_calls.clone();
        request_animation_frame(
            &owner.token(),
            move || {
                frame_calls_for_callback.set(frame_calls_for_callback.get() + 1);
                Ok(())
            },
            &error_handler,
        )
        .expect("animation frame should register");

        let idle_calls_for_callback = idle_calls.clone();
        request_idle_callback(
            &owner.token(),
            move || {
                idle_calls_for_callback.set(idle_calls_for_callback.get() + 1);
                Ok(())
            },
            &error_handler,
        )
        .expect("idle callback should register");
    }

    spy.wait(50).await;
    assert_eq!(timeout_calls.get(), 1);
    assert!(interval_calls.get() >= 2);
    assert_eq!(frame_calls.get(), 1);
    assert_eq!(idle_calls.get(), 1);

    interval_slot
        .borrow()
        .as_ref()
        .expect("interval handle is retained")
        .cancel()
        .expect("interval cancellation should succeed");
    interval_slot
        .borrow()
        .as_ref()
        .expect("interval handle is retained")
        .cancel()
        .expect("repeated interval cancellation should succeed");
    let interval_calls_after_cancel = interval_calls.get();
    spy.wait(30).await;
    assert_eq!(interval_calls.get(), interval_calls_after_cancel);
    assert_eq!(spy.count("interval_clear"), 1);

    drop(interval_slot);
    root.close().expect("root disposal should succeed");
    assert_eq!(spy.count("interval_clear"), 1);
}

#[wasm_bindgen_test(async)]
async fn microtask_cancel_and_owner_dispose_only_gate_user_callbacks() {
    let spy = Spy::new();
    let canceled_calls = Rc::new(Cell::new(0));
    let disposed_calls = Rc::new(Cell::new(0));
    let queued_before = spy.count("microtask_queue");
    let invoked_before = spy.count("microtask_invoke");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let canceled_slot = Rc::new(RefCell::new(None));
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let canceled_calls_for_task = canceled_calls.clone();
        let canceled = queue_microtask(
            &owner.token(),
            move || {
                canceled_calls_for_task.set(canceled_calls_for_task.get() + 1);
                Ok(())
            },
            &error_handler,
        )
        .expect("microtask should queue");
        *canceled_slot.borrow_mut() = Some(canceled);

        let disposed_calls_for_task = disposed_calls.clone();
        queue_microtask(
            &owner.token(),
            move || {
                disposed_calls_for_task.set(disposed_calls_for_task.get() + 1);
                Ok(())
            },
            error_handler,
        )
        .expect("microtask should queue");
    }

    canceled_slot
        .borrow()
        .as_ref()
        .expect("microtask handle is retained")
        .cancel()
        .expect("microtask cancellation should succeed");
    drop(canceled_slot);
    root.close().expect("root disposal should succeed");
    spy.wait(0).await;

    assert!(spy.count("microtask_queue") - queued_before >= 2);
    assert!(spy.count("microtask_invoke") - invoked_before >= 2);
    assert_eq!(canceled_calls.get(), 0);
    assert_eq!(disposed_calls.get(), 0);
}

#[wasm_bindgen_test(async)]
async fn debounce_clears_replaced_timer_and_blocks_dispose_completion() {
    let spy = Spy::new();
    let values = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let debounce_slot = Rc::new(RefCell::new(None));
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let token = owner.token();
        let values_for_callback = values.clone();
        let debounce = debounce(
            &token,
            Duration::from_millis(10),
            move |value: i32| {
                values_for_callback.borrow_mut().push(value);
                Ok(())
            },
            error_handler,
        )
        .expect("debounce should initialize");
        *debounce_slot.borrow_mut() = Some(debounce);
    }

    {
        let mut debounce = debounce_slot.borrow_mut();
        let callback = debounce.as_mut().expect("debounce is retained");
        callback(1);
        callback(2);
        callback(3);
    }
    spy.wait(40).await;
    assert_eq!(&*values.borrow(), &[3]);
    assert_eq!(spy.count("timeout_set"), 3);
    assert_eq!(spy.count("timeout_clear"), 2);

    {
        let mut debounce = debounce_slot.borrow_mut();
        debounce.as_mut().expect("debounce is retained")(4);
    }
    drop(debounce_slot);
    root.close().expect("root disposal should succeed");
    spy.wait(30).await;
    assert_eq!(&*values.borrow(), &[3]);
    assert_eq!(spy.count("timeout_clear"), 3);
}

#[wasm_bindgen_test]
fn debounce_timeout_creation_failure_reaches_owner_handler() {
    let spy = Spy::new();
    let errors = Rc::new(RefCell::new(Vec::<SilexError>::new()));
    let errors_for_reporter = errors.clone();
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let error_handler = owner
                .error_handler(move |error| errors_for_reporter.borrow_mut().push(error))
                .expect("error handler should register");
            let owner = silex_dom::view::MountOwnerToken::new(owner);
            let token = owner.token();
            let mut debounce = debounce(
                &token,
                Duration::from_millis(0),
                |_| Ok(()),
                error_handler.view(),
            )
            .expect("debounce should initialize");
            spy.fail_next_timeout();
            debounce(1_i32);
        })
        .expect("child owner should initialize");

    assert!(matches!(
        errors.borrow().as_slice(),
        [SilexError::Fatal(SilexErrorKind::Javascript(message))] if message.contains("forced timeout creation failure")
    ));
}

#[wasm_bindgen_test(async)]
async fn timer_callback_can_reenter_root_dispose_without_late_registration() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let dispose_slot = Rc::new(RefCell::new(None::<OwnerHandle>));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);
        let token = owner.token();

        let calls_for_callback = calls.clone();
        let dispose_for_callback = dispose_slot.clone();
        set_timeout(
            &token,
            move || {
                calls_for_callback.set(calls_for_callback.get() + 1);
                if let Some(root) = dispose_for_callback.borrow_mut().take() {
                    root.close()
                        .expect("reentrant root disposal should succeed");
                }
                Ok(())
            },
            Duration::from_millis(0),
            error_handler,
        )
        .expect("reentrant timeout should register");
    }
    *dispose_slot.borrow_mut() = Some(root);

    spy.wait(0).await;
    assert_eq!(calls.get(), 1);
    assert_eq!(spy.count("timeout_set"), 1);
    assert_eq!(spy.count("timeout_invoke"), 1);
}

#[wasm_bindgen_test]
fn timeout_lifecycle_handles_creation_failure_repeated_cancel_reentry_and_stale_callbacks() {
    let spy = Spy::new();
    let failed_calls = Rc::new(Cell::new(0));
    let canceled_calls = Rc::new(Cell::new(0));
    let reentrant_calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let dispose_slot = Rc::new(RefCell::new(None::<OwnerHandle>));
    let values = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();

    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        let (owner, error_handler) = test_owner(owner);

        spy.fail_next_timeout();
        let failed_calls_for_callback = failed_calls.clone();
        let failed_probe = DropProbe::new(drops.clone());
        assert!(
            set_timeout(
                &owner.token(),
                move || {
                    failed_calls_for_callback.set(failed_calls_for_callback.get() + 1);
                    let _ = &failed_probe;
                    Ok(())
                },
                Duration::from_millis(0),
                &error_handler,
            )
            .is_err()
        );
        assert_eq!(failed_calls.get(), 0);
        assert_eq!(drops.get(), 1);

        let canceled_calls_for_callback = canceled_calls.clone();
        let canceled_probe = DropProbe::new(drops.clone());
        let canceled = set_timeout(
            &owner.token(),
            move || {
                canceled_calls_for_callback.set(canceled_calls_for_callback.get() + 1);
                let _ = &canceled_probe;
                Ok(())
            },
            Duration::from_millis(0),
            &error_handler,
        )
        .expect("cancelable timeout should register");
        canceled
            .cancel()
            .expect("timeout cancellation should succeed");
        canceled
            .cancel()
            .expect("repeated timeout cancellation should succeed");
        spy.fire_timeout(0);
        assert_eq!(canceled_calls.get(), 0);

        let values_for_debounce = values.clone();
        let mut debounce = debounce(
            &owner.token(),
            Duration::from_millis(0),
            move |value| {
                values_for_debounce.borrow_mut().push(value);
                Ok(())
            },
            &error_handler,
        )
        .expect("debounce should initialize");
        debounce(1);
        debounce(2);

        let reentrant_calls_for_callback = reentrant_calls.clone();
        let dispose_for_callback = dispose_slot.clone();
        set_timeout(
            &owner.token(),
            move || {
                reentrant_calls_for_callback.set(reentrant_calls_for_callback.get() + 1);
                if let Some(root) = dispose_for_callback.borrow_mut().take() {
                    root.close()
                        .expect("reentrant root disposal should succeed");
                }
                Ok(())
            },
            Duration::from_millis(0),
            &error_handler,
        )
        .expect("reentrant timeout should register");
    }

    *dispose_slot.borrow_mut() = Some(root);

    spy.fire_timeout(1);
    assert!(
        values.borrow().is_empty(),
        "cleared debounce callback is stale"
    );
    spy.fire_timeout(3);
    assert_eq!(reentrant_calls.get(), 1);
    spy.fire_timeout(2);
    assert!(values.borrow().is_empty(), "late callback must stay gated");
    assert_eq!(failed_calls.get(), 0);
    assert_eq!(canceled_calls.get(), 0);
    assert_eq!(drops.get(), 2);
    assert_eq!(spy.count("timeout_clear"), 4);
    assert!(dispose_slot.borrow().is_none());
}
