use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root creation should succeed");
    let token = root
        .access()
        .completion_once(|_: i32| Ok::<(), ()>(()))
        .expect("completion creation should succeed");
    assert!(token.submit("wrong message type").is_ok());
}
