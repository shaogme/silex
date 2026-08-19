//! Long-lived root owner and close error handling.

use crate::{HandlerError, ReactiveError};
use std::{
    any::Any,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Identifies the panic payload shape preserved by a cleanup diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupPayloadKind {
    /// The payload was an owned [`String`].
    String,
    /// The payload was a string literal or another `&'static str`.
    StaticStr,
    /// The payload was not one of the safely inspectable string forms.
    Unknown,
}

/// Stable, owned information about a cleanup panic.
///
/// The diagnostic deliberately does not expose the original panic payload. The
/// payload remains owned by [`CloseError`] until the explicit error path
/// consumes it, while Drop-only paths can safely retain this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupDiagnostic {
    message: String,
    payload_kind: CleanupPayloadKind,
}

impl CleanupDiagnostic {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn payload_kind(&self) -> CleanupPayloadKind {
        self.payload_kind
    }
}

fn diagnostic_for(panic: &(dyn Any + Send)) -> CleanupDiagnostic {
    if let Some(message) = panic.downcast_ref::<String>() {
        return CleanupDiagnostic {
            message: message.clone(),
            payload_kind: CleanupPayloadKind::String,
        };
    }

    if let Some(message) = panic.downcast_ref::<&'static str>() {
        return CleanupDiagnostic {
            message: (*message).to_string(),
            payload_kind: CleanupPayloadKind::StaticStr,
        };
    }

    CleanupDiagnostic {
        message: "unknown cleanup panic payload".to_string(),
        payload_kind: CleanupPayloadKind::Unknown,
    }
}

/// A failure collected during explicit disposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CleanupFailure {
    Runtime(ReactiveError),
    Handler(HandlerError),
    Panic(CleanupDiagnostic),
}

/// The lifecycle phase that produced one close failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePhase {
    Child,
    Effect,
    Cleanup,
    Runtime,
    Boundary,
    Unknown,
}

/// The lifecycle source associated with one close failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseSource {
    Owner,
    Child,
    Effect,
    Cleanup,
    Handler,
    Boundary,
    Unknown,
}

/// A close failure with stable phase and source metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseFailure {
    phase: ClosePhase,
    source: CloseSource,
    failure: CleanupFailure,
}

impl CloseFailure {
    pub fn phase(&self) -> ClosePhase {
        self.phase
    }

    pub fn source(&self) -> CloseSource {
        self.source
    }

    pub fn failure(&self) -> &CleanupFailure {
        &self.failure
    }
}

/// Cleanup failures returned by an explicit root or owner disposal.
#[derive(Clone, PartialEq, Eq)]
pub struct CloseError {
    failures: Vec<CleanupFailure>,
    entries: Vec<CloseFailure>,
    diagnostic: CleanupDiagnostic,
}

/// Failure returned after a transient scope callback or its close operation.
///
/// Runtime setup failures and owner close failures have different recovery
/// semantics. Keeping them as separate variants prevents a close diagnostic
/// from being misreported as a dynamic borrow conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransientScopeError {
    Runtime(ReactiveError),
    Close(CloseError),
}

impl fmt::Display for TransientScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "transient scope runtime error: {error}"),
            Self::Close(error) => {
                write!(formatter, "transient scope close error: {error:?}")
            }
        }
    }
}

impl std::error::Error for TransientScopeError {}

/// Result returned by transient scope execution.
pub type TransientScopeResult<T> = Result<T, TransientScopeError>;

/// Aggregated error returned by every explicit owner close operation.
impl fmt::Debug for CloseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloseError").finish_non_exhaustive()
    }
}

impl CloseError {
    fn new(panic: Box<dyn std::any::Any + Send>) -> Self {
        let diagnostic = diagnostic_for(panic.as_ref());
        let _ = catch_unwind(AssertUnwindSafe(|| drop(panic)));
        Self {
            failures: vec![CleanupFailure::Panic(diagnostic.clone())],
            entries: vec![CloseFailure {
                phase: ClosePhase::Unknown,
                source: CloseSource::Unknown,
                failure: CleanupFailure::Panic(diagnostic.clone()),
            }],
            diagnostic,
        }
    }

    pub(crate) fn from_failures(failures: Vec<CleanupFailure>) -> Option<Self> {
        if failures.is_empty() {
            return None;
        }
        let entries = failures
            .iter()
            .cloned()
            .map(|failure| CloseFailure {
                phase: ClosePhase::Unknown,
                source: CloseSource::Unknown,
                failure,
            })
            .collect();
        Self::from_entries(entries)
    }

    fn from_entries(entries: Vec<CloseFailure>) -> Option<Self> {
        if entries.is_empty() {
            return None;
        }
        let failures = entries
            .iter()
            .map(|entry| entry.failure.clone())
            .collect::<Vec<_>>();
        let diagnostic = failures
            .iter()
            .find_map(|failure| match failure {
                CleanupFailure::Panic(diagnostic) => Some(diagnostic.clone()),
                CleanupFailure::Runtime(_) | CleanupFailure::Handler(_) => None,
            })
            .unwrap_or(CleanupDiagnostic {
                message: "runtime cleanup failure".to_string(),
                payload_kind: CleanupPayloadKind::Unknown,
            });
        Some(Self {
            failures,
            entries,
            diagnostic,
        })
    }

    /// Combine failures collected by independent cleanup phases.
    ///
    /// Close paths must be able to finish child, effect, and cleanup work even
    /// when one phase fails. Keeping this operation on `CloseError` preserves
    /// every failure in one structured value instead of forcing callers to
    /// resume a panic or report only the first error.
    #[doc(hidden)]
    pub fn combine(errors: impl IntoIterator<Item = Self>) -> Option<Self> {
        let entries = errors.into_iter().flat_map(|error| error.entries).collect();
        Self::from_entries(entries)
    }

    /// Apply the phase and source of an outer close transaction to all
    /// failures in this error.
    #[doc(hidden)]
    pub fn with_context(self, phase: ClosePhase, source: CloseSource) -> Self {
        let entries = self
            .entries
            .into_iter()
            .map(|entry| CloseFailure {
                phase,
                source,
                failure: entry.failure,
            })
            .collect();
        Self::from_entries(entries).expect("a close error must contain a failure")
    }

    pub(crate) fn panic_failure(panic: Box<dyn std::any::Any + Send>) -> CleanupFailure {
        let diagnostic = diagnostic_for(panic.as_ref());
        let _ = catch_unwind(AssertUnwindSafe(|| drop(panic)));
        CleanupFailure::Panic(diagnostic)
    }

    pub fn failures(&self) -> &[CleanupFailure] {
        &self.failures
    }

    pub fn entries(&self) -> &[CloseFailure] {
        &self.entries
    }

    /// Adapt a caught framework panic into a close error.
    ///
    /// This is hidden from generated documentation because it is intended for
    /// framework cleanup adapters, not as a replacement for ordinary errors.
    #[doc(hidden)]
    pub fn from_panic(panic: Box<dyn std::any::Any + Send>) -> Self {
        Self::new(panic)
    }

    /// Borrow the stable diagnostic without consuming the original error.
    pub fn diagnostic(&self) -> &CleanupDiagnostic {
        &self.diagnostic
    }

    /// Consume the error and return only its stable, owned diagnostic.
    pub fn into_diagnostic(self) -> CleanupDiagnostic {
        self.diagnostic
    }
}

/// Collects close failures while allowing every lifecycle phase to finish.
#[derive(Default)]
pub struct CloseTransaction {
    entries: Vec<CloseFailure>,
}

impl CloseTransaction {
    pub fn new() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn push(&mut self, phase: ClosePhase, source: CloseSource, failure: CleanupFailure) {
        self.entries.push(CloseFailure {
            phase,
            source,
            failure,
        });
    }

    #[doc(hidden)]
    pub fn push_error(&mut self, phase: ClosePhase, source: CloseSource, error: CloseError) {
        self.entries
            .extend(error.entries.into_iter().map(|entry| CloseFailure {
                phase,
                source,
                failure: entry.failure,
            }));
    }

    pub fn finish(self) -> Option<CloseError> {
        CloseError::from_entries(self.entries)
    }
}
