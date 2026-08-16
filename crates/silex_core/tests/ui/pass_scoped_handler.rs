use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let local = String::from("scoped");
        let handler = owner.error_handler(move |error: SilexError| {
            let _ = (&local, error);
        }).expect("handler should register");
        owner
            .effect(|| Ok::<(), SilexError>(()), handler)
            .expect("effect should initialize");
    }).expect("child owner should initialize");
}
