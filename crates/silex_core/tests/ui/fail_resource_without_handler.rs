use silex_core::{Resource, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, _) = scope.signal(1_u32);
        let _ = Resource::new(scope, source, |_| async { Ok::<u32, ()>(1) }, None);
    });
}
