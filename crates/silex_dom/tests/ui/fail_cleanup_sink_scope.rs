use silex_core::{Runtime, SilexError, SilexErrorKind};
use silex_dom::mounted::CleanupSink;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let handler = owner.error_handler(|_: SilexError| {}).expect("handler");
        let _sink = CleanupSink::new(move |_| {
            handler.handle(SilexError::recoverable(SilexErrorKind::Framework("scoped".to_string())));
        });
    });
}
