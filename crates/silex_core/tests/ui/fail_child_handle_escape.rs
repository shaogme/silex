use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _escaped = runtime.with_transient(|owner| {
        owner.with_transient(|child| {
            let signal = child.signal(1).expect("signal should initialize");
            signal.read_signal()
        })
    });
}
