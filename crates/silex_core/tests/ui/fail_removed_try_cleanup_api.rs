use silex_core::{ErrorHandler, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.try_on_cleanup(
            || Ok::<(), SilexError>(()),
            ErrorHandler::<SilexError>::new(|_| {}),
        );
    });
}
