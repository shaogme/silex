use silex_core::Runtime;
use wasm_bindgen_futures::spawn_local;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        root.child(|scope| {
            let (value, _) = scope.signal(1i32);
            spawn_local(async move {
                let _ = value.get();
            });
        });
    });
}
