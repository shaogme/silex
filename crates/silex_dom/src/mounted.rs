//! Error and ownership contracts for application-level DOM mounting.
//!
//! The transaction and boundary implementation is intentionally introduced in
//! a later phase. These types freeze the ownership model first so a future
//! `MountedApp` cannot replace an original cleanup error with a string or skip
//! a later cleanup stage.

use silex_core::{CleanupDiagnostic, CleanupError, SilexError};
use std::{fmt, rc::Rc};

/// Identifies the framework boundary that produced a cleanup failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOrigin {
    Root,
    ProvisionalOwner,
    MountBoundary,
}

/// A cleanup failure that retains the original root cleanup error.
#[derive(Debug)]
pub struct CleanupFailure {
    pub origin: CleanupOrigin,
    pub error: CleanupError,
}

impl CleanupFailure {
    pub fn new(origin: CleanupOrigin, error: CleanupError) -> Self {
        Self { origin, error }
    }

    pub fn into_parts(self) -> (CleanupOrigin, CleanupError) {
        (self.origin, self.error)
    }
}

/// Errors observed while rolling back or disposing an application boundary.
#[derive(Debug, Default)]
pub struct CleanupReport {
    cleanup_failures: Vec<CleanupFailure>,
    boundary_errors: Vec<SilexError>,
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

/// The primary mount error and every error observed while rolling it back.
#[derive(Debug)]
pub struct MountError {
    primary: SilexError,
    rollback: CleanupReport,
}

impl MountError {
    pub fn new(primary: SilexError, rollback: CleanupReport) -> Self {
        Self { primary, rollback }
    }

    pub fn primary(&self) -> &SilexError {
        &self.primary
    }

    pub fn rollback(&self) -> &CleanupReport {
        &self.rollback
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport) {
        (self.primary, self.rollback)
    }
}

impl fmt::Display for MountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application mount failed: {}", self.primary)
    }
}

impl std::error::Error for MountError {}

/// Errors observed while explicitly disposing a mounted application.
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("application disposal failed")
    }
}

impl std::error::Error for DisposeError {}

/// A cleanup failure converted to stable data for a Drop-only reporting path.
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

/// Stable diagnostics that cannot be returned from a Drop implementation.
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

/// An owned, scope-independent destination for Drop cleanup diagnostics.
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

    /// Create an explicit console/stderr diagnostic adapter.
    pub fn console() -> Self {
        Self::new(|report| {
            silex_core::log::console_error(format!("Silex cleanup failure: {report:?}"));
        })
    }

    pub fn record(&self, report: DropFailureReport) {
        (self.callback)(report);
    }
}
