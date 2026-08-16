use silex_core::{Runtime, SilexContext};
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let error_handler = owner
                .error_handler(|_| {})
                .expect("handler should register");
            let _style = Style::new(SilexContext::new(owner, error_handler.view()))
                .raw("--color", "red");
        })
        .expect("transient owner should initialize");
}
