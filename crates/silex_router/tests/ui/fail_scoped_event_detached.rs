use silex_core::Runtime;
use silex_dom::helpers::detached::window_event_listener_untyped;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(1_i32).expect("value signal should be created");
        let _ = window_event_listener_untyped("popstate", move |_| {
            let _ = value.get();
        });
    });
}
