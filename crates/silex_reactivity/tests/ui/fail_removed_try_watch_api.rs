use silex_reactivity::{Runtime, WatchOptions};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.try_watch_getter_from(
            silex_reactivity::RuntimeInputs::new(),
            || Ok::<i32, ()>(0),
            |_, _| Ok::<(), ()>(() ),
            WatchOptions::default(),
        );
    });
}
