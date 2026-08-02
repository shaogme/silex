use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        let local = String::from("root-local");
        root.signal(&local);
    });
}
