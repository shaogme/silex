#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::js_sys::{self, Reflect};
use silex::reexports::wasm_bindgen::{JsCast, JsValue, closure::Closure};
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlInputElement, Node};
use silex_showcase::mount_showcase_into;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn set_path(path: &str) {
    web_sys::window()
        .expect("window is available")
        .history()
        .expect("history is available")
        .replace_state_with_url(&JsValue::NULL, "", Some(path))
        .expect("test path can be set");
}

fn target() -> DomElement {
    let target = document()
        .create_element("div")
        .expect("target can be created");
    document()
        .body()
        .expect("body is available")
        .append_child(&target)
        .expect("target can be appended");
    target
}

fn detach(target: &Node) {
    if let Some(parent) = target.parent_node() {
        parent.remove_child(target).expect("target can be detached");
    }
}

struct ConsoleErrorCapture {
    console: JsValue,
    previous: JsValue,
    _replacement: Closure<dyn FnMut()>,
}

impl ConsoleErrorCapture {
    fn install() -> Self {
        let global: JsValue = js_sys::global().into();
        let console = Reflect::get(&global, &JsValue::from_str("console"))
            .expect("console should be available");
        let error_key = JsValue::from_str("error");
        let previous = Reflect::get(&console, &error_key).expect("console.error should exist");
        Reflect::set(
            &global,
            &JsValue::from_str("__silex_showcase_console_error_count"),
            &JsValue::from_f64(0.0),
        )
        .expect("console error counter should be initialized");
        let replacement: Closure<dyn FnMut()> = Closure::wrap(Box::new(|| {
            let global: JsValue = js_sys::global().into();
            let key = JsValue::from_str("__silex_showcase_console_error_count");
            let current = Reflect::get(&global, &key)
                .expect("console error counter should exist")
                .as_f64()
                .expect("console error counter should be numeric");
            Reflect::set(&global, &key, &JsValue::from_f64(current + 1.0))
                .expect("console error counter should be writable");
        }));
        Reflect::set(&console, &error_key, replacement.as_ref())
            .expect("console.error should be replaceable");
        Self {
            console,
            previous,
            _replacement: replacement,
        }
    }

    fn count(&self) -> u32 {
        let global: JsValue = js_sys::global().into();
        Reflect::get(
            &global,
            &JsValue::from_str("__silex_showcase_console_error_count"),
        )
        .expect("console error counter should exist")
        .as_f64()
        .expect("console error counter should be numeric") as u32
    }
}

impl Drop for ConsoleErrorCapture {
    fn drop(&mut self) {
        let error_key = JsValue::from_str("error");
        let _ = Reflect::set(&self.console, &error_key, &self.previous);
    }
}

async fn flush_browser_tasks() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

fn stability_slider(target: &DomElement) -> HtmlInputElement {
    target
        .query_selector("input[type='range']")
        .expect("stability slider query should succeed")
        .expect("stability slider should exist")
        .dyn_into::<HtmlInputElement>()
        .expect("stability slider should be an HTML input")
}

fn adaptive_status(target: &DomElement) -> String {
    let divs = target
        .query_selector_all("div")
        .expect("status bar query should succeed");
    for index in 0..divs.length() {
        let text = divs
            .item(index)
            .expect("status bar candidate should exist")
            .text_content()
            .unwrap_or_default();
        if text.starts_with("System: ") {
            return text;
        }
    }
    panic!("adaptive status bar was not found");
}

fn button_with_text(target: &DomElement, expected: &str) -> DomElement {
    let buttons = target
        .query_selector_all("button")
        .expect("button query should succeed");
    for index in 0..buttons.length() {
        let button = buttons
            .item(index)
            .expect("button candidate should exist")
            .dyn_into::<DomElement>()
            .expect("button candidate should be an element");
        if button.text_content().unwrap_or_default().trim() == expected {
            return button;
        }
    }
    panic!("button {expected:?} was not found");
}

#[wasm_bindgen_test]
fn flow_route_mounts_with_render_only_for_rows() {
    set_path("/flow");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");

    let text = target.text_content().unwrap_or_default();
    assert!(text.contains("List Rendering with Error Handling"));
    assert!(text.contains("Index For Loop Demo"));
    assert!(
        host.is_active()
            .expect("showcase should report active state")
    );

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test]
async fn adaptive_read_formats_normalized_stability_as_percentage() {
    set_path("/advanced/adaptive");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");
    let slider = stability_slider(&target);

    for (value, expected) in [("0.50", "Stability: 50%"), ("0.51", "Stability: 51%")] {
        slider.set_value(value);
        slider
            .dispatch_event(&web_sys::Event::new("input").expect("input event can be created"))
            .expect("input event should dispatch");
        flush_browser_tasks().await;

        let text = adaptive_status(&target);
        assert!(
            text.contains(expected),
            "adaptive status should contain {expected:?}, got {text:?}"
        );
    }

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test]
fn mutation_inputs_start_with_empty_values() {
    set_path("/advanced/mutation");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");

    for placeholder in ["Username", "Password"] {
        let input = target
            .query_selector(&format!("input[placeholder='{placeholder}']"))
            .expect("mutation input query should succeed")
            .expect("mutation input should exist")
            .dyn_into::<HtmlInputElement>()
            .expect("mutation input should be an HTML input");
        assert_eq!(input.value(), "", "{placeholder} input should start empty");
    }

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test(async)]
async fn basics_reactive_greeting_updates_after_submit() {
    set_path("/basics");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");
    let input = target
        .query_selector("input")
        .expect("greeting input query should succeed")
        .expect("greeting input should exist")
        .dyn_into::<HtmlInputElement>()
        .expect("greeting input should be an HTML input");

    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Hello, Developer!")
    );
    input.set_value("Ada");
    input
        .dispatch_event(&web_sys::Event::new("input").expect("input event can be created"))
        .expect("input event should dispatch");
    flush_browser_tasks().await;
    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Hello, Developer!"),
        "draft input should not commit the greeting before submit"
    );

    button_with_text(&target, "Submit")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("submit click should dispatch");
    flush_browser_tasks().await;
    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Hello, Ada!"),
        "submitted name should update the reactive greeting"
    );

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test(async)]
async fn basics_node_ref_check_reports_mounted_input() {
    set_path("/basics");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");

    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Input ref has not been checked")
    );
    assert!(
        target
            .query_selector("input[placeholder='I will be focused...']")
            .expect("NodeRef input query should succeed")
            .is_some()
    );

    button_with_text(&target, "Check Input Ref")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("NodeRef check click should dispatch");
    flush_browser_tasks().await;

    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Input ref is available"),
        "NodeRef check should report the mounted input"
    );

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test(async)]
async fn flow_portal_toggle_button_opens_modal() {
    set_path("/flow");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");
    let visibility_toggle = button_with_text(&target, "Toggle Visibility");
    let toggle = button_with_text(&target, "Toggle Modal");

    let portal = || {
        document()
            .query_selector("body > div[data-portal-host='showcase-modal']")
            .expect("showcase modal selector should be valid")
            .expect("showcase modal host should be mounted")
    };

    let root = || {
        portal()
            .query_selector("[data-portal-visibility-root]")
            .expect("showcase modal root selector should be valid")
            .expect("showcase modal root should be mounted")
    };

    assert_eq!(
        root().get_attribute("data-state").as_deref(),
        Some("closed")
    );
    visibility_toggle
        .dispatch_event(
            &web_sys::MouseEvent::new("click").expect("visibility click can be created"),
        )
        .expect("visibility click should dispatch");
    flush_browser_tasks().await;
    assert!(
        target
            .text_content()
            .unwrap_or_default()
            .contains("Content is hidden"),
        "normal flow event should update its signal"
    );
    toggle
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("toggle click should dispatch");
    flush_browser_tasks().await;

    assert_eq!(root().get_attribute("data-state").as_deref(), Some("open"));
    assert_eq!(
        portal()
            .text_content()
            .unwrap_or_default()
            .matches("I am a Modal!")
            .count(),
        1
    );

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test(async)]
async fn net_tab_switches_do_not_report_stale_reactive_handles() {
    let console_errors = ConsoleErrorCapture::install();
    set_path("/net");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");

    button_with_text(&target, "WebSocket")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("websocket tab click should dispatch");
    flush_browser_tasks().await;

    button_with_text(&target, "HTTP Client")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("http tab click should dispatch");
    flush_browser_tasks().await;

    button_with_text(&target, "EventStream")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("event stream tab click should dispatch");
    flush_browser_tasks().await;

    button_with_text(&target, "HTTP Client")
        .dispatch_event(&web_sys::MouseEvent::new("click").expect("click can be created"))
        .expect("second http tab click should dispatch");
    flush_browser_tasks().await;

    assert_eq!(
        console_errors.count(),
        0,
        "tab switching should not report reactive errors"
    );
    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}
