use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let _ = owner.try_on_cleanup(
            || Ok::<(), SilexError>(()),
            owner.error_handler(|_: SilexError| {}),
        );
    });
}
