use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        let _escaped = root.scope(|child| child.signal(1).0);
    });
}
