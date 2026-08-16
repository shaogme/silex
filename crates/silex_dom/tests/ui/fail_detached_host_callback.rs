use silex_core::Runtime;
use silex_dom::helpers::detached::window_event_listener_untyped;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|_| {
        let borrowed = String::from("detached");
        let borrowed_ref = borrowed.as_str();
        let _ = window_event_listener_untyped("click", move |_| {
            assert!(!borrowed_ref.is_empty());
        });
    });
}
