use silex_reactivity::{Callback, ReactiveError, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
        let callback: Callback<'_, String> = scope
            .callback(|_: String| Ok::<(), ReactiveError>(()))
            .expect("typed callback should initialize");
        callback
            .invoke(String::from("value"))
            .expect("typed callback should be alive");
        })
        .expect("child scope should complete");
}
