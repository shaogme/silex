use silex_reactivity::{track_batch, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (parent, _) = scope
            .signal(0i32)
            .expect("parent signal creation should succeed");
        scope.child(|child| {
            let (local, _) = child
                .signal(0i32)
                .expect("local signal creation should succeed");
            track_batch(&[parent, local]);
        });
    });
}
