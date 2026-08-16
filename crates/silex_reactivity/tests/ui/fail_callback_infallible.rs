use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let _callback = scope.callback(|_: i32| {});
    });
}
