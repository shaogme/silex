use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let token = root
        .scope()
        .completion_once(|_: i32| Ok::<(), ()>(()));
    assert!(token.submit("wrong message type").is_ok());
}
