use silex_core::{Runtime, SilexContext};
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let error_handler = scope
                .error_handler(|_| {})
                .expect("handler should register");
            let _style = Style::new(SilexContext::new(scope, error_handler))
                .raw("--color", "red");
        })
        .expect("runtime child should initialize");
}
