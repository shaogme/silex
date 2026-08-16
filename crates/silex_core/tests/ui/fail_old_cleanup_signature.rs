use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let _ = owner.on_cleanup(
            || {},
            owner
                .error_handler(|_: SilexError| {})
                .expect("handler should register"),
        );
    });
}
