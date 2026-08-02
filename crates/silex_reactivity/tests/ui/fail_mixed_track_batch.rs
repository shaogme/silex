use silex_reactivity::{Runtime, track_batch};

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|root| {
        root.scope(|scope| {
            let (parent, _) = scope.signal(0i32);
            scope.scope(|child| {
                let (local, _) = child.signal(0i32);
                track_batch(&[parent, local]);
            });
        });
    });
}
