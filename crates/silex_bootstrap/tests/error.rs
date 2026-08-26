use silex_bootstrap::AppHostError;
use silex_core::{BootstrapError, CloseError, Runtime, SilexError, SilexErrorKind};
use silex_dom::lifecycle::{CleanupFailure as DomCleanupFailure, CleanupOrigin, CleanupReport};
use silex_view::errors::{DisposeError, MountError};

fn cleanup_error() -> CloseError {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");
    root.with_access(|owner| {
        owner
            .on_cleanup(
                || panic!("host cleanup failure"),
                owner
                    .error_handler(|_: SilexError| {})
                    .expect("cleanup error handler should be registered"),
            )
            .expect("cleanup should register");
    });
    root.close().expect_err("cleanup should fail")
}

#[test]
fn app_host_error_keeps_mount_and_dispose_reports() {
    let report = CleanupReport::from_parts(
        vec![DomCleanupFailure::new(CleanupOrigin::Root, cleanup_error())],
        vec![SilexError::recoverable(SilexErrorKind::Framework(
            "boundary failure".to_string(),
        ))],
    );
    let mount = AppHostError::Mount(Box::new(MountError::new(
        SilexError::recoverable(SilexErrorKind::Framework("mount failure".to_string())),
        report,
    )));

    let unified = SilexError::from(mount.clone());
    assert!(matches!(
        unified.kind(),
        SilexErrorKind::Bootstrap(error)
            if matches!(error.as_ref(), BootstrapError::Host(host)
                if matches!(host.as_ref(), AppHostError::Mount(_)))
    ));

    let mount_error = mount
        .mount_error()
        .expect("mount error should be available");
    assert_eq!(
        mount_error.primary().to_string(),
        "Recoverable: Framework Error: mount failure"
    );
    assert!(!mount_error.rollback().is_clean());

    let report = match mount {
        AppHostError::Mount(error) => error.into_parts().1,
        _ => unreachable!("expected mount error"),
    };
    let dispose = AppHostError::Dispose(Box::new(DisposeError::new(report)));
    let dispose_error = dispose
        .dispose_error()
        .expect("dispose error should be available");
    assert!(!dispose_error.report().is_clean());
}
