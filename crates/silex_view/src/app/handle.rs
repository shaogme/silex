use super::boundary::{
    MountBoundary, MountSession, cleanup_reporter, cleanup_session, new_builder_context,
    panic_error,
};
use crate::app::MountBuilderContext;
use crate::errors::MountError;
use crate::kernel::MountTransaction;
use crate::lifecycle::MountOwnerToken;
use silex_core::{DisposeError, Runtime, SilexError, SilexErrorKind, SilexResult};
use silex_dom::lifecycle::{
    CleanupFailureDiagnostic, CleanupReport, CleanupSink, DropFailureReport,
};
use silex_dom::{diagnostics::logging::console_error, model::DomNode, runtime::DomContext};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MountedState {
    Ready,
    Mounting,
    Mounted,
    Disposing,
    Poisoned,
}

/// 可重复 mount 的应用句柄。
pub struct MountedApp {
    runtime: Runtime,
    dom: DomContext,
    host: DomNode,
    session: Option<MountSession>,
    cleanup_sink: CleanupSink,
    state: MountedState,
    next_generation: u64,
}

impl fmt::Debug for MountedApp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MountedApp")
            .field("active", &self.is_active())
            .field("poisoned", &self.is_poisoned())
            .field(
                "generation",
                &self.session.as_ref().map(|session| session.generation),
            )
            .finish()
    }
}

impl MountedApp {
    pub fn try_new(
        runtime: Runtime,
        dom: DomContext,
        host: DomNode,
        cleanup_sink: CleanupSink,
    ) -> SilexResult<Self> {
        let app = Self::new(runtime, dom, host, cleanup_sink);
        app.validate_host()?;
        Ok(app)
    }

    pub fn new(
        runtime: Runtime,
        dom: DomContext,
        host: DomNode,
        cleanup_sink: CleanupSink,
    ) -> Self {
        Self {
            runtime,
            dom,
            host,
            session: None,
            cleanup_sink,
            state: MountedState::Ready,
            next_generation: 0,
        }
    }

    pub fn mount<F>(&mut self, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        self.validate_host()?;
        if self.state == MountedState::Poisoned {
            return Err(SilexError::from(poisoned_error()));
        }
        if matches!(self.state, MountedState::Mounting | MountedState::Disposing) {
            self.state = MountedState::Poisoned;
            return Err(SilexError::from(poisoned_error()));
        }
        if self.session.is_some() {
            self.dispose_inner().map_err(|error| {
                MountError::new(
                    SilexError::fatal(SilexErrorKind::Framework(
                        "previous mount cleanup failed".into(),
                    )),
                    error.into_parts(),
                )
            })?;
        }

        self.state = MountedState::Mounting;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        let result = catch_unwind(AssertUnwindSafe(|| self.start_mount(builder, generation)));
        match result {
            Ok(Ok(session)) => {
                self.session = Some(session);
                self.state = MountedState::Mounted;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = if error.can_retry() {
                    MountedState::Ready
                } else {
                    MountedState::Poisoned
                };
                Err(error.into())
            }
            Err(panic) => {
                self.state = MountedState::Poisoned;
                Err(MountError::poisoned(panic_error("mount operation", panic)).into())
            }
        }
    }

    pub fn is_active(&self) -> SilexResult<bool> {
        if self.state != MountedState::Mounted {
            return Ok(false);
        }
        self.session
            .as_ref()
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Framework(
                    "mounted state has no session".into(),
                ))
            })?
            .root
            .is_active()
    }

    pub fn is_poisoned(&self) -> bool {
        self.state == MountedState::Poisoned
    }

    pub fn host(&self) -> DomNode {
        self.host.clone()
    }

    pub fn dispose(&mut self) -> SilexResult<()> {
        self.dispose_inner().map_err(Into::into)
    }

    fn dispose_inner(&mut self) -> Result<(), DisposeError> {
        if self.state == MountedState::Poisoned {
            return Ok(());
        }
        let Some(mut session) = self.session.take() else {
            self.state = MountedState::Ready;
            return Ok(());
        };
        self.state = MountedState::Disposing;
        let report = cleanup_session(&mut session);
        if report.is_clean() {
            self.state = MountedState::Ready;
            Ok(())
        } else {
            self.state = MountedState::Poisoned;
            Err(DisposeError::new(report))
        }
    }

    fn start_mount<F>(&mut self, builder: F, generation: u64) -> Result<MountSession, MountError>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        let root = self
            .runtime
            .owner()
            .map_err(|error| MountError::new(error, CleanupReport::new()))?;
        let boundary = MountBoundary::new(self.dom.clone(), self.host.clone())
            .map_err(|error| MountError::new(error, CleanupReport::new()))?;
        let mut session = MountSession {
            root,
            boundary,
            generation,
        };
        let (reporter, failures) = cleanup_reporter();
        let result = session.root.with_access(|access| {
            let owner = MountOwnerToken::with_cleanup_reporter(access, reporter);
            let transaction = MountTransaction::new();
            let context = new_builder_context(
                access,
                self.dom.clone(),
                &session.boundary,
                owner,
                transaction.clone(),
            );
            match builder(&context) {
                Ok(()) => {
                    session.boundary.finish_staging()?;
                    session.boundary.commit()?;
                    transaction.commit()
                }
                Err(error) => {
                    let _ = transaction.rollback();
                    Err(error)
                }
            }
        });
        match result {
            Ok(()) => Ok(session),
            Err(primary) => {
                let report = cleanup_session(&mut session);
                let mut cleanup_failures = report.cleanup_failures().to_vec();
                cleanup_failures.extend(failures.borrow_mut().drain(..));
                Err(MountError::new(
                    primary,
                    CleanupReport::from_parts(cleanup_failures, report.boundary_errors().to_vec()),
                ))
            }
        }
    }

    fn validate_host(&self) -> SilexResult<()> {
        self.dom.validate_node(&self.host)?;
        Ok(())
    }
}

impl Drop for MountedApp {
    fn drop(&mut self) {
        let Some(mut session) = self.session.take() else {
            return;
        };
        let report = cleanup_session(&mut session);
        if report.is_clean() {
            return;
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let (cleanup, boundary) = report.into_parts();
            let cleanup = cleanup
                .into_iter()
                .map(|failure| {
                    CleanupFailureDiagnostic::new(failure.origin, failure.error.into_diagnostic())
                })
                .collect();
            self.cleanup_sink
                .record(DropFailureReport::from_parts(cleanup, boundary));
        }));
        if result.is_err() {
            console_error("Silex cleanup sink panicked");
        }
    }
}

fn poisoned_error() -> MountError {
    MountError::poisoned(SilexError::fatal(SilexErrorKind::Framework(
        "mounted app handle is poisoned".into(),
    )))
}
