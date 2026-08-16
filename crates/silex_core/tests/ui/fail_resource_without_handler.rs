use silex_core::{Resource, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let (source, _) = owner.signal(1_u32);
        let _ = Resource::new(owner, source, |_| async { Ok::<u32, ()>(1) }, None);
    });
}
