use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (value, _) = scope.signal(1_i32).unwrap();
            if false {
                let _task = scope.spawn_scoped(
                    async move {
                        let _ = value.get();
                    },
                    scope.error_handler(|_| {}).unwrap(),
                );
            }
        })
        .unwrap();
}
