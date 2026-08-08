use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.effect_with_previous(
            |previous: Option<i32>| Ok::<i32, SilexError>(previous.unwrap_or_default()),
            scope.error_handler(|_: SilexError| {}),
        );
    });
}
