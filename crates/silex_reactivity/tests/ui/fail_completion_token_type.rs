use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let token = root.scope().completion(|_: i32| {});
    let _ = token.submit("wrong message type");
}
