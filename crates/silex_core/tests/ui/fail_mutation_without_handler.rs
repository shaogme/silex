use silex_core::{Mutation, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = Mutation::new(scope, |_: u32| async { Ok::<u32, ()>(1) });
    });
}
