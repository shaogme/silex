use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let _ = scope.effect_with_previous(
            |previous: Option<i32>| Ok::<i32, ()>(previous.unwrap_or_default()),
            scope
                .error_handler(|_| {})
                .expect("error handler registration should succeed"),
        );
    });
}
