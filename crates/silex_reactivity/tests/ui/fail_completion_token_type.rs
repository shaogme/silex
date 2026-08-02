use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let token = scope.completion(|_: i32| {});
        let _ = token.submit("wrong message type");
    });
}
