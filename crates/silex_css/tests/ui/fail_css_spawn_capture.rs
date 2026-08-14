use silex_core::{Runtime, SilexContext};
use silex_css::prelude::*;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = scope
            .error_handler(|_| {})
            .expect("handler should register");
        let style = Style::new(SilexContext::new(scope, error_handler))
            .raw("--color", value)
            .expect("style should build");
        require_static(style);
    });
}
