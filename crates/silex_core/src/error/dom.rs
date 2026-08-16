use super::SilexError;
use silex_reactivity::{CleanupDiagnostic, CloseError};
use std::{fmt, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOrigin {
    Root,
    ProvisionalOwner,
    MountBoundary,
}

#[derive(Clone, Debug)]
pub struct CleanupFailure {
    pub origin: CleanupOrigin,
    pub error: CloseError,
}

impl CleanupFailure {
    pub fn new(origin: CleanupOrigin, error: CloseError) -> Self {
        Self { origin, error }
    }

    pub fn into_parts(self) -> (CleanupOrigin, CloseError) {
        (self.origin, self.error)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CleanupReport {
    cleanup_failures: Vec<CleanupFailure>,
    boundary_errors: Vec<SilexError>,
}

/// Structured result of a failed mount rollback.
#[derive(Clone, Debug)]
pub struct RollbackError {
    primary: SilexError,
    cleanup_failures: Vec<CleanupFailure>,
    boundary_failures: Vec<SilexError>,
}

impl RollbackError {
    pub fn new(primary: SilexError, report: CleanupReport) -> Self {
        let (cleanup_failures, boundary_failures) = report.into_parts();
        Self {
            primary,
            cleanup_failures,
            boundary_failures,
        }
    }

    pub fn primary(&self) -> &SilexError {
        &self.primary
    }

    pub fn cleanup_failures(&self) -> &[CleanupFailure] {
        &self.cleanup_failures
    }

    pub fn boundary_failures(&self) -> &[SilexError] {
        &self.boundary_failures
    }

    pub fn into_parts(self) -> (SilexError, Vec<CleanupFailure>, Vec<SilexError>) {
        (self.primary, self.cleanup_failures, self.boundary_failures)
    }
}

impl CleanupReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(
        cleanup_failures: Vec<CleanupFailure>,
        boundary_errors: Vec<SilexError>,
    ) -> Self {
        Self {
            cleanup_failures,
            boundary_errors,
        }
    }

    pub fn is_clean(&self) -> bool {
        self.cleanup_failures.is_empty() && self.boundary_errors.is_empty()
    }

    pub fn cleanup_failures(&self) -> &[CleanupFailure] {
        &self.cleanup_failures
    }
    pub fn boundary_errors(&self) -> &[SilexError] {
        &self.boundary_errors
    }

    pub fn into_parts(self) -> (Vec<CleanupFailure>, Vec<SilexError>) {
        (self.cleanup_failures, self.boundary_errors)
    }
}

#[derive(Debug)]
struct MountErrorInner {
    primary: SilexError,
    rollback: CleanupReport,
    availability: MountAvailability,
}

#[derive(Debug)]
pub struct MountError {
    inner: Box<MountErrorInner>,
}

impl MountError {
    pub fn new(primary: SilexError, rollback: CleanupReport) -> Self {
        let availability = MountAvailability::from_report(&rollback);
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback,
                availability,
            }),
        }
    }

    pub fn poisoned(primary: SilexError) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback: CleanupReport::new(),
                availability: MountAvailability::Poisoned,
            }),
        }
    }

    /// Create a terminal mount error while retaining the rollback report.
    pub fn poisoned_with_report(primary: SilexError, rollback: CleanupReport) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback,
                availability: MountAvailability::Poisoned,
            }),
        }
    }

    pub fn primary(&self) -> &SilexError {
        &self.inner.primary
    }
    pub fn rollback(&self) -> &CleanupReport {
        &self.inner.rollback
    }

    pub fn rollback_error(&self) -> RollbackError {
        RollbackError::new(self.primary().clone(), self.rollback().clone())
    }
    pub fn availability(&self) -> MountAvailability {
        self.inner.availability
    }
    pub fn can_retry(&self) -> bool {
        self.availability() == MountAvailability::Retryable
    }
    pub fn is_poisoned(&self) -> bool {
        self.availability() == MountAvailability::Poisoned
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport, MountAvailability) {
        let inner = *self.inner;
        (inner.primary, inner.rollback, inner.availability)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountAvailability {
    Retryable,
    Poisoned,
}

impl MountAvailability {
    fn from_report(report: &CleanupReport) -> Self {
        if report.is_clean() {
            Self::Retryable
        } else {
            Self::Poisoned
        }
    }
}

impl fmt::Display for MountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "application mount failed: {}",
            self.inner.primary
        )
    }
}
impl std::error::Error for MountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner.primary)
    }
}

#[derive(Debug)]
pub struct DisposeError {
    report: CleanupReport,
}

impl DisposeError {
    pub fn new(report: CleanupReport) -> Self {
        Self { report }
    }
    pub fn report(&self) -> &CleanupReport {
        &self.report
    }
    pub fn into_parts(self) -> CleanupReport {
        self.report
    }
}
impl fmt::Display for DisposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("application disposal failed")
    }
}
impl std::error::Error for DisposeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupFailureDiagnostic {
    origin: CleanupOrigin,
    diagnostic: CleanupDiagnostic,
}
impl CleanupFailureDiagnostic {
    pub fn new(origin: CleanupOrigin, diagnostic: CleanupDiagnostic) -> Self {
        Self { origin, diagnostic }
    }
    pub fn origin(&self) -> CleanupOrigin {
        self.origin
    }
    pub fn diagnostic(&self) -> &CleanupDiagnostic {
        &self.diagnostic
    }
    pub fn into_parts(self) -> (CleanupOrigin, CleanupDiagnostic) {
        (self.origin, self.diagnostic)
    }
}

#[derive(Debug, Default)]
pub struct DropFailureReport {
    cleanup: Vec<CleanupFailureDiagnostic>,
    boundary: Vec<SilexError>,
}
impl DropFailureReport {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_parts(cleanup: Vec<CleanupFailureDiagnostic>, boundary: Vec<SilexError>) -> Self {
        Self { cleanup, boundary }
    }
    pub fn is_clean(&self) -> bool {
        self.cleanup.is_empty() && self.boundary.is_empty()
    }
    pub fn cleanup_failures(&self) -> &[CleanupFailureDiagnostic] {
        &self.cleanup
    }
    pub fn boundary_errors(&self) -> &[SilexError] {
        &self.boundary
    }
    pub fn into_parts(self) -> (Vec<CleanupFailureDiagnostic>, Vec<SilexError>) {
        (self.cleanup, self.boundary)
    }
}

#[derive(Clone)]
pub struct CleanupSink {
    callback: Rc<dyn Fn(DropFailureReport)>,
}
impl CleanupSink {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(DropFailureReport) + 'static,
    {
        Self {
            callback: Rc::new(handler),
        }
    }

    pub fn console() -> Self {
        Self::new(|report| {
            super::super::log::console_error(format!("Silex cleanup failure: {report:?}"));
        })
    }

    pub fn record(&self, report: DropFailureReport) {
        (self.callback)(report);
    }
}
