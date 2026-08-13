#![cfg(target_arch = "wasm32")]

use std::borrow::Cow;

use silex_core::{ErrorReporter, Runtime, Scope, SilexError, SilexErrorKind, SilexResult};
use silex_dom::attribute::AttributeBuilder;
use silex_dom::element::Element;
use silex_dom::view::{
    AnyView, ApplyAttributes, MountInstance, MountOwner, ScopedMountOwner, View,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_test::*;
use web_sys::{Element as DomElement, Node};

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

struct RejectingView {
    element: Rc<RefCell<Option<DomElement>>>,
}

impl<'scope> ApplyAttributes<'scope> for RejectingView {}

impl<'scope> View<'scope> for RejectingView {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let document = web_sys::window()
            .ok_or_else(|| SilexError::fatal(SilexErrorKind::Dom("window is unavailable".into())))?
            .document()
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom("document is unavailable".into()))
            })?;
        let element = document.create_element("div").map_err(SilexError::fatal)?;
        parent.append_child(&element).map_err(SilexError::fatal)?;
        let token = owner.token();
        for attr in attrs {
            attr.apply(&element, &token, error_handler)?;
        }
        *self.element.borrow_mut() = Some(element);
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "intentional mount rejection".to_string(),
        )))
    }
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

#[wasm_bindgen_test]
fn reactive_borrowed_str_style_property_updates_and_cleans_up() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope.signal("red").expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write.set("blue").expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.dispose().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_string_reference_style_property_updates_and_cleans_up() {
    let host = host();
    let initial = String::from("red");
    let updated = String::from("blue");
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope.signal(&initial).expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write.set(&updated).expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.dispose().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_style_property_restores_static_value_after_dispose() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let element;
    {
        let scope = root.scope();
        let (read, write) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = test_handler(scope);
        let owner = ScopedMountOwner::new(scope);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("color", "green"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write
            .set(String::from("blue"))
            .expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.dispose().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("color: green"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_style_plan_cleans_up_after_failed_mount() {
    let host = host();
    let captured = Rc::new(RefCell::new(None));
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, _) = scope
                .signal(String::from("red"))
                .expect("signal should initialize");
            let error_handler = test_handler(scope);
            let owner = ScopedMountOwner::new(scope);
            let view = AnyView::new(RejectingView {
                element: captured.clone(),
            })
            .attr("style", ("color", read.into_rx()));
            assert!(
                view.mount(&owner, &host, Vec::new(), error_handler)
                    .is_err()
            );
        })
        .expect("child scope should initialize");

    let element = captured
        .borrow()
        .clone()
        .expect("failed mount should expose its detached element");
    assert!(!style_text(&element).contains("color"));
}
