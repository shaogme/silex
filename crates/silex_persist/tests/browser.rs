#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    EffectPhase, ErrorHandlerToken, ErrorReporter, OwnerAccess, OwnerHandle, Runtime, SilexContext,
    SilexResult,
};
use silex_dom::view::{
    AnyView, IndexedListView, MountContext, MountInstance, MountOwnerToken, View,
};
use silex_persist::{
    PersistExternalSync, PersistWriteMode, PersistenceState, Persistent, WriteDefault,
};
use silex_router::{RouterContext, RouterContextProps};
use std::{
    cell::{Cell, RefCell},
    panic::AssertUnwindSafe,
    rc::Rc,
    time::Duration,
};
use wasm_bindgen::{JsValue, closure::Closure, prelude::wasm_bindgen};
use wasm_bindgen_test::*;
use web_sys::{Node, StorageEvent, window};

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
    owner
        .error_handler(|_| {})
        .expect("test error handler should be registered")
}

fn test_owner<'scope>(
    owner: OwnerAccess<'scope>,
) -> (MountOwnerToken<'scope>, ErrorHandlerToken<'scope>) {
    let error_handler = test_handler(owner);
    (MountOwnerToken::new(owner), error_handler)
}

fn mount_view<'scope, V: View<'scope>>(
    view: &V,
    owner: &MountOwnerToken<'scope>,
    parent: &Node,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    let context = MountContext::for_parent(parent.clone(), owner.clone(), error_handler);
    let instance = view.mount(&context)?;
    context.transaction().commit()?;
    Ok(instance)
}

const STORAGE_KEY: &str = "silex-persist-runtime-refactor";
const LOCAL_ROUNDTRIP_KEY: &str = "silex-persist-runtime-refactor-local-roundtrip";
const SESSION_EVENT_KEY: &str = "silex-persist-runtime-refactor-session-event";
const DEBOUNCE_KEY: &str = "silex-persist-runtime-refactor-debounce";
const DEBOUNCE_REMOVE_KEY: &str = "silex-persist-runtime-refactor-debounce-remove";
const LISTENER_CLEANUP_KEY: &str = "silex-persist-runtime-refactor-listener-cleanup";
const LISTENER_REENTRY_KEY: &str = "silex-persist-runtime-refactor-listener-reentry";
const QUERY_HISTORY_KEY: &str = "page";

#[wasm_bindgen(inline_js = r#"
export function installStorageListenerSpy() {
    const spy = {
        adds: 0,
        removes: 0,
        reentry: null,
        originalAdd: window.addEventListener,
        originalRemove: window.removeEventListener,
    };
    window.addEventListener = function(name, callback, options) {
        if (name === "storage") {
            spy.adds += 1;
        }
        return spy.originalAdd.call(this, name, callback, options);
    };
    window.removeEventListener = function(name, callback, options) {
        if (name === "storage") {
            spy.removes += 1;
            const reentry = spy.reentry;
            spy.reentry = null;
            if (reentry !== null) {
                reentry();
            }
        }
        return spy.originalRemove.call(this, name, callback, options);
    };
    return spy;
}

export function storageListenerSpyCount(spy, name) {
    return name === "add" ? spy.adds : spy.removes;
}

export function setStorageListenerSpyReentry(spy, callback) {
    spy.reentry = callback;
}

export function restoreStorageListenerSpy(spy) {
    window.addEventListener = spy.originalAdd;
    window.removeEventListener = spy.originalRemove;
}

export function installQueryHistorySpy() {
    const history = window.history;
    const spy = {
        pushes: 0,
        replaces: 0,
        urls: [],
        originalPush: history.pushState,
        originalReplace: history.replaceState,
    };
    history.pushState = function(state, title, url) {
        spy.pushes += 1;
        spy.urls.push(String(url));
        return spy.originalPush.call(this, state, title, url);
    };
    history.replaceState = function(state, title, url) {
        spy.replaces += 1;
        spy.urls.push(String(url));
        return spy.originalReplace.call(this, state, title, url);
    };
    return spy;
}

export function queryHistorySpyCount(spy, name) {
    return name === "push" ? spy.pushes : spy.replaces;
}

export function queryHistorySpyLastUrl(spy) {
    return spy.urls.length === 0 ? null : spy.urls[spy.urls.length - 1];
}

export function restoreQueryHistorySpy(spy) {
    window.history.pushState = spy.originalPush;
    window.history.replaceState = spy.originalReplace;
}

export function installTimeoutController() {
    const controller = {
        callbacks: [],
        clears: [],
        invokes: 0,
        failNext: false,
        originalSet: window.setTimeout,
        originalClear: window.clearTimeout,
    };
    window.setTimeout = function(callback, ...args) {
        if (controller.failNext) {
            controller.failNext = false;
            throw new Error("forced timeout creation failure");
        }
        let id;
        const entry = { id: undefined, callback: undefined, cancelled: false, fired: false };
        const wrapped = (...callbackArgs) => {
            if (entry.cancelled || entry.fired) {
                return undefined;
            }
            entry.fired = true;
            controller.invokes += 1;
            return callback(...callbackArgs);
        };
        id = controller.originalSet.call(this, wrapped, ...args);
        entry.id = id;
        entry.callback = wrapped;
        controller.callbacks.push(entry);
        return id;
    };
    window.clearTimeout = function(id) {
        controller.clears.push(id);
        const entry = controller.callbacks.find((entry) => entry.id === id);
        if (entry !== undefined) {
            entry.cancelled = true;
        }
        return controller.originalClear.call(this, id);
    };
    return controller;
}

export function failNextTimeout(controller) {
    controller.failNext = true;
}

export function fireTimeout(controller, id) {
    const entry = controller.callbacks.find((entry) => entry.id === id);
    if (entry === undefined || entry.cancelled || entry.fired) {
        return false;
    }
    entry.callback();
    return true;
}

export function timeoutPendingIds(controller) {
    return controller.callbacks
        .filter((entry) => !entry.cancelled && !entry.fired)
        .map((entry) => entry.id);
}

export function timeoutClearCount(controller) {
    return controller.clears.length;
}

export function timeoutInvokeCount(controller) {
    return controller.invokes;
}

export function restoreTimeoutController(controller) {
    window.setTimeout = controller.originalSet;
    window.clearTimeout = controller.originalClear;
}
"#)]
unsafe extern "C" {
    #[wasm_bindgen(js_name = installStorageListenerSpy)]
    fn install_storage_listener_spy() -> JsValue;

    #[wasm_bindgen(js_name = storageListenerSpyCount)]
    fn storage_listener_spy_count(spy: &JsValue, name: &str) -> u32;

    #[wasm_bindgen(js_name = setStorageListenerSpyReentry)]
    fn set_storage_listener_spy_reentry(spy: &JsValue, callback: &JsValue);

    #[wasm_bindgen(js_name = restoreStorageListenerSpy)]
    fn restore_storage_listener_spy(spy: &JsValue);

    #[wasm_bindgen(js_name = installQueryHistorySpy)]
    fn install_query_history_spy() -> JsValue;

    #[wasm_bindgen(js_name = queryHistorySpyCount)]
    fn query_history_spy_count(spy: &JsValue, name: &str) -> u32;

    #[wasm_bindgen(js_name = queryHistorySpyLastUrl)]
    fn query_history_spy_last_url(spy: &JsValue) -> Option<String>;

    #[wasm_bindgen(js_name = restoreQueryHistorySpy)]
    fn restore_query_history_spy(spy: &JsValue);

    #[wasm_bindgen(js_name = installTimeoutController)]
    fn install_timeout_controller() -> JsValue;

    #[wasm_bindgen(js_name = failNextTimeout)]
    fn fail_next_timeout(controller: &JsValue);

    #[wasm_bindgen(js_name = fireTimeout)]
    fn fire_timeout(controller: &JsValue, id: i32) -> bool;

    #[wasm_bindgen(js_name = timeoutPendingIds)]
    fn timeout_pending_ids(controller: &JsValue) -> Vec<i32>;

    #[wasm_bindgen(js_name = timeoutClearCount)]
    fn timeout_clear_count(controller: &JsValue) -> u32;

    #[wasm_bindgen(js_name = timeoutInvokeCount)]
    fn timeout_invoke_count(controller: &JsValue) -> u32;

    #[wasm_bindgen(js_name = restoreTimeoutController)]
    fn restore_timeout_controller(controller: &JsValue);
}

struct StorageListenerSpy {
    value: JsValue,
}

impl StorageListenerSpy {
    fn new() -> Self {
        Self {
            value: install_storage_listener_spy(),
        }
    }

    fn count(&self, name: &str) -> u32 {
        storage_listener_spy_count(&self.value, name)
    }
}

impl Drop for StorageListenerSpy {
    fn drop(&mut self) {
        restore_storage_listener_spy(&self.value);
    }
}

struct QueryHistorySpy {
    value: JsValue,
}

impl QueryHistorySpy {
    fn new() -> Self {
        Self {
            value: install_query_history_spy(),
        }
    }

    fn count(&self, name: &str) -> u32 {
        query_history_spy_count(&self.value, name)
    }

    fn last_url(&self) -> Option<String> {
        query_history_spy_last_url(&self.value)
    }
}

impl Drop for QueryHistorySpy {
    fn drop(&mut self) {
        restore_query_history_spy(&self.value);
    }
}

struct TimeoutController {
    value: JsValue,
}

impl TimeoutController {
    fn new() -> Self {
        Self {
            value: install_timeout_controller(),
        }
    }

    fn fail_next(&self) {
        fail_next_timeout(&self.value);
    }

    fn fire(&self, id: i32) -> bool {
        fire_timeout(&self.value, id)
    }

    fn pending_ids(&self) -> Vec<i32> {
        timeout_pending_ids(&self.value)
    }

    fn clear_count(&self) -> u32 {
        timeout_clear_count(&self.value)
    }

    fn invoke_count(&self) -> u32 {
        timeout_invoke_count(&self.value)
    }
}

impl Drop for TimeoutController {
    fn drop(&mut self) {
        restore_timeout_controller(&self.value);
    }
}

fn local_storage() -> web_sys::Storage {
    window()
        .expect("window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage available")
}

fn session_storage() -> web_sys::Storage {
    window()
        .expect("window")
        .session_storage()
        .expect("sessionStorage access")
        .expect("sessionStorage available")
}

fn set_url(path: &str) {
    window()
        .expect("window is available")
        .history()
        .expect("history is available")
        .replace_state_with_url(&JsValue::NULL, "", Some(path))
        .expect("test URL can be replaced");
}

struct CapturedPersistent<'scope> {
    binding: Persistent<'scope, String>,
    node: Rc<RefCell<Option<Node>>>,
}

impl<'scope> View<'scope> for CapturedPersistent<'scope> {
    fn mount(
        &self,
        context: &MountContext<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        let instance = self.binding.mount(context)?;
        *self.node.borrow_mut() = instance.first_node().cloned();
        Ok(instance)
    }
}

#[wasm_bindgen_test]
async fn storage_listener_is_physically_removed_after_last_binding_cleanup() {
    let spy = StorageListenerSpy::new();
    let storage = local_storage();
    storage
        .remove_item(LISTENER_CLEANUP_KEY)
        .expect("clear key");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let _binding = Persistent::builder(scope, LISTENER_CLEANUP_KEY, test_handler(scope))
            .local()
            .string()
            .default("initial".to_string())
            .build()
            .expect("persistent binding should build");
        assert_eq!(spy.count("add"), 1);
        assert_eq!(spy.count("remove"), 0);
    });

    root.close().expect("root cleanup should succeed");
    TimeoutFuture::new(0).await;
    assert_eq!(spy.count("remove"), 1);
    storage
        .remove_item(LISTENER_CLEANUP_KEY)
        .expect("cleanup key");
}

#[wasm_bindgen_test]
async fn storage_listener_reentrant_cleanup_does_not_leave_a_listener() {
    let spy = StorageListenerSpy::new();
    let storage = local_storage();
    storage
        .remove_item(LISTENER_REENTRY_KEY)
        .expect("clear key");

    let mut reentrant_runtime = Runtime::new();
    let reentrant_root = reentrant_runtime.owner().expect("owner should be created");
    let reentrant_root = Rc::new(RefCell::new(Some(reentrant_root)));
    let root_for_reentry = reentrant_root.clone();
    let root_for_reentry = AssertUnwindSafe(root_for_reentry);
    let reentry: Closure<dyn FnMut()> = Closure::wrap(Box::new(move || {
        let root_for_reentry = &root_for_reentry;
        let Some(root) = root_for_reentry.0.borrow_mut().take() else {
            return;
        };
        root.with_access(|scope| {
            let _binding = Persistent::builder(scope, LISTENER_REENTRY_KEY, test_handler(scope))
                .local()
                .string()
                .default("reentrant".to_string())
                .build()
                .expect("persistent binding should build");
        });
        root.close().expect("reentrant root cleanup should succeed");
    }));
    set_storage_listener_spy_reentry(&spy.value, reentry.as_ref());

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let _binding = Persistent::builder(scope, LISTENER_REENTRY_KEY, test_handler(scope))
            .local()
            .string()
            .default("initial".to_string())
            .build()
            .expect("persistent binding should build");
        assert_eq!(spy.count("add"), 1);
    });

    root.close().expect("root cleanup should succeed");
    TimeoutFuture::new(0).await;
    assert_eq!(spy.count("add"), 1);
    assert_eq!(spy.count("remove"), 1);
    assert!(reentrant_root.borrow().is_none());
    drop(reentry);
    storage
        .remove_item(LISTENER_REENTRY_KEY)
        .expect("cleanup key");
}

#[wasm_bindgen_test]
fn local_storage_event_updates_bindings_and_scope_cleanup() {
    let window = window().expect("window");
    let storage = local_storage();
    storage.remove_item(STORAGE_KEY).expect("clear key");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let first = Persistent::builder(scope, STORAGE_KEY, test_handler(scope))
            .local()
            .string()
            .default("initial".to_string())
            .build().expect("persistent binding should build");
        let second = Persistent::builder(scope, STORAGE_KEY, test_handler(scope))
            .local()
            .string()
            .default("initial".to_string())
            .build().expect("persistent binding should build");
        let event = StorageEvent::new("storage").expect("storage event");
        event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(STORAGE_KEY),
            Some("initial"),
            Some("external"),
            Some("https://example.test/"),
            Some(&storage),
        );
        window
            .dispatch_event(event.as_ref())
            .expect("dispatch storage event");

        assert_eq!(first.get_untracked().expect("reactive value should be readable"), "external");
        assert_eq!(second.get_untracked().expect("reactive value should be readable"), "external");
    });
    root.close().expect("dispose root");

    storage.remove_item(STORAGE_KEY).expect("cleanup key");
}

#[wasm_bindgen_test]
fn local_storage_binding_round_trip_uses_explicit_scope() {
    let storage = local_storage();
    storage
        .set_item(LOCAL_ROUNDTRIP_KEY, "saved")
        .expect("seed localStorage key");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let binding = Persistent::builder(scope, LOCAL_ROUNDTRIP_KEY, test_handler(scope))
            .local()
            .string()
            .write_default(WriteDefault::Never)
            .default("fallback".to_string())
            .build()
            .expect("persistent binding should build");

        assert_eq!(
            binding
                .get_untracked()
                .expect("reactive value should be readable"),
            "saved"
        );
        assert_eq!(
            binding
                .state()
                .get_untracked()
                .expect("reactive value should be readable"),
            PersistenceState::Ready("saved".to_string())
        );

        binding
            .set("updated".to_string())
            .expect("reactive update should succeed");
        assert_eq!(
            storage.get_item(LOCAL_ROUNDTRIP_KEY).expect("read key"),
            Some("updated".to_string())
        );

        binding.remove().expect("remove localStorage key");
        assert_eq!(
            storage
                .get_item(LOCAL_ROUNDTRIP_KEY)
                .expect("read removed key"),
            None
        );
        assert_eq!(
            binding
                .state()
                .get_untracked()
                .expect("reactive value should be readable"),
            PersistenceState::Ready(String::new())
        );
    });
    root.close().expect("dispose root");

    storage
        .remove_item(LOCAL_ROUNDTRIP_KEY)
        .expect("cleanup localStorage key");
}

#[wasm_bindgen_test]
fn session_storage_binding_reads_writes_and_reacts_to_storage_events() {
    let window = window().expect("window");
    let storage = session_storage();
    storage
        .set_item(SESSION_EVENT_KEY, "saved")
        .expect("seed sessionStorage key");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let first = Persistent::builder(scope, SESSION_EVENT_KEY, test_handler(scope))
            .session()
            .string()
            .write_default(WriteDefault::Never)
            .default("fallback".to_string())
            .build().expect("persistent binding should build");
        let second = Persistent::builder(scope, SESSION_EVENT_KEY, test_handler(scope))
            .session()
            .string()
            .write_default(WriteDefault::Never)
            .default("fallback".to_string())
            .build().expect("persistent binding should build");

        assert_eq!(first.get_untracked().expect("reactive value should be readable"), "saved");
        assert_eq!(second.get_untracked().expect("reactive value should be readable"), "saved");

        first.set("updated".to_string()).expect("reactive update should succeed");
        assert_eq!(
            storage
                .get_item(SESSION_EVENT_KEY)
                .expect("read sessionStorage key"),
            Some("updated".to_string())
        );

        let set_event = StorageEvent::new("storage").expect("storage event");
        set_event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(SESSION_EVENT_KEY),
            Some("updated"),
            Some("external"),
            Some("https://example.test/"),
            Some(&storage),
        );
        window
            .dispatch_event(set_event.as_ref())
            .expect("dispatch sessionStorage set event");
        assert_eq!(first.get_untracked().expect("reactive value should be readable"), "external");
        assert_eq!(second.get_untracked().expect("reactive value should be readable"), "external");

        first.remove().expect("remove sessionStorage key");
        assert_eq!(
            storage
                .get_item(SESSION_EVENT_KEY)
                .expect("read removed sessionStorage key"),
            None
        );

        let remove_event = StorageEvent::new("storage").expect("storage event");
        remove_event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(SESSION_EVENT_KEY),
            Some("external"),
            None,
            Some("https://example.test/"),
            Some(&storage),
        );
        window
            .dispatch_event(remove_event.as_ref())
            .expect("dispatch sessionStorage remove event");
        assert_eq!(first.get_untracked().expect("reactive value should be readable"), "fallback");
        assert_eq!(second.get_untracked().expect("reactive value should be readable"), "fallback");
    });
    root.close().expect("dispose root");

    storage
        .remove_item(SESSION_EVENT_KEY)
        .expect("cleanup sessionStorage key");
}

#[wasm_bindgen_test]
fn query_binding_uses_target_scope_and_updates_only_its_key() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let (path, set_path) = scope
            .signal("/settings".to_string())
            .expect("path signal should be created");
        let (search, set_search) = scope
            .signal("?page=2&other=keep".to_string())
            .expect("search signal should be created");
        let context_handler = test_handler(scope);
        let ctx = RouterContext::new(
            SilexContext::new(scope, context_handler.view()),
            RouterContextProps {
                base_path: "/".to_string(),
                path,
                search,
                set_path,
                set_search,
            },
        )
        .expect("router ctx should be created");
        let page = Persistent::builder(scope, "page", test_handler(scope))
            .query(ctx)
            .parse::<u32>()
            .default(1)
            .build()
            .expect("persistent binding should build");
        set_search
            .set("?page=3&other=keep".to_string())
            .expect("reactive update should succeed");
        assert_eq!(
            page.get_untracked()
                .expect("reactive value should be readable"),
            3
        );
        assert_eq!(
            search
                .get_untracked()
                .expect("reactive value should be readable"),
            "?page=3&other=keep"
        );
    });
    root.close().expect("dispose root");
}

#[wasm_bindgen_test]
fn query_backend_writes_one_push_and_one_url_update_per_change() {
    set_url("/persist-query?keep=yes");
    let spy = QueryHistorySpy::new();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let (path, set_path) = scope
            .signal("/persist-query".to_string())
            .expect("path signal should be created");
        let (search, set_search) = scope
            .signal("?keep=yes".to_string())
            .expect("search signal should be created");
        let context_handler = test_handler(scope);
        let ctx = RouterContext::new(
            SilexContext::new(scope, context_handler.view()),
            RouterContextProps {
                base_path: "/".to_string(),
                path,
                search,
                set_path,
                set_search,
            },
        )
        .expect("router ctx should be created");
        let search_updates = Rc::new(Cell::new(0));
        let search_updates_for_effect = search_updates.clone();
        scope
            .effect(
                EffectPhase::Normal,
                move || -> SilexResult<()> {
                    search.get()?;
                    search_updates_for_effect.set(search_updates_for_effect.get() + 1);
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("query update effect can be registered");
        let initial_search_updates = search_updates.get();

        let binding = Persistent::builder(scope, QUERY_HISTORY_KEY, test_handler(scope))
            .query(ctx)
            .parse::<u32>()
            .write_default(WriteDefault::Never)
            .default(1)
            .build()
            .expect("persistent binding should build");
        assert_eq!(spy.count("push"), 0);
        assert_eq!(spy.count("replace"), 0);

        binding.set(2).expect("reactive update should succeed");
        assert_eq!(spy.count("push"), 1);
        assert_eq!(spy.count("replace"), 0);
        assert_eq!(
            spy.last_url().as_deref(),
            Some("/persist-query?keep=yes&page=2")
        );
        assert_eq!(search_updates.get(), initial_search_updates + 1);

        binding.set(2).expect("reactive update should succeed");
        assert_eq!(spy.count("push"), 1);
        assert_eq!(search_updates.get(), initial_search_updates + 1);

        binding.remove().expect("query key can be removed");
        assert_eq!(spy.count("push"), 2);
        assert_eq!(spy.count("replace"), 0);
        assert_eq!(spy.last_url().as_deref(), Some("/persist-query?keep=yes"));
        assert_eq!(search_updates.get(), initial_search_updates + 2);

        binding.remove().expect("missing query key can be removed");
        assert_eq!(spy.count("push"), 2);
        assert_eq!(search_updates.get(), initial_search_updates + 2);
    });

    root.close().expect("root cleanup should succeed");
    drop(spy);
    set_url("/");
}

#[wasm_bindgen_test]
fn persistent_view_updates_and_stops_with_root() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");

    root.with_access(|scope| {
        let binding = Persistent::builder(scope, "view", test_handler(scope))
            .local()
            .string()
            .default("one".to_string())
            .build()
            .expect("persistent binding should build");
        let (owner, error_handler) = test_owner(scope);
        let _ = mount_view(&binding, &owner, parent.as_ref(), error_handler.view())
            .expect("persistent view should mount");
        assert_eq!(parent.text_content(), Some("one".to_string()));
        binding
            .set("two".to_string())
            .expect("reactive update should succeed");
        assert_eq!(parent.text_content(), Some("two".to_string()));
    });

    root.close().expect("dispose root");
    assert_eq!(parent.text_content(), Some(String::new()));
}

#[wasm_bindgen_test]
fn persistent_view_stops_after_lexical_owner_dispose() {
    const KEY: &str = "silex-persist-runtime-refactor-lexical-owner";
    let storage = local_storage();
    storage.remove_item(KEY).expect("clear key");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");

    root.with_access(|scope| {
        let _root_binding = Persistent::builder(scope, KEY, test_handler(scope))
            .local()
            .string()
            .default("one".to_string())
            .build().expect("persistent binding should build");
        let captured_node = Rc::new(RefCell::new(None::<Node>));
        let captured_node_for_child = captured_node.clone();
        let _ = scope.with_transient(|child| {
            let binding = Persistent::builder(child, KEY, test_handler(child))
                .local()
                .string()
                .default("one".to_string())
                .build().expect("persistent binding should build");
            let (owner, error_handler) = test_owner(child);
            let view = CapturedPersistent {
                binding,
                node: captured_node_for_child,
            };
            let _ = mount_view(
                &view,
                &owner,
                parent.as_ref(),
                error_handler.view(),
            )
            .expect("captured persistent view should mount");
            assert_eq!(parent.text_content(), Some("one".to_string()));
            binding.set("two".to_string()).expect("reactive update should succeed");
            assert_eq!(parent.text_content(), Some("two".to_string()));
        });

        let event = StorageEvent::new("storage").expect("storage event");
        event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(KEY),
            Some("two"),
            Some("stale"),
            Some("https://example.test/"),
            Some(&storage),
        );
        window()
            .expect("window")
            .dispatch_event(event.as_ref())
            .expect("dispatch storage event");
        assert_eq!(parent.text_content(), Some(String::new()));
        assert_eq!(
            captured_node
                .borrow()
                .as_ref()
                .and_then(Node::node_value),
            Some("two".to_string())
        );
    });

    root.close().expect("root cleanup should succeed");
    storage.remove_item(KEY).expect("cleanup key");
}

#[wasm_bindgen_test]
fn persistent_view_stops_after_row_owner_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");

    root.with_access(|scope| {
        let binding = Persistent::builder(scope, "silex-persist-row-owner", test_handler(scope))
            .local()
            .string()
            .external_sync(PersistExternalSync::Disabled)
            .default("one".to_string())
            .build()
            .expect("persistent binding should build");
        let captured_node = Rc::new(RefCell::new(None::<Node>));
        let captured_node_for_view = captured_node.clone();
        let (items, set_items) = scope
            .signal(vec![0_i32])
            .expect("items signal should be created");
        let list = IndexedListView {
            each: items,
            view_fn: Rc::new(move |_, _| {
                AnyView::new(CapturedPersistent {
                    binding,
                    node: captured_node_for_view.clone(),
                })
            }),
            _marker: std::marker::PhantomData,
        };
        let (owner, error_handler) = test_owner(scope);
        let _ = mount_view(&list, &owner, parent.as_ref(), error_handler.view())
            .expect("persistent list should mount");
        assert_eq!(parent.text_content(), Some("one".to_string()));

        set_items
            .set(Vec::new())
            .expect("reactive update should succeed");
        assert_eq!(parent.text_content(), Some(String::new()));
        binding
            .set("stale".to_string())
            .expect("reactive update should succeed");
        assert_eq!(
            captured_node.borrow().as_ref().and_then(Node::node_value),
            Some("one".to_string())
        );
    });

    root.close().expect("root cleanup should succeed");
}

#[wasm_bindgen_test(async)]
async fn debounce_writes_only_latest_value() {
    let storage = local_storage();
    storage.remove_item(DEBOUNCE_KEY).expect("clear key");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let binding = Persistent::builder(scope, DEBOUNCE_KEY, test_handler(scope))
            .local()
            .string()
            .write_mode(PersistWriteMode::Debounced(Duration::from_millis(5)))
            .default(String::new())
            .build()
            .expect("persistent binding should build");
        binding
            .set("first".to_string())
            .expect("reactive update should succeed");
        binding
            .set("latest".to_string())
            .expect("reactive update should succeed");
    });
    TimeoutFuture::new(25).await;
    assert_eq!(
        storage.get_item(DEBOUNCE_KEY).expect("read key"),
        Some("latest".to_string())
    );
    root.close().expect("dispose root");
    storage.remove_item(DEBOUNCE_KEY).expect("cleanup key");
}

#[wasm_bindgen_test(async)]
async fn debounce_late_callback_is_gated_after_root_dispose() {
    let storage = local_storage();
    storage.remove_item(DEBOUNCE_KEY).expect("clear key");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let binding = Persistent::builder(scope, DEBOUNCE_KEY, test_handler(scope))
            .local()
            .string()
            .write_mode(PersistWriteMode::Debounced(Duration::from_millis(20)))
            .default(String::new())
            .build()
            .expect("persistent binding should build");
        binding
            .set("late".to_string())
            .expect("reactive update should succeed");
    });
    root.close().expect("dispose root");
    TimeoutFuture::new(35).await;
    assert_eq!(
        storage.get_item(DEBOUNCE_KEY).expect("read key"),
        Some("".to_string())
    );
    storage.remove_item(DEBOUNCE_KEY).expect("cleanup key");
}

#[wasm_bindgen_test(async)]
async fn debounce_external_remove_does_not_skip_next_write() {
    let window = window().expect("window");
    let storage = local_storage();
    storage
        .set_item(DEBOUNCE_REMOVE_KEY, "5")
        .expect("seed key");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");
    root.with_access(|scope| {
        let binding = Persistent::builder(scope, DEBOUNCE_REMOVE_KEY, test_handler(scope))
            .local()
            .string()
            .write_mode(PersistWriteMode::Debounced(Duration::from_millis(5)))
            .default("5".to_string())
            .build().expect("persistent binding should build");
        storage.remove_item(DEBOUNCE_REMOVE_KEY).expect("remove key");
        let event = StorageEvent::new("storage").expect("storage event");
        event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(DEBOUNCE_REMOVE_KEY),
            Some("5"),
            None,
            Some("https://example.test/"),
            Some(&storage),
        );
        window
            .dispatch_event(event.as_ref())
            .expect("dispatch storage event");
        binding.set("6".to_string()).expect("reactive update should succeed");
    });
    TimeoutFuture::new(25).await;
    assert_eq!(
        storage.get_item(DEBOUNCE_REMOVE_KEY).expect("read key"),
        Some("6".to_string())
    );
    root.close().expect("dispose root");
    storage
        .remove_item(DEBOUNCE_REMOVE_KEY)
        .expect("cleanup key");
}

#[wasm_bindgen_test]
fn debounce_timer_failure_reentry_and_late_callbacks_are_gated() {
    const KEY: &str = "silex-persist-runtime-refactor-debounce-failure";
    let storage = local_storage();
    storage.remove_item(KEY).expect("clear key");
    let controller = TimeoutController::new();
    let dispose_slot = Rc::new(RefCell::new(None::<OwnerHandle>));
    let stale_timer_id = Cell::new(None::<i32>);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("owner should be created");

    root.with_access(|scope| {
        let errors_for_handler = errors.clone();
        let error_handler = scope
            .error_handler(move |error| errors_for_handler.borrow_mut().push(error))
            .expect("error handler should be registered");
        let binding = Persistent::builder(scope, KEY, error_handler)
            .local()
            .string()
            .write_default(WriteDefault::Never)
            .write_mode(PersistWriteMode::Debounced(
                std::time::Duration::from_millis(1),
            ))
            .default(String::new())
            .build()
            .expect("persistent binding should build");
        let binding_for_dispose = binding;
        let dispose_for_effect = dispose_slot.clone();
        scope
            .effect(
                EffectPhase::Normal,
                move || -> SilexResult<()> {
                    if binding_for_dispose.state().get()?
                        == PersistenceState::Ready("second".to_string())
                        && let Some(root) = dispose_for_effect.borrow_mut().take()
                    {
                        root.close()
                            .expect("state effect can dispose its root reentrantly");
                    }
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("debounce state effect can be registered");

        controller.fail_next();
        binding
            .set("failed".to_string())
            .expect("reactive update should succeed");
        assert!(
            matches!(
                binding
                    .state()
                    .get_untracked()
                    .expect("reactive value should be readable"),
                PersistenceState::WriteError(_)
            ),
            "errors after timer failure: {:?}",
            errors.borrow()
        );

        binding
            .set("first".to_string())
            .expect("reactive update should succeed");
        let first_timer_id = controller
            .pending_ids()
            .into_iter()
            .next()
            .expect("first timer should be pending");
        stale_timer_id.set(Some(first_timer_id));
        binding
            .set("second".to_string())
            .expect("reactive update should succeed");
        assert!(
            !controller.pending_ids().is_empty(),
            "latest timer was not scheduled; state: {:?}; errors: {:?}",
            binding
                .state()
                .get_untracked()
                .expect("reactive value should be readable"),
            errors.borrow()
        );
    });

    *dispose_slot.borrow_mut() = Some(root);
    let active_timer_id = controller
        .pending_ids()
        .into_iter()
        .next()
        .expect("latest timer should be pending");
    assert!(controller.fire(active_timer_id));
    assert_eq!(
        storage.get_item(KEY).expect("read persisted value"),
        Some("second".to_string())
    );
    if let Some(stale_timer_id) = stale_timer_id.get() {
        assert!(!controller.fire(stale_timer_id));
    }
    assert_eq!(controller.invoke_count(), 1);
    assert_eq!(
        storage.get_item(KEY).expect("read persisted value"),
        Some("second".to_string())
    );
    assert_eq!(controller.clear_count(), 1);
    assert!(dispose_slot.borrow().is_none());
    assert!(
        errors.borrow().is_empty(),
        "unexpected errors: {:?}",
        errors.borrow()
    );
    storage.remove_item(KEY).expect("cleanup key");
}
