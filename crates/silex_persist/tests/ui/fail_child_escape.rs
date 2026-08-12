use silex_core::Runtime;
use silex_persist::Persistent;

fn escaped() -> Persistent<'static, i32> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        Persistent::builder(
            scope,
            "counter",
            scope
                .error_handler(|_| {})
                .expect("error handler should be registered"),
        )
        .local()
        .parse::<i32>()
        .default(0)
        .build()
        .expect("persistent binding should build")
    })
    .expect("child scope should run")
}

fn main() {
    let _ = escaped();
}
