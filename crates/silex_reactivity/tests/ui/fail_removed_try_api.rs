use silex_reactivity::{Runtime, RuntimeInputs};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.try_effect_from(
            RuntimeInputs::new(),
            || Ok::<(), ()>(()),
            scope.error_handler(|_| {}),
        );
    });
}
