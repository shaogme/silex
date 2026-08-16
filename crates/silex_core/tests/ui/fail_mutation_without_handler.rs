use silex_core::{Mutation, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let _ = Mutation::new(owner, |_: u32| async { Ok::<u32, ()>(1) });
    });
}
