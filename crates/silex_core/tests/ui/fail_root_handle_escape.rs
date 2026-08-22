use silex_core::{Runtime, RxGet};

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let signal = root
        .access()
        .signal(0i32)
        .expect("signal should initialize")
        .read_signal();
    drop(root);
    let _ = signal.get();
}
