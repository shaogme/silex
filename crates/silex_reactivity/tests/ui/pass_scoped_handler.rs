use silex_reactivity::{EffectPhase, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
        let local = String::from("scoped");
        let handler = scope
            .error_handler(move |error: &'_ str| {
                assert_eq!(error, local.as_str());
            })
            .expect("handler should initialize");
        scope
            .effect(EffectPhase::Normal, || Ok::<(), &str>(()), handler)
            .expect("effect should initialize");
        })
        .expect("child scope should complete");
}
