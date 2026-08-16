use silex_core::Runtime;
use silex_persist::Persistent;

fn escaped() -> Persistent<'static, i32> {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            Persistent::builder(
                owner,
                "counter",
                owner
                    .error_handler(|_| {})
                    .expect("error handler should be registered"),
            )
            .local()
            .parse::<i32>()
            .default(0)
            .build()
            .expect("persistent binding should build")
        })
        .expect("transient owner should run")
}

fn main() {
    let _ = escaped();
}
