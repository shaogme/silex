#![cfg(target_arch = "wasm32")]

use js_sys::{Array, Reflect};
use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, SilexContext};
use silex_css::{
    CssPart, DynamicCss, IntoCssReactive,
    prelude::{
        Style, ThemePatchToCss, ThemeToCss, ThemeType, set_global_theme, theme_patch,
        theme_variables,
    },
};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom, AttrOp},
    view::{MountContext, MountOwner, MountOwnerToken},
};
use std::{
    cell::Cell,
    fmt::{Display, Formatter},
    rc::Rc,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{Element, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
    owner
        .error_handler(|_| {})
        .expect("test error handler should register")
}

fn test_owner<'scope>(
    owner: OwnerAccess<'scope>,
) -> (MountOwnerToken<'scope>, ErrorHandlerToken<'scope>) {
    let error_handler = test_handler(owner);
    (MountOwnerToken::new(owner), error_handler)
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
    runtime
        .with_transient(|owner| {
            let (value, set_value) = owner
                .signal(String::from("red"))
                .expect("signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
            let class_name = Style::new(SilexContext::new(owner, error_handler.view()))
                .raw("--test-color", value)
                .expect("style should build")
                .apply_to_element(&element, &token, error_handler.view())
                .expect("style can be applied");

            assert!(element.class_list().contains(&class_name));
            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("red")
            );

            set_value
                .set(String::from("blue"))
                .expect("signal should update");
            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("blue")
            );
        })
        .expect("child scope should initialize");

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
    runtime
        .with_transient(|owner| {
            let (theme, set_theme) = owner
                .signal(TestTheme {
                    color: String::from("red"),
                })
                .expect("theme signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
            theme_variables(theme)
                .apply(&element, ApplyTarget::Apply, &token, error_handler.view())
                .expect("theme variables can be applied");

            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("--theme-color: red")
            );
            set_theme
                .set(TestTheme {
                    color: String::from("blue"),
                })
                .expect("theme signal should update");
            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("--theme-color: blue")
            );
        })
        .expect("child scope should initialize");

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
    runtime
        .with_transient(|owner| {
            let (value, set_value) = owner
                .signal(String::from("red"))
                .expect("signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
            Style::new(SilexContext::new(owner, error_handler.view()))
                .raw("--svg-color", value)
                .expect("style should build")
                .apply_to_element(&element, &token, error_handler.view())
                .expect("svg style can be applied");
            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("red")
            );

            set_value
                .set(String::from("blue"))
                .expect("signal should update");
            assert!(
                element
                    .get_attribute("style")
                    .unwrap_or_default()
                    .contains("blue")
            );
        })
        .expect("child scope should initialize");

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
    runtime
        .with_transient(|owner| {
            let (value, set_value) = owner
                .signal(String::from("red"))
                .expect("signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
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
                .apply(&element, ApplyTarget::Class, &token, error_handler.view())
                .expect("dynamic style can be applied");
            let first_class = element.class_name();
            assert!(first_class.contains("slx-owner-test"));
            assert!(first_class.contains("-d"));

            set_value
                .set(String::from("blue"))
                .expect("signal should update");
            let second_class = element.class_name();
            assert!(second_class.contains("slx-owner-test"));
            assert_ne!(first_class, second_class);
        })
        .expect("child scope should initialize");

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
    runtime
        .with_transient(|owner| {
            let (value, _) = owner
                .signal(String::from("red"))
                .expect("signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
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
                .apply(&element, ApplyTarget::Class, &token, error_handler.view())
                .expect("dynamic style can be applied");
            assert!(element.class_name().contains("slx-pending-owner"));
        })
        .expect("child scope should initialize");

    flush_style_microtasks().await;
    assert!(!adopted_sheet_contains("slx-pending-owner"));
    remove(&host.into());
}

#[wasm_bindgen_test(async)]
async fn global_theme_stylesheets_are_isolated_per_owner() {
    let mut first_runtime = Runtime::new();
    let first_root = first_runtime
        .owner()
        .expect("first owner should initialize");
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime
        .owner()
        .expect("second owner should initialize");

    first_root.with_access(|first_access| {
        second_root.with_access(|second_access| {
            let (first_owner, first_error_handler) = test_owner(first_access);
            let (second_owner, second_error_handler) = test_owner(second_access);
            set_global_theme(
                &first_owner,
                first_access
                    .stored(TestTheme {
                        color: String::from("owner-red"),
                    })
                    .expect("first theme should initialize"),
                first_error_handler.view(),
            )
            .expect("first global theme can be registered");
            set_global_theme(
                &second_owner,
                second_access
                    .stored(TestTheme {
                        color: String::from("owner-blue"),
                    })
                    .expect("second theme should initialize"),
                second_error_handler.view(),
            )
            .expect("second global theme can be registered");
        });
    });

    flush_style_microtasks().await;
    assert!(adopted_sheet_contains("owner-red"));
    assert!(adopted_sheet_contains("owner-blue"));

    first_root.close().expect("first owner can be closed");
    flush_style_microtasks().await;
    assert!(!adopted_sheet_contains("owner-red"));
    assert!(adopted_sheet_contains("owner-blue"));

    second_root.close().expect("second owner can be closed");
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
    runtime
        .with_transient(|owner| {
            let (patch, set_patch) = owner
                .signal(TestPatch { alternate: false })
                .expect("patch signal should initialize");
            let (owner_token, error_handler) = test_owner(owner);
            let token = owner_token.token();
            theme_patch(patch)
                .apply(&element, ApplyTarget::Apply, &token, error_handler.view())
                .expect("theme patch can be applied");
            let initial = element.get_attribute("style").unwrap_or_default();
            assert!(initial.contains("--patch-old"), "{initial}");

            set_patch
                .set(TestPatch { alternate: true })
                .expect("patch signal should update");
            let updated = element.get_attribute("style").unwrap_or_default();
            assert!(!updated.contains("--patch-old"), "{updated}");
            assert!(updated.contains("--patch-new"), "{updated}");
        })
        .expect("child scope should initialize");

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn foreign_runtime_css_read_is_rejected_during_custom_callback() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime
        .owner()
        .expect("foreign owner should initialize");
    let mut local_runtime = Runtime::new();
    let callback_runs = Rc::new(Cell::new(0));

    foreign_root.with_access(|foreign_owner| {
        let (foreign, _) = foreign_owner
            .signal(1_i32)
            .expect("foreign signal should initialize");
        local_runtime
            .with_transient(|owner| {
                let (owner_token, error_handler) = test_owner(owner);
                let token = owner_token.token();
                let callback_runs_in_operation = callback_runs.clone();
                let operation = AttrOp::on_commit(move |element, _| {
                    callback_runs_in_operation.set(callback_runs_in_operation.get() + 1);
                    foreign.get().map(|_| ())?;
                    let _ = element.set_attribute("data-foreign", "unexpected");
                    Ok(())
                });
                let context =
                    MountContext::for_parent(element.clone().into(), token, error_handler.view());
                operation
                    .apply(&element, &context)
                    .expect("foreign operation should register");
                context
                    .transaction()
                    .commit()
                    .expect_err("foreign runtime read should be rejected");
            })
            .expect("local transient owner should initialize");
    });

    assert_eq!(callback_runs.get(), 1);
    assert!(!element.has_attribute("data-foreign"));
    remove(&host.into());
    foreign_root.close().expect("foreign owner cleanup");
}
