use std::cell::Cell;

use js_sys::Promise;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Element;

pub async fn yield_microtask() -> Result<(), JsValue> {
    JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
        .await
        .map(|_| ())
}

pub async fn wait_until_dom_text<F>(
    host: &Element,
    expected: &str,
    max_attempts: usize,
    diagnostics: F,
) where
    F: Fn() -> String,
{
    for _ in 0..=max_attempts {
        if host.text_content().as_deref() == Some(expected) {
            return;
        }
        yield_microtask()
            .await
            .expect("DOM wait microtask should resolve");
    }

    panic!(
        "timed out waiting for DOM text {expected:?}; actual={:?}; diagnostics={}",
        host.text_content(),
        diagnostics()
    );
}

pub async fn wait_until_owner_closed<F>(
    is_closed: F,
    max_attempts: usize,
    diagnostics: impl Fn() -> String,
) where
    F: Fn() -> bool,
{
    for _ in 0..=max_attempts {
        if is_closed() {
            return;
        }
        yield_microtask()
            .await
            .expect("owner wait microtask should resolve");
    }

    panic!(
        "timed out waiting for owner close; diagnostics={}",
        diagnostics()
    );
}

pub async fn wait_until_condition<F, D>(
    condition: F,
    max_attempts: usize,
    description: &str,
    diagnostics: D,
) where
    F: Fn() -> bool,
    D: Fn() -> String,
{
    for _ in 0..=max_attempts {
        if condition() {
            return;
        }
        yield_microtask()
            .await
            .expect("condition wait microtask should resolve");
    }

    panic!(
        "timed out waiting for {description}; diagnostics={}",
        diagnostics()
    );
}

pub fn assert_no_parent_error(errors: &Cell<usize>, diagnostics: impl Fn() -> String) {
    assert_eq!(
        errors.get(),
        0,
        "parent error handler received {} errors; diagnostics={}",
        errors.get(),
        diagnostics()
    );
}
