use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    let _ = root.access().create_persistent_child();
}
