use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _escaped = runtime.with_transient(|owner| owner.with_transient(|child| child.signal(1).0));
}
