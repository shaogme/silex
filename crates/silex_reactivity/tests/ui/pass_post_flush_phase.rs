use silex_reactivity::{EffectPhase, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let handler = scope
                .error_handler(|_: ()| {})
                .expect("handler should register");
            scope
                .effect(EffectPhase::PostFlush, || Ok::<(), ()>(()), handler)
                .expect("post-flush effect should initialize");
        })
        .expect("scope should complete");
}
