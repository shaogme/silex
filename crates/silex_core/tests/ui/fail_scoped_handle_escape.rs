use silex_core::{Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    let task = runtime
        .with_transient(|owner| {
            owner
                .spawn_scoped(
                    async {},
                    owner.error_handler(|_: SilexError| {}).unwrap(),
                )
                .unwrap()
        })
        .unwrap();
    let _ = task;
}
