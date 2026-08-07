#![cfg(target_arch = "wasm32")]

use silex_core::{ErrorHandler, ErrorReporter, RootHandle, Runtime, SilexError};
use silex_dom::{
    attribute::{AttrOp, AttributeBuilder, PendingAttribute},
    document,
    element::{Element, bind_event},
    event::click,
    helpers::{
        debounce_owned, queue_microtask_owned, request_animation_frame_owned,
        request_idle_callback_owned, set_interval_owned, set_timeout_owned,
        window_event_listener_untyped_owned,
    },
    view::{AnyView, KeyedLoopView, ScopedViewOwner, View, ViewOwner, mount_text_node},
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

fn test_handler<'scope>() -> ErrorReporter<'scope> {
    ErrorHandler::new(|_| {})
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

impl<'scope> silex_dom::view::ApplyAttributes<'scope> for WindowResourceView {}

impl<'scope> View<'scope> for WindowResourceView {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let calls = self.calls.clone();
        let id = self.id;
        window_event_listener_untyped_owned(&owner.token(), "silex-window-resource", move |_| {
            calls.borrow_mut().push(id);
            Ok(())
        })?;
        mount_text_node(parent, &self.id.to_string())?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
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
    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();
        let element = document()
            .create_element("div")
            .expect("test element can be created");
        let invalid_class = AttrOp::static_class("invalid token".into());
        assert!(invalid_class.apply(&element, &token).is_err());
    });

    let reported = Rc::new(Cell::new(false));
    let reported_for_owner = reported.clone();
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(
            scope,
            ErrorReporter::new(move |error| {
                reported_for_owner.set(matches!(error, SilexError::Framework(_)));
            }),
        );
        let view = Element::new("div").apply(PendingAttribute::new_scoped(|_, _| {
            Err(SilexError::Framework("attribute rejected".to_string()))
        }));
        assert!(matches!(
            view.mount_owned(&owner, &host.clone().into(), Vec::new()),
            Err(SilexError::Framework(message)) if message == "attribute rejected"
        ));
    });
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

    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();
        let element = Element::new("button");
        spy.spy_target(element.dom_element.as_ref());
        let element_node: Node = element.dom_element.clone().into();
        *element_slot.borrow_mut() = Some(element.dom_element.clone());

        let node_ref = scope.node_ref::<WebElement>();
        let calls_for_handler = calls.clone();
        let probe = DropProbe::new(drops.clone());
        bind_event(
            &element.dom_element,
            click,
            move |_| {
                calls_for_handler.set(calls_for_handler.get() + 1);
                let _ = &probe;
                Ok(())
            },
            &token,
        )
        .expect("element listener can be registered");

        node_ref.load(element.dom_element.clone());
        element
            .mount_owned(&owner, &host.clone().into(), Vec::new())
            .expect("element should mount");
        assert!(node_ref.get().is_some());
        dispatch(&element_node, MouseEvent::new("click").unwrap().into());
        assert_eq!(calls.get(), 1);
    }

    root.dispose().expect("root disposal should succeed");
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
fn render_rerun_replaces_old_window_listener() {
    let spy = Spy::new();
    let host = mount_point();
    let host_node: Node = host.clone().into();
    let calls = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();

    let root = runtime.run();
    {
        let scope = root.scope();
        let (value, set_value) = scope.signal(0i32);
        let owner = ScopedViewOwner::new(scope, test_handler());
        let calls_for_view = calls.clone();
        let view = move || WindowResourceView {
            id: value.get(),
            calls: calls_for_view.clone(),
        };
        view.mount_owned(&owner, &host_node, Vec::new())
            .expect("view should mount");

        let window = web_sys::window().expect("window is available");
        window
            .dispatch_event(&Event::new("silex-window-resource").unwrap())
            .expect("window event dispatch should succeed");
        set_value.set(1);
        assert_eq!(spy.count("event_add:silex-window-resource"), 2);
        assert_eq!(spy.count("event_remove:silex-window-resource"), 1);
        window
            .dispatch_event(&Event::new("silex-window-resource").unwrap())
            .expect("window event dispatch should succeed");
    }

    assert_eq!(&*calls.borrow(), &[0, 1]);
    root.dispose().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn lexical_owner_disposes_window_listener_on_scope_exit() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope, test_handler());
        let calls_for_handler = calls.clone();
        window_event_listener_untyped_owned(&owner.token(), "silex-lexical-resource", move |_| {
            calls_for_handler.set(calls_for_handler.get() + 1);
            Ok(())
        })
        .expect("lexical window listener should register");
        let window = web_sys::window().expect("window is available");
        window
            .dispatch_event(&Event::new("silex-lexical-resource").unwrap())
            .expect("window event dispatch should succeed");
    });

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

    let root = runtime.run();
    {
        let scope = root.scope();
        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32, 2]);
            let calls_for_factory = calls.clone();
            let list = KeyedLoopView {
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
            let owner = ScopedViewOwner::new(child, test_handler());
            list.mount_owned(&owner, &host_node, Vec::new())
                .expect("keyed list should mount");
            assert_eq!(spy.count("event_add:silex-window-resource"), 2);

            set_items.set(vec![2, 1]);
            assert_eq!(spy.count("event_remove:silex-window-resource"), 0);
            set_items.set(vec![2]);
            assert_eq!(spy.count("event_remove:silex-window-resource"), 1);
        });
    }

    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    root.dispose().expect("root disposal should succeed");
    assert_eq!(spy.count("event_remove:silex-window-resource"), 2);
    remove_mount_point(&host);
}

#[wasm_bindgen_test]
fn window_listener_cancel_is_idempotent_and_owner_keeps_final_control() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let handle_slot = Rc::new(RefCell::new(None));
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let calls_for_handler = calls.clone();
        let probe = DropProbe::new(drops.clone());
        let handle =
            window_event_listener_untyped_owned(&owner.token(), "silex-resize", move |_| {
                calls_for_handler.set(calls_for_handler.get() + 1);
                let _ = &probe;
                Ok(())
            })
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
        .cancel();
    handle_slot
        .borrow()
        .as_ref()
        .expect("listener handle is retained")
        .cancel();
    window
        .dispatch_event(&Event::new("silex-resize").unwrap())
        .expect("window event dispatch should succeed");
    assert_eq!(calls.get(), 1);
    assert_eq!(spy.count("event_add:silex-resize"), 1);
    assert_eq!(spy.count("event_remove:silex-resize"), 1);

    drop(handle_slot);
    root.dispose().expect("root disposal should succeed");
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

    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());

        let timeout_calls_for_callback = timeout_calls.clone();
        let timeout_probe = DropProbe::new(drops.clone());
        set_timeout_owned(
            &owner.token(),
            move || {
                timeout_calls_for_callback.set(timeout_calls_for_callback.get() + 1);
                let _ = &timeout_probe;
                Ok(())
            },
            Duration::from_millis(100),
        )
        .expect("timeout should register");

        let interval_calls_for_callback = interval_calls.clone();
        let interval_probe = DropProbe::new(drops.clone());
        set_interval_owned(
            &owner.token(),
            move || {
                interval_calls_for_callback.set(interval_calls_for_callback.get() + 1);
                let _ = &interval_probe;
                Ok(())
            },
            Duration::from_millis(100),
        )
        .expect("interval should register");

        let frame_calls_for_callback = frame_calls.clone();
        let frame_probe = DropProbe::new(drops.clone());
        request_animation_frame_owned(&owner.token(), move || {
            frame_calls_for_callback.set(frame_calls_for_callback.get() + 1);
            let _ = &frame_probe;
            Ok(())
        })
        .expect("animation frame should register");

        let idle_calls_for_callback = idle_calls.clone();
        let idle_probe = DropProbe::new(drops.clone());
        request_idle_callback_owned(&owner.token(), move || {
            idle_calls_for_callback.set(idle_calls_for_callback.get() + 1);
            let _ = &idle_probe;
            Ok(())
        })
        .expect("idle callback should register");
    }

    root.dispose().expect("root disposal should succeed");
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
    let root = runtime.run();
    let interval_slot = Rc::new(RefCell::new(None));
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let timeout_calls_for_callback = timeout_calls.clone();
        set_timeout_owned(
            &owner.token(),
            move || {
                timeout_calls_for_callback.set(timeout_calls_for_callback.get() + 1);
                Ok(())
            },
            Duration::from_millis(0),
        )
        .expect("timeout should register");

        let interval_calls_for_callback = interval_calls.clone();
        let interval = set_interval_owned(
            &owner.token(),
            move || {
                interval_calls_for_callback.set(interval_calls_for_callback.get() + 1);
                Ok(())
            },
            Duration::from_millis(5),
        )
        .expect("interval should register");
        *interval_slot.borrow_mut() = Some(interval);

        let frame_calls_for_callback = frame_calls.clone();
        request_animation_frame_owned(&owner.token(), move || {
            frame_calls_for_callback.set(frame_calls_for_callback.get() + 1);
            Ok(())
        })
        .expect("animation frame should register");

        let idle_calls_for_callback = idle_calls.clone();
        request_idle_callback_owned(&owner.token(), move || {
            idle_calls_for_callback.set(idle_calls_for_callback.get() + 1);
            Ok(())
        })
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
        .cancel();
    interval_slot
        .borrow()
        .as_ref()
        .expect("interval handle is retained")
        .cancel();
    let interval_calls_after_cancel = interval_calls.get();
    spy.wait(30).await;
    assert_eq!(interval_calls.get(), interval_calls_after_cancel);
    assert_eq!(spy.count("interval_clear"), 1);

    drop(interval_slot);
    root.dispose().expect("root disposal should succeed");
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
    let root = runtime.run();
    let canceled_slot = Rc::new(RefCell::new(None));
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let canceled_calls_for_task = canceled_calls.clone();
        let canceled = queue_microtask_owned(&owner.token(), move || {
            canceled_calls_for_task.set(canceled_calls_for_task.get() + 1);
            Ok(())
        });
        *canceled_slot.borrow_mut() = Some(canceled);

        let disposed_calls_for_task = disposed_calls.clone();
        queue_microtask_owned(&owner.token(), move || {
            disposed_calls_for_task.set(disposed_calls_for_task.get() + 1);
            Ok(())
        });
    }

    canceled_slot
        .borrow()
        .as_ref()
        .expect("microtask handle is retained")
        .cancel();
    drop(canceled_slot);
    root.dispose().expect("root disposal should succeed");
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
    let root = runtime.run();
    let debounce_slot = Rc::new(RefCell::new(None));
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();
        let values_for_callback = values.clone();
        let debounce = debounce_owned(&token, Duration::from_millis(10), move |value: i32| {
            values_for_callback.borrow_mut().push(value);
            Ok(())
        });
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
    root.dispose().expect("root disposal should succeed");
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

    runtime.child(|scope| {
        let owner = silex_dom::view::ScopedViewOwner::new(
            scope,
            ErrorReporter::new(move |error| errors_for_reporter.borrow_mut().push(error)),
        );
        let token = owner.token();
        let mut debounce = debounce_owned(&token, Duration::from_millis(0), |_| Ok(()));
        spy.fail_next_timeout();
        debounce(1_i32);
    });

    assert!(matches!(
        errors.borrow().as_slice(),
        [SilexError::Javascript(message)] if message.contains("forced timeout creation failure")
    ));
}

#[wasm_bindgen_test(async)]
async fn timer_callback_can_reenter_root_dispose_without_late_registration() {
    let spy = Spy::new();
    let calls = Rc::new(Cell::new(0));
    let dispose_slot = Rc::new(RefCell::new(None::<RootHandle>));
    let mut runtime = Runtime::new();

    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();

        let calls_for_callback = calls.clone();
        let dispose_for_callback = dispose_slot.clone();
        set_timeout_owned(
            &token,
            move || {
                calls_for_callback.set(calls_for_callback.get() + 1);
                if let Some(root) = dispose_for_callback.borrow_mut().take() {
                    root.dispose()
                        .expect("reentrant root disposal should succeed");
                }
                Ok(())
            },
            Duration::from_millis(0),
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
    let dispose_slot = Rc::new(RefCell::new(None::<RootHandle>));
    let values = Rc::new(RefCell::new(Vec::<i32>::new()));
    let mut runtime = Runtime::new();

    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, test_handler());

        spy.fail_next_timeout();
        let failed_calls_for_callback = failed_calls.clone();
        let failed_probe = DropProbe::new(drops.clone());
        assert!(
            set_timeout_owned(
                &owner.token(),
                move || {
                    failed_calls_for_callback.set(failed_calls_for_callback.get() + 1);
                    let _ = &failed_probe;
                    Ok(())
                },
                Duration::from_millis(0),
            )
            .is_err()
        );
        assert_eq!(failed_calls.get(), 0);
        assert_eq!(drops.get(), 1);

        let canceled_calls_for_callback = canceled_calls.clone();
        let canceled_probe = DropProbe::new(drops.clone());
        let canceled = set_timeout_owned(
            &owner.token(),
            move || {
                canceled_calls_for_callback.set(canceled_calls_for_callback.get() + 1);
                let _ = &canceled_probe;
                Ok(())
            },
            Duration::from_millis(0),
        )
        .expect("cancelable timeout should register");
        canceled.cancel();
        canceled.cancel();
        spy.fire_timeout(0);
        assert_eq!(canceled_calls.get(), 0);

        let values_for_debounce = values.clone();
        let mut debounce = debounce_owned(&owner.token(), Duration::from_millis(0), move |value| {
            values_for_debounce.borrow_mut().push(value);
            Ok(())
        });
        debounce(1);
        debounce(2);

        let reentrant_calls_for_callback = reentrant_calls.clone();
        let dispose_for_callback = dispose_slot.clone();
        set_timeout_owned(
            &owner.token(),
            move || {
                reentrant_calls_for_callback.set(reentrant_calls_for_callback.get() + 1);
                if let Some(root) = dispose_for_callback.borrow_mut().take() {
                    root.dispose()
                        .expect("reentrant root disposal should succeed");
                }
                Ok(())
            },
            Duration::from_millis(0),
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
