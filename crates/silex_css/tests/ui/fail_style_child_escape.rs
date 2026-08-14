use silex_core::{Runtime, SilexContext};
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    let style = runtime.child(|scope| {
        let (value, _) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = scope
            .error_handler(|_| {})
            .expect("handler should register");
        Style::new(SilexContext::new(scope, error_handler))
            .raw("--color", value)
            .expect("style should build")
    });
    let _ = style;
}
