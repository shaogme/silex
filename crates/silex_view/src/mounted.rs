//! 应用级 mount 生命周期。

use crate::context::{MountAncestry, MountContext, MountDomAction, MountTarget, MountTransaction};
use crate::contract::{MountInstance, View};
use crate::error::MountError;
use crate::owner::{CleanupReporter, MountOwnerToken};
use silex_core::{
    CloseError, DisposeError, ErrorHandlerInput, OwnerAccess, OwnerHandle, Runtime, SilexError,
    SilexErrorKind, SilexResult,
};
use silex_dom::lifecycle::{
    CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink,
    DropFailureReport,
};
use silex_dom::{diagnostics::logging::console_error, model::DomNode, runtime::DomContext};
use std::cell::RefCell;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

/// 应用 builder 使用的 context；它不等同于 View kernel 的 `MountContext`。
pub struct MountBuilderContext<'scope> {
    access: OwnerAccess<'scope>,
    dom: DomContext,
    parent: DomNode,
    owner: MountOwnerToken<'scope>,
    transaction: MountTransaction<'scope>,
}

impl<'scope> MountBuilderContext<'scope> {
    pub fn access(&self) -> OwnerAccess<'scope> {
        self.access
    }

    pub fn dom(&self) -> &DomContext {
        &self.dom
    }

    /// Create an owner-bound DOM action for callbacks created during mount.
    pub fn dom_action(&self) -> MountDomAction<'scope> {
        MountDomAction::new(self.dom.clone(), self.owner.clone())
    }

    pub fn parent(&self) -> &DomNode {
        &self.parent
    }

    pub fn owner(&self) -> MountOwnerToken<'scope> {
        self.owner.clone()
    }

    pub fn mount<V, H>(&self, view: V, error_handler: H) -> SilexResult<MountInstance<'scope>>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let context = MountContext::new(
            MountTarget::append(self.dom.clone(), self.parent.clone()),
            MountAncestry::root(),
            self.owner.clone(),
            self.transaction.clone(),
            error_handler.handler_ref(),
        );
        context.mount(&view)
    }

    pub fn mount_unit<V, H>(&self, view: V, error_handler: H) -> SilexResult<()>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.mount(view, error_handler).map(|_| ())
    }
}

struct MountBoundary {
    dom: DomContext,
    host: DomNode,
    staging: DomNode,
    owned_nodes: Vec<DomNode>,
    finished: bool,
    committed: bool,
}

impl MountBoundary {
    fn new(dom: DomContext, host: DomNode) -> SilexResult<Self> {
        let staging = dom.create_fragment()?;
        Ok(Self {
            dom,
            host,
            staging,
            owned_nodes: Vec::new(),
            finished: false,
            committed: false,
        })
    }

    fn finish_staging(&mut self) -> SilexResult<()> {
        if self.finished {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was finalized twice".into(),
            )));
        }
        self.owned_nodes = self.dom.children(&self.staging)?;
        self.finished = true;
        Ok(())
    }

    fn commit(&mut self) -> SilexResult<()> {
        if self.committed {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was committed twice".into(),
            )));
        }
        self.dom.append(&self.host, &self.staging)?;
        self.committed = true;
        Ok(())
    }

    fn dispose(&mut self) -> Vec<SilexError> {
        let parent = if self.committed {
            self.host.clone()
        } else {
            self.staging.clone()
        };
        let mut errors = Vec::new();
        let nodes = if self.finished {
            std::mem::take(&mut self.owned_nodes)
        } else {
            self.dom.children(&self.staging).unwrap_or_default()
        };
        for node in nodes {
            match self.dom.parent(&node) {
                Ok(Some(current)) if current == parent => {
                    if let Err(error) = self.dom.remove(&node) {
                        errors.push(error.into());
                    }
                }
                Ok(_) => {}
                Err(error) => errors.push(error.into()),
            }
        }
        self.committed = false;
        self.finished = false;
        errors
    }
}

struct MountSession {
    root: OwnerHandle,
    boundary: MountBoundary,
    generation: u64,
}

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
    /// Construct an app after validating that the host belongs to `dom`.
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

    /// Construct an app while preserving the historical infallible signature.
    ///
    /// The host is still checked before every mutating mount operation; a
    /// foreign host is never silently mounted. Prefer [`Self::try_new`] when
    /// construction-time validation is available to the caller.
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
        let failures = Rc::new(RefCell::new(Vec::<CleanupFailure>::new()));
        let boundary = MountBoundary::new(self.dom.clone(), self.host.clone())
            .map_err(|error| MountError::new(error, CleanupReport::new()))?;
        let mut session = MountSession {
            root,
            boundary,
            generation,
        };
        let result = session.root.with_access(|access| {
            let failures_for_reporter = failures.clone();
            let reporter: CleanupReporter = Rc::new(move |error| {
                failures_for_reporter
                    .borrow_mut()
                    .push(CleanupFailure::new(CleanupOrigin::ProvisionalOwner, error));
            });
            let owner = MountOwnerToken::with_cleanup_reporter(access, reporter);
            let transaction = MountTransaction::new();
            let context = MountBuilderContext {
                access,
                dom: self.dom.clone(),
                parent: session.boundary.staging.clone(),
                owner,
                transaction: transaction.clone(),
            };
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

fn cleanup_session(session: &mut MountSession) -> CleanupReport {
    let mut cleanup_failures = Vec::new();
    match catch_unwind(AssertUnwindSafe(|| session.root.close())) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => cleanup_failures.push(CleanupFailure::new(CleanupOrigin::Root, error)),
        Err(panic) => cleanup_failures.push(CleanupFailure::new(
            CleanupOrigin::Root,
            CloseError::from_panic(panic),
        )),
    }
    let boundary_errors = catch_unwind(AssertUnwindSafe(|| session.boundary.dispose()))
        .unwrap_or_else(|panic| vec![panic_error("mount boundary cleanup", panic)]);
    CleanupReport::from_parts(cleanup_failures, boundary_errors)
}

fn poisoned_error() -> MountError {
    MountError::poisoned(SilexError::fatal(SilexErrorKind::Framework(
        "mounted app handle is poisoned".into(),
    )))
}

fn panic_error(operation: &str, panic: Box<dyn std::any::Any + Send>) -> SilexError {
    let error = CloseError::from_panic(panic);
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "{operation} panicked: {}",
        error.diagnostic().message()
    )))
}
