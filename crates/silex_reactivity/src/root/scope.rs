//! Long-lived root owner and cleanup error handling.

use crate::{Scope, runtime::GlobalScheduler, scope::ScopeStorage};
use std::{
    any::Any,
    cell::Cell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
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
/// payload remains owned by [`CleanupError`] until the explicit error path
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

/// A cleanup failure returned by an explicit root disposal.
pub struct CleanupError {
    panic: Box<dyn std::any::Any + Send>,
    diagnostic: CleanupDiagnostic,
}

impl fmt::Debug for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CleanupError").finish_non_exhaustive()
    }
}

impl CleanupError {
    fn new(panic: Box<dyn std::any::Any + Send>) -> Self {
        let diagnostic = diagnostic_for(panic.as_ref());
        Self { panic, diagnostic }
    }

    /// Adapt a caught framework panic into a cleanup error.
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
        let Self { panic, diagnostic } = self;
        let _ = catch_unwind(AssertUnwindSafe(|| drop(panic)));
        diagnostic
    }

    fn resume(self) -> ! {
        resume_unwind(self.panic)
    }
}

/// Owns one long-lived root storage.
pub struct RootHandle {
    storage: Rc<ScopeStorage>,
    runtime_slot: Rc<Cell<bool>>,
    disposed: bool,
}

impl RootHandle {
    pub(crate) fn new(runtime_slot: Rc<Cell<bool>>) -> Self {
        Self {
            storage: Rc::new(ScopeStorage::new(GlobalScheduler::new())),
            runtime_slot,
            disposed: false,
        }
    }

    /// Borrow the ordinary scope capability used to create root nodes.
    pub fn scope(&self) -> Scope<'_> {
        Scope {
            storage: self.storage.as_ref(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Execute a callback with a root scope borrowed for exactly this owner.
    pub fn with_scope<'scope, R>(&'scope self, f: impl FnOnce(Scope<'scope>) -> R) -> R {
        let scope = Scope {
            storage: self.storage.as_ref(),
            _marker: std::marker::PhantomData,
        };
        f(scope)
    }

    /// Dispose the root exactly once.
    pub fn dispose(mut self) -> Result<(), CleanupError> {
        self.dispose_inner()
    }

    pub fn is_active(&self) -> bool {
        !self.disposed
            && self
                .storage
                .state
                .borrow()
                .scheduler
                .borrow()
                .is_scope_active(self.storage.scope_id)
    }

    fn dispose_inner(&mut self) -> Result<(), CleanupError> {
        if self.disposed {
            self.runtime_slot.set(false);
            return Ok(());
        }

        self.disposed = true;
        self.runtime_slot.set(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.storage.dispose_untracked();
        }));

        match result {
            Ok(()) => Ok(()),
            Err(panic) => Err(CleanupError::new(panic)),
        }
    }
}

impl Drop for RootHandle {
    fn drop(&mut self) {
        if let Err(error) = self.dispose_inner() {
            error.resume();
        }
    }
}
