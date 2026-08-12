use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let local = String::from("scoped");
        let handler = scope.error_handler(move |error: SilexError| {
            let _ = (&local, error);
        }).expect("handler should register");
        scope
            .effect(|| Ok::<(), SilexError>(()), handler)
            .expect("effect should initialize");
    }).expect("child scope should initialize");
}
