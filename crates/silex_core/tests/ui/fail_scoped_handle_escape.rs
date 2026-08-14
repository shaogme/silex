use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    let task = runtime
        .child(|scope| {
            scope
                .spawn_scoped(
                    async {},
                    scope.error_handler(|_: SilexError| {}).unwrap(),
                )
                .unwrap()
        })
        .unwrap();
    let _ = task;
}
