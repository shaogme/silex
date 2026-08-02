use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        let _signal = root.child(|child| child.signal(0i32).0);
    });
}
