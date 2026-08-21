#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::wasm_bindgen::JsCast;
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlElement, Node};
use silex_ui_example::mount_ui_into;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn target(id: &str) -> DomElement {
    let target = document()
        .create_element("div")
        .expect("target can be created");
    target.set_id(id);
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

fn clear_theme_storage() {
    let storage = web_sys::window()
        .expect("window is available")
        .local_storage()
        .expect("local storage access should succeed");
    if let Some(storage) = storage {
        storage
            .remove_item("silex-ui-dark")
            .expect("theme storage should be cleared");
    }
}

async fn flush_browser_tasks() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

fn app_text(target: &DomElement) -> String {
    target.text_content().unwrap_or_default()
}

fn body_element() -> DomElement {
    document().body().expect("body is available").into()
}

fn body_text() -> String {
    document()
        .body()
        .expect("body is available")
        .text_content()
        .unwrap_or_default()
}

fn find_button(target: &DomElement, label: &str) -> HtmlElement {
    let buttons = target
        .query_selector_all("button")
        .expect("button query should succeed");
    for index in 0..buttons.length() {
        let button = buttons
            .item(index)
            .expect("button should exist")
            .dyn_into::<HtmlElement>()
            .expect("button should be an HTML element");
        if button.inner_text() == label {
            return button;
        }
    }
    panic!("button {label:?} was not found");
}

fn find_button_containing(target: &DomElement, label: &str) -> HtmlElement {
    let buttons = target
        .query_selector_all("button")
        .expect("button query should succeed");
    for index in 0..buttons.length() {
        let button = buttons
            .item(index)
            .expect("button should exist")
            .dyn_into::<HtmlElement>()
            .expect("button should be an HTML element");
        if button.inner_text().contains(label) {
            return button;
        }
    }
    panic!("button containing {label:?} was not found");
}

fn input_by_placeholder(target: &DomElement, placeholder: &str) -> web_sys::HtmlInputElement {
    target
        .query_selector(&format!("input[placeholder='{placeholder}']"))
        .expect("input query should succeed")
        .expect("input should exist")
        .dyn_into::<web_sys::HtmlInputElement>()
        .expect("element should be an input")
}

fn dispatch_input(input: &web_sys::HtmlInputElement, event_name: &str) {
    input
        .dispatch_event(&web_sys::Event::new(event_name).expect("input event can be created"))
        .expect("input event should dispatch");
}

fn radio_state(target: &DomElement) -> String {
    let items = target
        .query_selector_all("[data-slot='radio-group-item']")
        .expect("radio item query should succeed");
    let mut states = Vec::new();
    for index in 0..items.length() {
        let item = items
            .item(index)
            .expect("radio item should exist")
            .dyn_into::<HtmlElement>()
            .expect("radio item should be an HTML element");
        let input = item
            .query_selector("input[type='radio']")
            .expect("radio input query should succeed")
            .expect("radio input should exist")
            .dyn_into::<web_sys::HtmlInputElement>()
            .expect("radio input should be an input");
        states.push(format!(
            "{}: data-state={:?}, aria-checked={:?}, disabled={}, input-disabled={}, checked={}",
            item.get_attribute("data-value").unwrap_or_default(),
            item.get_attribute("data-state"),
            item.get_attribute("aria-checked"),
            item.has_attribute("disabled"),
            input.disabled(),
            input.checked()
        ));
    }
    states.join("; ")
}

#[wasm_bindgen_test(async)]
async fn ui_showcase_mounts_and_updates_interactive_components() {
    clear_theme_storage();
    let app = target("ui-app");
    let mut host = mount_ui_into(app.clone().into()).expect("UI showcase should mount");

    assert!(host.is_active().expect("UI showcase should be active"));
    assert_eq!(host.state(), "active");
    let initial = app_text(&app);
    for expected in [
        "Pure Rust shadcn/ui Component Library",
        "Button & Badge System",
        "Form & Interactive Controls",
        "Tabs & Modal Dialog",
        "Avatars, Progress & Feedback",
        "Extended shadcn/ui Components",
    ] {
        assert!(
            initial.contains(expected),
            "missing {expected:?} in {initial:?}"
        );
    }

    let dark_mode = find_button_containing(&app, "Dark Mode");
    assert!(
        document()
            .document_element()
            .expect("document element exists")
            .class_list()
            .contains("dark")
    );
    dark_mode.click();
    flush_browser_tasks().await;
    assert!(
        find_button_containing(&app, "Light Mode")
            .inner_text()
            .contains("Light Mode")
    );
    assert!(
        !document()
            .document_element()
            .expect("document element exists")
            .class_list()
            .contains("dark")
    );

    let text_input = input_by_placeholder(&app, "Type something...");
    text_input.set_value("Ada");
    dispatch_input(&text_input, "input");
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Live Bound Value: 'Ada'"));

    find_button(&app, "Password").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Change your password and configure 2FA security."));

    let dialog_host = body_element()
        .query_selector("[data-portal-host='dialog']")
        .expect("dialog host query should succeed")
        .expect("dialog host should exist before opening");
    find_button(&app, "Open Modal Dialog").click();
    flush_browser_tasks().await;
    assert!(!dialog_host.has_attribute("hidden"));
    assert!(body_text().contains("Edit Profile"));
    find_button(&body_element(), "Cancel").click();
    flush_browser_tasks().await;
    let closed_dialog_host = body_element()
        .query_selector("[data-portal-host='dialog']")
        .expect("closed dialog host query should succeed")
        .expect("closed dialog host should remain mounted");
    assert!(dialog_host.is_same_node(Some(closed_dialog_host.as_ref())));
    assert!(closed_dialog_host.has_attribute("hidden"));

    find_button(&app, "-10%").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("35%"));
    find_button(&app, "+10%").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("45%"));

    let slider = app
        .query_selector("input[type='range']")
        .expect("slider query should succeed")
        .expect("slider input should exist")
        .dyn_into::<web_sys::HtmlInputElement>()
        .expect("slider should be an input");
    slider.set_value("80");
    dispatch_input(&slider, "input");
    dispatch_input(&slider, "change");
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("80%"));

    let toggle = find_button(&app, "B");
    assert_eq!(
        toggle.get_attribute("aria-pressed").as_deref(),
        Some("true")
    );
    toggle.click();
    flush_browser_tasks().await;
    assert_eq!(
        toggle.get_attribute("aria-pressed").as_deref(),
        Some("false")
    );

    let popover_host = body_element()
        .query_selector("[data-portal-host='popover']")
        .expect("popover host query should succeed")
        .expect("popover host should exist before opening");
    find_button(&app, "Open Popover").click();
    flush_browser_tasks().await;
    assert!(!popover_host.has_attribute("hidden"));
    assert!(body_text().contains("Dimensions"));
    find_button(&body_element(), "Close").click();
    flush_browser_tasks().await;
    let closed_popover_host = body_element()
        .query_selector("[data-portal-host='popover']")
        .expect("closed popover host query should succeed")
        .expect("closed popover host should remain mounted");
    assert!(popover_host.is_same_node(Some(closed_popover_host.as_ref())));
    assert!(closed_popover_host.has_attribute("hidden"));

    let radio_items = app
        .query_selector_all("[data-slot='radio-group-item']")
        .expect("radio item query should succeed");
    assert_eq!(
        radio_items.length(),
        2,
        "radio state: {}",
        radio_state(&app)
    );
    let option_2 = app
        .query_selector("[data-slot='radio-group-item'][data-value='option-2']")
        .expect("option-2 radio query should succeed")
        .expect("option-2 radio should exist")
        .dyn_into::<HtmlElement>()
        .expect("option-2 radio should be an HTML element");
    option_2.click();
    flush_browser_tasks().await;
    let selected = app
        .query_selector("[data-slot='radio-group-item'][data-state='checked']")
        .expect("selected radio query should succeed")
        .expect("selected radio should exist");
    assert_eq!(
        selected.get_attribute("data-value").as_deref(),
        Some("option-2"),
        "radio state after clicking option-2: {}",
        radio_state(&app)
    );

    let accordion_trigger = find_button(&app, "Is Silex 1:1 compatible?");
    let accordion_content = app
        .query_selector("[data-slot='accordion-content']")
        .expect("accordion content query should succeed")
        .expect("accordion content should exist");
    assert!(app_text(&app).contains("Every layout, utility class"));
    assert_eq!(
        accordion_content.get_attribute("data-state").as_deref(),
        Some("open")
    );
    assert_eq!(
        accordion_content.get_attribute("aria-hidden").as_deref(),
        Some("false")
    );
    accordion_trigger.click();
    flush_browser_tasks().await;
    let closed_content = app
        .query_selector("[data-slot='accordion-content']")
        .expect("closed accordion content query should succeed")
        .expect("closed accordion content should remain mounted");
    assert!(accordion_content.is_same_node(Some(closed_content.as_ref())));
    assert!(app_text(&app).contains("Every layout, utility class"));
    assert_eq!(
        closed_content.get_attribute("data-state").as_deref(),
        Some("closed")
    );
    assert_eq!(
        closed_content.get_attribute("aria-hidden").as_deref(),
        Some("true")
    );
    assert!(closed_content.has_attribute("inert"));
    accordion_trigger.click();
    flush_browser_tasks().await;
    let reopened_content = app
        .query_selector("[data-slot='accordion-content']")
        .expect("reopened accordion content query should succeed")
        .expect("reopened accordion content should remain mounted");
    assert!(accordion_content.is_same_node(Some(reopened_content.as_ref())));
    assert_eq!(
        reopened_content.get_attribute("data-state").as_deref(),
        Some("open")
    );
    assert!(!reopened_content.has_attribute("inert"));
    assert!(app_text(&app).contains("Every layout, utility class"));

    host.unmount().expect("UI showcase should unmount");
    assert!(
        !host
            .is_active()
            .expect("unmounted UI showcase should be inactive")
    );
    assert_eq!(host.state(), "ready");
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated unmount should remain idempotent");
    clear_theme_storage();
    detach(&app.into());
}

#[wasm_bindgen_test]
fn ui_owner_unmounts_after_target_is_removed() {
    clear_theme_storage();
    let app = target("ui-detached");
    let mut host = mount_ui_into(app.clone().into()).expect("UI showcase should mount");
    assert!(host.is_active().expect("UI showcase should be active"));

    detach(&app.clone().into());
    host.unmount()
        .expect("unmount should work after external target removal");
    assert_eq!(app.child_nodes().length(), 0);
    clear_theme_storage();
}
