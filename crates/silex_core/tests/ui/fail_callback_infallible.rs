use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _callback = scope.callback(|_: i32| {});
    });
}
