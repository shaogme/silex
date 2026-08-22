use silex_reactivity::{track_batch, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let parent_signal = scope
            .signal(0i32)
            .expect("parent signal creation should succeed");
        scope.with_transient(|child| {
            let local_signal = child
                .signal(0i32)
                .expect("local signal creation should succeed");
            track_batch(&[parent_signal.read(), local_signal.read()]);
        });
    });
}
