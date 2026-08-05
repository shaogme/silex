use silex_core::ReadSignal;
use silex_persist::{BackendEvent, BackendEventSink};
use std::rc::Rc;

fn static_sink<'scope>(signal: ReadSignal<'scope, i32>) -> BackendEventSink {
    Rc::new(move |_event: BackendEvent| {
        let _ = signal.get();
    })
}

fn main() {
    let _ = static_sink;
}
