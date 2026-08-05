#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{Runtime, RxGet};
use silex_dom::view::{ScopedViewOwner, View};
use silex_persist::{PersistMode, Persistent, SyncStrategy};
use silex_router::{RouterContext, RouterContextProps};
use std::time::Duration;
use wasm_bindgen_test::*;
use web_sys::{StorageEvent, window};

wasm_bindgen_test_configure!(run_in_browser);

const STORAGE_KEY: &str = "silex-persist-runtime-refactor";
const DEBOUNCE_KEY: &str = "silex-persist-runtime-refactor-debounce";

fn local_storage() -> web_sys::Storage {
    window()
        .expect("window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage available")
}

#[wasm_bindgen_test]
fn local_storage_event_updates_bindings_and_scope_cleanup() {
    let window = window().expect("window");
    let storage = local_storage();
    storage.remove_item(STORAGE_KEY).expect("clear key");

    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let first = Persistent::builder(scope, STORAGE_KEY)
            .local()
            .string()
            .default("initial".to_string())
            .build();
        let second = Persistent::builder(scope, STORAGE_KEY)
            .local()
            .string()
            .default("initial".to_string())
            .build();
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

        assert_eq!(first.get_untracked(), "external");
        assert_eq!(second.get_untracked(), "external");
    });
    root.dispose().expect("dispose root");

    storage.remove_item(STORAGE_KEY).expect("cleanup key");
}

#[wasm_bindgen_test]
fn query_binding_uses_target_scope_and_updates_only_its_key() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let (path, set_path) = scope.signal("/settings".to_string());
        let (search, set_search) = scope.signal("?page=2&other=keep".to_string());
        let context = RouterContext::new(
            scope,
            RouterContextProps {
                base_path: "/".to_string(),
                path,
                search,
                set_path,
                set_search,
            },
        );
        let page = Persistent::builder(scope, "page")
            .query(&context)
            .parse::<u32>()
            .default(1)
            .build();
        set_search.set("?page=3&other=keep".to_string());
        assert_eq!(page.get_untracked(), 3);
        assert_eq!(search.get_untracked(), "?page=3&other=keep");
    });
    root.dispose().expect("dispose root");
}

#[wasm_bindgen_test]
fn persistent_view_updates_and_stops_with_root() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");

    root.with_scope(|scope| {
        let binding = Persistent::builder(scope, "view")
            .local()
            .string()
            .default("one".to_string())
            .build();
        let owner = ScopedViewOwner::new(scope);
        binding.mount(&owner, parent.as_ref(), Vec::new());
        assert_eq!(parent.text_content(), Some("one".to_string()));
        binding.set("two".to_string());
        assert_eq!(parent.text_content(), Some("two".to_string()));
    });

    root.dispose().expect("dispose root");
    assert_eq!(parent.text_content(), Some("two".to_string()));
}

#[wasm_bindgen_test(async)]
async fn debounce_writes_only_latest_value() {
    let storage = local_storage();
    storage.remove_item(DEBOUNCE_KEY).expect("clear key");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let binding = Persistent::builder(scope, DEBOUNCE_KEY)
            .local()
            .string()
            .sync(SyncStrategy::Debounce(Duration::from_millis(5)))
            .default(String::new())
            .build();
        binding.set("first".to_string());
        binding.set("latest".to_string());
    });
    TimeoutFuture::new(25).await;
    assert_eq!(
        storage.get_item(DEBOUNCE_KEY).expect("read key"),
        Some("latest".to_string())
    );
    root.dispose().expect("dispose root");
    storage.remove_item(DEBOUNCE_KEY).expect("cleanup key");
}

#[wasm_bindgen_test(async)]
async fn debounce_late_callback_is_gated_after_root_dispose() {
    let storage = local_storage();
    storage.remove_item(DEBOUNCE_KEY).expect("clear key");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let binding = Persistent::builder(scope, DEBOUNCE_KEY)
            .local()
            .string()
            .mode(PersistMode::Immediate)
            .sync(SyncStrategy::Debounce(Duration::from_millis(20)))
            .default(String::new())
            .build();
        binding.set("late".to_string());
    });
    root.dispose().expect("dispose root");
    TimeoutFuture::new(35).await;
    assert_eq!(
        storage.get_item(DEBOUNCE_KEY).expect("read key"),
        Some("".to_string())
    );
    storage.remove_item(DEBOUNCE_KEY).expect("cleanup key");
}
