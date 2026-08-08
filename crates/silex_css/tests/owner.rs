#![cfg(target_arch = "wasm32")]

use js_sys::{Array, Reflect};
use silex_core::{ErrorReporter, Runtime, Scope};
use silex_css::{
    CssPart, DynamicCss, IntoCssReactive,
    prelude::{
        Style, ThemePatchToCss, ThemeToCss, ThemeType, set_global_theme, theme_patch,
        theme_variables,
    },
};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom, AttrOp},
    view::{ScopedViewOwner, ViewOwner},
};
use std::{
    cell::Cell,
    fmt::{Display, Formatter},
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{Element, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {})
}

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("browser tests have a window")
        .document()
        .expect("browser tests have a document")
}

fn mount_point() -> Element {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("browser tests have a body")
        .append_child(&host)
        .expect("test host can be mounted");
    host
}

fn remove(node: &Node) {
    if let Some(parent) = node.parent_node() {
        parent.remove_child(node).expect("test node can be removed");
    }
}

fn adopted_sheet_contains(needle: &str) -> bool {
    let sheets = Reflect::get(
        document().as_ref(),
        &JsValue::from_str("adoptedStyleSheets"),
    )
    .expect("document exposes adoptedStyleSheets");
    let sheets = Array::from(&sheets);
    for sheet_index in 0..sheets.length() {
        let sheet = sheets.get(sheet_index);
        let rules = Reflect::get(&sheet, &JsValue::from_str("cssRules"))
            .expect("constructed stylesheet exposes cssRules");
        let rules = Array::from(&rules);
        for rule_index in 0..rules.length() {
            let rule = rules.get(rule_index);
            let text = Reflect::get(&rule, &JsValue::from_str("cssText"))
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_default();
            if text.contains(needle) {
                return true;
            }
        }
    }
    false
}

async fn flush_style_microtasks() {
    for _ in 0..4 {
        JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
            .await
            .expect("microtask promise resolves");
    }
}

#[derive(Clone)]
struct TestTheme {
    color: String,
}

impl Display for TestTheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "--theme-color:{};", self.color)
    }
}

impl ThemeType for TestTheme {}

impl ThemeToCss for TestTheme {
    fn get_variable_values(&self) -> Vec<String> {
        vec![self.color.clone()]
    }

    fn get_variable_names() -> &'static [&'static str] {
        &["--theme-color"]
    }
}

#[derive(Clone)]
struct TestPatch {
    alternate: bool,
}

impl ThemePatchToCss for TestPatch {
    fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)> {
        if self.alternate {
            vec![("--patch-new", Some(String::from("blue")))]
        } else {
            vec![("--patch-old", Some(String::from("red")))]
        }
    }
}

impl Display for TestPatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("patch")
    }
}

#[wasm_bindgen_test]
fn style_updates_inline_values_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        let token = owner.token();
        let class_name = Style::new()
            .raw("--test-color", value)
            .apply_to_element(&element, &token)
            .expect("style can be applied");

        assert!(element.class_list().contains(&class_name));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("red")
        );

        set_value.set(String::from("blue"));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("blue")
        );
    });

    assert!(element.class_name().is_empty());
    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn theme_updates_variables_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (theme, set_theme) = scope.signal(TestTheme {
            color: String::from("red"),
        });
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        let token = owner.token();
        theme_variables(theme)
            .apply(&element, ApplyTarget::Apply, &token)
            .expect("theme variables can be applied");

        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--theme-color: red")
        );
        set_theme.set(TestTheme {
            color: String::from("blue"),
        });
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--theme-color: blue")
        );
    });

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn svg_style_updates_inline_values_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .expect("svg element can be created");
    host.append_child(&element)
        .expect("svg element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        let token = owner.token();
        Style::new()
            .raw("--svg-color", value)
            .apply_to_element(&element, &token)
            .expect("svg style can be applied");
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("red")
        );

        set_value.set(String::from("blue"));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("blue")
        );
    });

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn dynamic_css_replaces_rule_class_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        let token = owner.token();
        let dynamic = DynamicCss::new("slx-owner-test").with_rule(
            &[
                CssPart::Lit("."),
                CssPart::Class,
                CssPart::Lit(" "),
                CssPart::SelectorVal(0),
                CssPart::Lit("{color:red}"),
            ],
            vec![value.into_css_reactive()],
        );

        dynamic
            .apply(&element, ApplyTarget::Class, &token)
            .expect("dynamic style can be applied");
        let first_class = element.class_name();
        assert!(first_class.contains("slx-owner-test"));
        assert!(first_class.contains("-d"));

        set_value.set(String::from("blue"));
        let second_class = element.class_name();
        assert!(second_class.contains("slx-owner-test"));
        assert_ne!(first_class, second_class);
    });

    assert!(element.class_name().is_empty());
    remove(&host.into());
}

#[wasm_bindgen_test(async)]
async fn pending_dynamic_sheet_operations_do_not_survive_owner_dispose() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        let token = owner.token();
        let dynamic = DynamicCss::new("slx-pending-owner").with_rule(
            &[
                CssPart::Lit("."),
                CssPart::Class,
                CssPart::Lit(" "),
                CssPart::SelectorVal(0),
                CssPart::Lit("{color:red}"),
            ],
            vec![value.into_css_reactive()],
        );
        dynamic
            .apply(&element, ApplyTarget::Class, &token)
            .expect("dynamic style can be applied");
        assert!(element.class_name().contains("slx-pending-owner"));
    });

    flush_style_microtasks().await;
    assert!(!adopted_sheet_contains("slx-pending-owner"));
    remove(&host.into());
}

#[wasm_bindgen_test(async)]
async fn global_theme_stylesheets_are_isolated_per_owner() {
    let mut first_runtime = Runtime::new();
    let first_root = first_runtime.run();
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime.run();

    first_root.with_scope(|first_scope| {
        second_root.with_scope(|second_scope| {
            let first_owner = ScopedViewOwner::new(first_scope, test_handler());
            let second_owner = ScopedViewOwner::new(second_scope, test_handler());
            set_global_theme(
                &first_owner,
                first_scope.stored(TestTheme {
                    color: String::from("owner-red"),
                }),
            )
            .expect("first global theme can be registered");
            set_global_theme(
                &second_owner,
                second_scope.stored(TestTheme {
                    color: String::from("owner-blue"),
                }),
            )
            .expect("second global theme can be registered");
        });
    });

    flush_style_microtasks().await;
    assert!(adopted_sheet_contains("owner-red"));
    assert!(adopted_sheet_contains("owner-blue"));

    first_root.dispose().expect("first owner can be disposed");
    flush_style_microtasks().await;
    assert!(!adopted_sheet_contains("owner-red"));
    assert!(adopted_sheet_contains("owner-blue"));

    second_root.dispose().expect("second owner can be disposed");
    flush_style_microtasks().await;
    assert!(!adopted_sheet_contains("owner-blue"));
}

#[wasm_bindgen_test]
fn theme_patch_removes_variables_that_disappear_from_the_next_round() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (patch, set_patch) = scope.signal(TestPatch { alternate: false });
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();
        theme_patch(patch)
            .apply(&element, ApplyTarget::Apply, &token)
            .expect("theme patch can be applied");
        let initial = element.get_attribute("style").unwrap_or_default();
        assert!(initial.contains("--patch-old"), "{initial}");

        set_patch.set(TestPatch { alternate: true });
        let updated = element.get_attribute("style").unwrap_or_default();
        assert!(!updated.contains("--patch-old"), "{updated}");
        assert!(updated.contains("--patch-new"), "{updated}");
    });

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn foreign_runtime_css_input_is_rejected_before_custom_callback() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut foreign_runtime = Runtime::new();
    let foreign_inputs =
        foreign_runtime.child(|scope| scope.rw_signal(1i32).into_rx().runtime_inputs());
    let callback_runs = Cell::new(0);

    let mut local_runtime = Runtime::new();
    local_runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope, test_handler());
        let token = owner.token();
        let operation = AttrOp::custom_with_inputs(foreign_inputs, |element, _| {
            callback_runs.set(callback_runs.get() + 1);
            let _ = element.set_attribute("data-foreign", "unexpected");
            Ok(())
        });
        operation
            .apply(&element, &token)
            .expect_err("foreign runtime input should be rejected");
    });

    assert_eq!(callback_runs.get(), 0);
    assert!(!element.has_attribute("data-foreign"));
    remove(&host.into());
}
