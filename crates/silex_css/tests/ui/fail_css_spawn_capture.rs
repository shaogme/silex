use silex_core::{Runtime, SilexContext};
use silex_css::prelude::*;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let value = owner
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("handler should register");
        let style = Style::new(SilexContext::new(owner, error_handler.view()))
            .raw("--color", value)
            .expect("style should build");
        require_static(style);
    });
}
