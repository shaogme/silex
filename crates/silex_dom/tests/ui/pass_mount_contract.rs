use silex_core::{Runtime, SilexError};
use silex_dom::mounted::{CleanupReport, CleanupSink, DropFailureReport, MountError};

fn main() {
    let sink = CleanupSink::new(|report| assert!(report.is_clean()));
    let mut runtime = Runtime::new();
    let root = runtime.try_run().expect("root should be created");

    root.with_scope(|scope| {
        let _handler = scope.error_handler(|_: SilexError| {});
        let _stored = scope.stored("scoped");
    });

    let mount = MountError::new(SilexError::Framework("primary".to_string()), CleanupReport::new());
    let (_, rollback) = mount.into_parts();
    assert!(rollback.is_clean());
    sink.record(DropFailureReport::new());
    root.dispose().expect("root should dispose");
}
