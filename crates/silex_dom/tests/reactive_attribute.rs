#![cfg(target_arch = "wasm32")]

use std::borrow::Cow;

use silex_core::{ErrorReporter, Runtime, Scope};
use silex_dom::attribute::AttributeBuilder;
use silex_dom::element::Element;
use silex_dom::view::{ScopedMountOwner, View};
use wasm_bindgen_test::*;
use web_sys::Element as DomElement;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

fn host() -> DomElement {
    web_sys::window()
        .expect("window is available in browser tests")
        .document()
        .expect("document is available in browser tests")
        .create_element("div")
        .expect("test host can be created")
}

fn mounted(host: &DomElement) -> DomElement {
    host.first_element_child()
        .expect("reactive element should be mounted")
}

fn style_text(element: &DomElement) -> String {
    element.get_attribute("style").unwrap_or_default()
}

#[wasm_bindgen_test]
fn reactive_static_str_attribute_updates() {
    let host = host();
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, write) = scope.signal("initial").expect("signal should initialize");
            let error_handler = test_handler(scope);
            let owner = ScopedMountOwner::new(scope);
            let view = Element::new("button").attr("data-state", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler)
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("initial")
            );

            write.set("updated").expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("updated")
            );
        })
        .expect("child scope should initialize");
}

#[wasm_bindgen_test]
fn reactive_borrowed_str_attribute_updates() {
    let host = host();
    let initial = String::from("initial");
    let updated = String::from("updated");
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, write) = scope
                .signal(initial.as_str())
                .expect("signal should initialize");
            let error_handler = test_handler(scope);
            let owner = ScopedMountOwner::new(scope);
            let view = Element::new("div").attr("data-value", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler)
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-value").as_deref(),
                Some("initial")
            );

            write
                .set(updated.as_str())
                .expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-value").as_deref(),
                Some("updated")
            );
        })
        .expect("child scope should initialize");
}

#[wasm_bindgen_test]
fn reactive_cow_attribute_updates() {
    let host = host();
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, write) = scope
                .signal(Cow::Borrowed("initial"))
                .expect("signal should initialize");
            let error_handler = test_handler(scope);
            let owner = ScopedMountOwner::new(scope);
            let view = Element::new("span").attr("data-state", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler)
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("initial")
            );

            write
                .set(Cow::Owned(String::from("updated")))
                .expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("updated")
            );
        })
        .expect("child scope should initialize");
}

#[wasm_bindgen_test]
fn reactive_string_reference_attribute_updates() {
    let host = host();
    let initial = String::from("initial");
    let updated = String::from("updated");
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, write) = scope.signal(&initial).expect("signal should initialize");
            let error_handler = test_handler(scope);
            let owner = ScopedMountOwner::new(scope);
            let view = Element::new("p").attr("data-text", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler)
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-text").as_deref(),
                Some("initial")
            );

            write.set(&updated).expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-text").as_deref(),
                Some("updated")
            );
        })
        .expect("child scope should initialize");
}

#[wasm_bindgen_test]
fn reactive_str_classes_merge_update_and_cleanup() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope
            .signal("dynamic-one")
            .expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("class", "static")
            .attr("class", read.into_rx());
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(element.class_list().contains("static"));
        assert!(element.class_list().contains("dynamic-one"));

        write.set("dynamic-two").expect("signal should be writable");
        assert!(element.class_list().contains("static"));
        assert!(!element.class_list().contains("dynamic-one"));
        assert!(element.class_list().contains("dynamic-two"));
    }

    root.dispose().expect("root cleanup should succeed");
    assert!(element.class_list().contains("static"));
    assert!(!element.class_list().contains("dynamic-two"));
}

#[wasm_bindgen_test]
fn reactive_str_stylesheet_merges_update_and_cleanup() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope
            .signal("color: red;")
            .expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("style", "display: block;")
            .attr("style", read.into_rx());
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        let initial = style_text(&element);
        assert!(initial.contains("display: block"), "{initial}");
        assert!(initial.contains("color: red"), "{initial}");

        write
            .set("color: blue;")
            .expect("signal should be writable");
        let updated = style_text(&element);
        assert!(updated.contains("display: block"), "{updated}");
        assert!(updated.contains("color: blue"), "{updated}");
        assert!(!updated.contains("color: red"), "{updated}");
    }

    root.dispose().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_cow_style_property_updates_and_cleans_up() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope
            .signal(Cow::Borrowed("red"))
            .expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        let initial = style_text(&element);
        assert!(initial.contains("display: block"), "{initial}");
        assert!(initial.contains("color: red"), "{initial}");

        write
            .set(Cow::Owned(String::from("blue")))
            .expect("signal should be writable");
        let updated = style_text(&element);
        assert!(updated.contains("display: block"), "{updated}");
        assert!(updated.contains("color: blue"), "{updated}");
        assert!(!updated.contains("color: red"), "{updated}");
    }

    root.dispose().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}
