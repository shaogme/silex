use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        let value = String::from("scope-local");
        let token = root.completion(|_: ()| {
            let _ = value.as_str();
        });
        drop(value);
        let _ = token.submit(());
    });
}
