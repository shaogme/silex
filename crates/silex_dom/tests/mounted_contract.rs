use silex_core::{Runtime, SilexError, SilexErrorKind};
use silex_dom::mounted::{
    CleanupFailure, CleanupOrigin, CleanupReport, CleanupSink, DisposeError, DropFailureReport,
    MountAvailability, MountError,
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn mount_and_dispose_errors_keep_their_separate_ownership() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    root.with_access(|owner| {
        let handler = owner
            .error_handler(|_: SilexError| {})
            .expect("error handler should register");
        owner
            .on_cleanup(
                || -> silex_core::SilexResult<()> { panic!("root cleanup") },
                handler.view(),
            )
            .expect("cleanup should register");
    });

    let cleanup = root.close().expect_err("cleanup panic should be returned");
    let report = CleanupReport::from_parts(
        vec![CleanupFailure::new(CleanupOrigin::Root, cleanup)],
        vec![SilexError::recoverable(SilexErrorKind::Framework(
            "boundary failure".to_string(),
        ))],
    );
    assert!(!report.is_clean());
    assert_eq!(report.cleanup_failures().len(), 1);
    assert_eq!(report.boundary_errors().len(), 1);

    let mount = MountError::new(
        SilexError::recoverable(SilexErrorKind::Framework("primary failure".to_string())),
        report,
    );
    assert_eq!(
        mount.primary().to_string(),
        "Recoverable: Framework Error: primary failure"
    );

    let (primary, report, availability) = mount.into_parts();
    assert_eq!(availability, MountAvailability::Poisoned);
    assert_eq!(
        primary.to_string(),
        "Recoverable: Framework Error: primary failure"
    );
    let dispose = DisposeError::new(report);
    let report = dispose.into_parts();
    let (failures, boundary_errors) = report.into_parts();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].error.diagnostic().message(), "root cleanup");
    assert_eq!(boundary_errors.len(), 1);
}

#[test]
fn cleanup_sink_is_owned_and_can_be_called_without_a_scope() {
    let observed = Rc::new(RefCell::new(None));
    let observed_by_sink = observed.clone();
    let sink = CleanupSink::new(move |report| {
        *observed_by_sink.borrow_mut() = Some(report);
    });

    sink.record(DropFailureReport::new());
    assert!(
        observed
            .borrow()
            .as_ref()
            .is_some_and(DropFailureReport::is_clean)
    );
}
