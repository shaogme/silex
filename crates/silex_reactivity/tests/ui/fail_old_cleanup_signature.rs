use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.on_cleanup(
            || {},
            scope
                .error_handler(|_| {})
                .expect("error handler registration should succeed"),
        );
    });
}
