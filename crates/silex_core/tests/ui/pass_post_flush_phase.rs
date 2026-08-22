use silex_core::{EffectPhase, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let handler = owner
                .error_handler(|_: SilexError| {})
                .expect("handler should register");
            owner
                .effect(
                    EffectPhase::PostFlush,
                    || Ok::<(), SilexError>(()),
                    handler,
                )
                .expect("post-flush effect should initialize");
        })
        .expect("owner should complete");
}
