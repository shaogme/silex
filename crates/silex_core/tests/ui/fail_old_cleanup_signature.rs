use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.on_cleanup(
            || {},
            scope.error_handler(|_: SilexError| {}),
        );
    });
}
