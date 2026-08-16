use silex_core::{Mutation, Runtime, SilexError};

fn copy_value<T: Copy>(value: T) -> T {
    value
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let mutation = Mutation::new(
            owner,
            |_: String| async { Ok::<String, String>(String::new()) },
            owner
                .error_handler(|_: SilexError| {})
                .expect("handler should register"),
        )
        .expect("mutation should initialize");
        let copied = copy_value(mutation);
        let _ = (mutation, copied);
    }).expect("child owner should initialize");
}
