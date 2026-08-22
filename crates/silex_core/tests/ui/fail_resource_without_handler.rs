use silex_core::{Resource, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let source = owner.signal(1_u32).expect("source");
        let _ = Resource::builder(owner)
            .source(source)
            .fetch(|_| async { Ok::<u32, ()>(1) })
            .build(());
    });
}
