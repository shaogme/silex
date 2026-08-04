use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _escaped = runtime.child(|scope| scope.child(|child| child.signal(1).0));
}
