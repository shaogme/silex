use silex_reactivity::{ErrorHandler, Runtime};

#[test]
fn error_handler_clone_keeps_scoped_callback_contract() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let label = String::from("scoped");
        let handler = ErrorHandler::new(move |error: &'static str| {
            assert_eq!(error, label);
        });
        let cloned = handler.clone();

        cloned.handle("scoped");
        scope
            .effect(|| Ok::<(), &'static str>(()), handler)
            .expect("effect should initialize");
    });
}
