use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.effect(|| Ok::<(), SilexError>(()));
    });
}
