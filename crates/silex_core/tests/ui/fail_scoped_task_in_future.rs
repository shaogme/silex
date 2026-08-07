use silex_core::{ErrorHandler, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(1i32);
        scope.spawn_scoped(
            async move {
                let _ = value.get();
            },
            ErrorHandler::new(|_: SilexError| {}),
        );
    });
}
