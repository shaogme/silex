use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let _ = owner.effect_with_previous(
            |previous: Option<i32>| Ok::<i32, SilexError>(previous.unwrap_or_default()),
            owner
                .error_handler(|_: SilexError| {})
                .expect("handler should register"),
        );
    });
}
