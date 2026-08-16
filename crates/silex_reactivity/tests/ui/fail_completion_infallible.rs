use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let _sender = scope.completion_sender(|_: i32| {});
    });
}
