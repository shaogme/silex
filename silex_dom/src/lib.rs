pub mod attribute;
pub mod element;
pub mod view;

pub use attribute::*;
pub use element::*;
pub use view::*;

pub mod props;
pub use props::*;

pub mod tags;
pub use tags::*;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Document;

// --- Custom Panic Hook ---

#[cfg(debug_assertions)]
use std::panic;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "export function get_stack() { return new Error().stack; }")]
extern "C" {
    fn get_stack() -> String;
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: &str);
}

/// A panic hook for use with
/// [`std::panic::set_hook`](https://doc.rust-lang.org/nightly/std/panic/fn.set_hook.html)
/// that logs panics into
/// [`console.error`](https://developer.mozilla.org/en-US/docs/Web/API/Console/error).
///
/// On non-wasm targets, prints the panic to `stderr`.
#[allow(dead_code)]
#[cfg(debug_assertions)]
fn hook(info: &panic::PanicHookInfo) {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: get_stack is a JS interop call
        let stack = get_stack();
        let msg = format!("{}\n\nStack:\n\n{}\n\n", info, stack);
        error(&msg);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::{self, Write};
        let _ = writeln!(io::stderr(), "{}", info);
    }
}

#[cfg(debug_assertions)]
fn register() {
    panic::set_hook(Box::new(hook));
}

pub fn setup_global_error_handlers() {
    // 1. Panic Hook
    #[cfg(debug_assertions)]
    register();

    let window = web_sys::window().expect("Window not found");

    // 2. Window "error" event
    let closure = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
        silex_core::log::console_error(&format!("Global Error: {:?}", event.message()));
    }) as Box<dyn FnMut(_)>);

    window
        .add_event_listener_with_callback("error", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    // 3. Promise Rejection
    let closure_rej = Closure::wrap(Box::new(move |event: web_sys::PromiseRejectionEvent| {
        silex_core::log::console_error(&format!("Unhandled Rejection: {:?}", event.reason()));
    }) as Box<dyn FnMut(_)>);

    window
        .add_event_listener_with_callback(
            "unhandledrejection",
            closure_rej.as_ref().unchecked_ref(),
        )
        .unwrap();
    closure_rej.forget();
}

thread_local! {
    static DOCUMENT: Document = {
        let window = web_sys::window().expect("No global window");
        window.document().expect("No document")
    };
}

pub fn document() -> Document {
    DOCUMENT.with(|d| d.clone())
}
