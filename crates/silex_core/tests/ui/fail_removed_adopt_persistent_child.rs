use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    let owner = root.access();
    let handler = owner
        .error_handler(|_| {})
        .expect("cleanup handler");
    let _ = owner.adopt_persistent_child((), |_| Ok(()), handler.view());
}
