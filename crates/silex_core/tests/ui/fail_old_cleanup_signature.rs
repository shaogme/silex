use silex_core::{ErrorHandler, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.on_cleanup(
            || {},
            ErrorHandler::<SilexError>::new(|_| {}),
        );
    });
}
