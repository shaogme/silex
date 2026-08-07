use silex_reactivity::{ErrorHandler, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.effect_with_previous(
            |previous: Option<i32>| Ok::<i32, ()>(previous.unwrap_or_default()),
            ErrorHandler::ignore(),
        );
    });
}
