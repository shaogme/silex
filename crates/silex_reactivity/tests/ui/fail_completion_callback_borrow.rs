use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let value = String::from("scope-local");
        let token = scope.completion(|_: ()| {
            let _ = value.as_str();
        });
        drop(value);
        let _ = token.submit(());
    });
}
