use silex_bootstrap::AppHostError;
use silex_core::{Runtime, SilexError};
use silex_dom::{CleanupFailure, CleanupOrigin, CleanupReport, DisposeError, MountError};

fn cleanup_error() -> silex_core::CleanupError {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        scope
            .on_cleanup(
                || panic!("host cleanup failure"),
                scope.error_handler(|_: SilexError| {}),
            )
            .expect("cleanup should register");
    });
    root.dispose().expect_err("cleanup should fail")
}

#[test]
fn app_host_error_keeps_mount_and_dispose_reports() {
    let report = CleanupReport::from_parts(
        vec![CleanupFailure::new(CleanupOrigin::Root, cleanup_error())],
        vec![SilexError::Framework("boundary failure".to_string())],
    );
    let mount = AppHostError::Mount(MountError::new(
        SilexError::Framework("mount failure".to_string()),
        report,
    ));

    let mount_error = mount
        .mount_error()
        .expect("mount error should be available");
    assert_eq!(
        mount_error.primary().to_string(),
        "Framework Error: mount failure"
    );
    assert!(!mount_error.rollback().is_clean());

    let report = match mount {
        AppHostError::Mount(error) => error.into_parts().1,
        _ => unreachable!("expected mount error"),
    };
    let dispose = AppHostError::Dispose(DisposeError::new(report));
    let dispose_error = dispose
        .dispose_error()
        .expect("dispose error should be available");
    assert!(!dispose_error.report().is_clean());
}
