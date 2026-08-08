use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.try_on_cleanup(
            || Ok::<(), SilexError>(()),
            scope.error_handler(|_: SilexError| {}),
        );
    });
}
