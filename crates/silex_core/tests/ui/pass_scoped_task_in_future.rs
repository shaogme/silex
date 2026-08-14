use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (value, _) = scope.signal(1i32).expect("signal should initialize");
            let error_handler = scope
                .error_handler(|_: SilexError| {})
                .expect("error handler should initialize");
            if false {
                scope
                    .spawn_scoped(
                        async move {
                            let _ = value.get();
                        },
                        error_handler,
                    )
                    .expect("task should initialize");
            }
        })
        .expect("scope should run");
}
