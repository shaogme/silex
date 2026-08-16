use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let _ = scope.on_cleanup(
            || {},
            scope
                .error_handler(|_| {})
                .expect("error handler registration should succeed"),
        );
    });
}
