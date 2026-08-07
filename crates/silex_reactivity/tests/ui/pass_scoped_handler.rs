use silex_reactivity::{ErrorHandler, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let local = String::from("scoped");
        let handler = ErrorHandler::new(move |error: &'_ str| {
            assert_eq!(error, local.as_str());
        });
        scope
            .effect(|| Ok::<(), &str>(()), handler)
            .expect("effect should initialize");
    });
}
