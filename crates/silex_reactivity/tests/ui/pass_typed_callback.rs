use silex_reactivity::{Callback, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let callback: Callback<'_, String> = scope.callback(|_: String| {});
        callback
            .invoke(String::from("value"))
            .expect("typed callback should be alive");
    });
}
