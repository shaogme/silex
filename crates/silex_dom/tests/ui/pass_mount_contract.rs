use silex_core::{Runtime, SilexError, SilexErrorKind};
use silex_dom::mounted::{CleanupReport, CleanupSink, DropFailureReport, MountError};

fn main() {
    let sink = CleanupSink::new(|report| assert!(report.is_clean()));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should be created");

    root.with_access(|owner| {
        let _handler = owner
            .error_handler(|_: SilexError| {})
            .expect("error handler should register");
        let _stored = owner.stored("owned").expect("stored should initialize");
    });

    let mount = MountError::new(
        SilexError::recoverable(SilexErrorKind::Framework("primary".to_string())),
        CleanupReport::new(),
    );
    let (_, rollback, _) = mount.into_parts();
    assert!(rollback.is_clean());
    sink.record(DropFailureReport::new());
    root.close().expect("root should close");
}
