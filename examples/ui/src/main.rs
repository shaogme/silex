#[cfg(target_arch = "wasm32")]
use silex::bootstrap::{JsAppHost, bootstrap_error_to_js};
#[cfg(target_arch = "wasm32")]
use silex::reexports::wasm_bindgen::{JsValue, prelude::wasm_bindgen};

/// Expose the application mount glue to the Trunk page owner.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = mountUi)]
pub fn mount_ui_js() -> Result<JsAppHost, JsValue> {
    silex_ui_example::mount_ui().map_err(|error| {
        bootstrap_error_to_js(&error).unwrap_or_else(|_| JsValue::from_str(&error.to_string()))
    })
}

fn main() {}
