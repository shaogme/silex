use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _sender = scope.completion_sender(|_: i32| {});
    });
}
