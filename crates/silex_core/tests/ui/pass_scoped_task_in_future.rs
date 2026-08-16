use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (value, _) = owner.signal(1i32).expect("signal should initialize");
            let error_handler = owner
                .error_handler(|_: SilexError| {})
                .expect("error handler should initialize");
            if false {
                owner
                    .spawn_scoped(
                        async move {
                            let _ = value.get();
                        },
                        error_handler,
                    )
                    .expect("task should initialize");
            }
        })
        .expect("owner should run");
}
