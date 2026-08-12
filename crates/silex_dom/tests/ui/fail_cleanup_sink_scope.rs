use silex_core::{Runtime, SilexError};
use silex_dom::mounted::CleanupSink;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler = scope.error_handler(|_: SilexError| {}).expect("handler");
        let _sink = CleanupSink::new(move |_| {
            handler.handle(SilexError::Framework("scoped".to_string()));
        });
    });
}
