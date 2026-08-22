use silex_core::{Runtime, RxGet};
use wasm_bindgen_futures::spawn_local;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        owner.with_transient(|child| {
            let value = child.signal(1i32).expect("signal should initialize");
            spawn_local(async move {
                let _ = value.get();
            });
        });
    });
}
