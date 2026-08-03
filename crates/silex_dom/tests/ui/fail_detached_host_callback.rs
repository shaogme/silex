use silex_core::Runtime;
use silex_dom::helpers::window_event_listener_untyped_detached;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|_| {
        let borrowed = String::from("detached");
        let borrowed_ref = borrowed.as_str();
        let _ = window_event_listener_untyped_detached("click", move |_| {
            assert!(!borrowed_ref.is_empty());
        });
    });
}
