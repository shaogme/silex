//! Application-level DOM mount transaction and ownership contracts.

use crate::{
    attribute::AttrOp,
    view::{
        CleanupReporter, MountAncestry, MountContext as ViewMountContext, MountInstance,
        MountOwnerToken, MountTarget, MountTransaction, View,
    },
};
use silex_core::{
    CloseError, ErrorHandlerInput, OwnerAccess, OwnerHandle, Runtime, SilexError, SilexErrorKind,
    SilexResult, log::console_error,
};
use std::{
    any::Any,
    cell::RefCell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use web_sys::Node;

pub use silex_core::{
    CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink,
    DisposeError, DropFailureReport, MountAvailability, MountError, RollbackError,
};

/// The owner-bound context exposed to an application mount builder.
///
/// A builder receives this context for the duration of one root owner borrow.
/// It can mount multiple views, but cannot return the context, an owner-bound
/// view, or an owner-bound error handler from the callback.
pub struct MountContext<'scope> {
    access: OwnerAccess<'scope>,
    owner: MountOwnerToken<'scope>,
    parent: Node,
    transaction: MountTransaction<'scope>,
}

impl<'scope> MountContext<'scope> {
    fn with_cleanup_failures(
        access: OwnerAccess<'scope>,
        parent: Node,
        cleanup_failures: Rc<RefCell<Vec<CleanupFailure>>>,
        transaction: MountTransaction<'scope>,
    ) -> Self {
        let failures_for_reporter = cleanup_failures.clone();
        let cleanup_reporter: CleanupReporter = Rc::new(move |error| {
            failures_for_reporter
                .borrow_mut()
                .push(CleanupFailure::new(CleanupOrigin::ProvisionalOwner, error));
        });
        let owner = MountOwnerToken::with_cleanup_reporter(access, cleanup_reporter);
        Self {
            access,
            owner,
            parent,
            transaction,
        }
    }

    /// Borrow the explicit owner capability used by this transaction.
    pub fn access(&self) -> OwnerAccess<'scope> {
        self.access
    }

    /// Return the detached staging parent for advanced view adapters.
    pub fn parent(&self) -> &Node {
        &self.parent
    }

    /// Return the owner capability for this mount transaction.
    pub fn owner(&self) -> MountOwnerToken<'scope> {
        self.owner.clone()
    }

    /// Mount one owned view into the transaction staging parent.
    pub fn mount<V, H>(&self, view: V, error_handler: H) -> SilexResult<()>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.mount_with_attributes(view, Vec::new(), error_handler)
    }

    /// 挂载一个工厂并返回这次挂载产生的物理实例。
    pub fn mount_instance<V, H>(
        &self,
        view: V,
        error_handler: H,
    ) -> SilexResult<MountInstance<'scope>>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.mount_instance_with_attributes(view, Vec::new(), error_handler)
    }

    /// Mount one owned view with top-level pending attributes.
    pub fn mount_with_attributes<V, H>(
        &self,
        view: V,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: H,
    ) -> SilexResult<()>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.mount_instance_with_attributes(view, attrs, error_handler)
            .map(|_| ())
    }

    /// 带顶层属性地创建一次独立挂载实例。
    pub fn mount_instance_with_attributes<V, H>(
        &self,
        view: V,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: H,
    ) -> SilexResult<MountInstance<'scope>>
    where
        V: View<'scope> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let owner = self.owner();
        let context = ViewMountContext::new(
            MountTarget::Append(self.parent.clone()),
            MountAncestry::root(),
            owner,
            self.transaction.clone(),
            error_handler.handler_ref(),
        );
        view.mount(&context, attrs)
    }
}

struct MountBoundary {
    host: Node,
    staging: Node,
    start: Node,
    end: Option<Node>,
    owned_nodes: Vec<Node>,
    committed: bool,
}

impl MountBoundary {
    fn new(host: Node) -> SilexResult<Self> {
        let document = crate::document();
        let staging: Node = document.create_document_fragment().into();
        let start: Node = document.create_comment("mount-start").into();
        staging.append_child(&start).map_err(SilexError::fatal)?;
        Ok(Self {
            host,
            staging,
            start,
            end: None,
            owned_nodes: Vec::new(),
            committed: false,
        })
    }

    fn staging_parent(&self) -> Node {
        self.staging.clone()
    }

    fn finish_staging(&mut self) -> SilexResult<()> {
        if self.end.is_some() {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was finalized twice".to_string(),
            )));
        }
        if self.start.parent_node().as_ref() != Some(&self.staging) {
            return Err(SilexError::fatal(SilexErrorKind::Dom(
                "mount boundary start anchor was detached".to_string(),
            )));
        }
        let end: Node = crate::document().create_comment("mount-end").into();
        self.staging.append_child(&end).map_err(SilexError::fatal)?;
        self.end = Some(end);
        self.owned_nodes = Self::children_of(&self.staging);
        Ok(())
    }

    fn commit(&mut self) -> SilexResult<()> {
        if self.committed {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was committed twice".to_string(),
            )));
        }
        if self.end.is_none() {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was committed before finalization".to_string(),
            )));
        }
        self.host
            .append_child(&self.staging)
            .map_err(SilexError::fatal)?;
        self.committed = true;
        Ok(())
    }

    fn dispose(&mut self) -> Vec<SilexError> {
        let mut errors = Vec::new();
        let nodes = if self.committed {
            self.owned_nodes.clone()
        } else if self.owned_nodes.is_empty() {
            Self::children_of(&self.staging)
        } else {
            self.owned_nodes.clone()
        };
        let parent = if self.committed {
            Some(self.host.clone())
        } else {
            Some(self.staging.clone())
        };

        if let Some(parent) = parent {
            for node in nodes {
                if node.parent_node().as_ref() != Some(&parent) {
                    continue;
                }
                if let Err(error) = parent.remove_child(&node).map_err(SilexError::fatal) {
                    errors.push(error);
                }
            }
        }

        self.owned_nodes.clear();
        self.end = None;
        self.committed = false;
        errors
    }

    fn children_of(parent: &Node) -> Vec<Node> {
        let children = parent.child_nodes();
        (0..children.length())
            .filter_map(|index| children.item(index))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MountState {
    Ready,
    Mounting,
    Mounted,
    Disposing,
    Poisoned,
}

struct MountSession {
    root: OwnerHandle,
    boundary: MountBoundary,
    generation: u64,
}

/// A stable application handle that can be mounted repeatedly.
pub struct MountedApp {
    runtime: Runtime,
    host: Node,
    session: Option<MountSession>,
    cleanup_sink: CleanupSink,
    state: MountState,
    next_generation: u64,
}

impl fmt::Debug for MountedApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MountedApp")
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
    /// Create a reusable handle without allocating a root or DOM boundary.
    pub fn new(runtime: Runtime, host: Node, cleanup_sink: CleanupSink) -> Self {
        Self {
            runtime,
            host,
            session: None,
            cleanup_sink,
            state: MountState::Ready,
            next_generation: 0,
        }
    }

    /// Mount a new session, disposing an existing session before starting it.
    pub fn mount<F>(&mut self, builder: F) -> Result<(), MountError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        match self.state {
            MountState::Poisoned => return Err(Self::poisoned_mount_error()),
            MountState::Mounting | MountState::Disposing => {
                self.state = MountState::Poisoned;
                return Err(Self::poisoned_mount_error());
            }
            MountState::Ready | MountState::Mounted => {}
        }

        if let Some(session) = self.session.take() {
            self.state = MountState::Disposing;
            let mut root = Some(session.root);
            let mut boundary = Some(session.boundary);
            let report = cleanup_parts(&mut root, &mut boundary);
            if !report.is_clean() {
                self.state = MountState::Poisoned;
                return Err(MountError::new(
                    SilexError::fatal(SilexErrorKind::Framework(
                        "previous mount cleanup failed".to_string(),
                    )),
                    report,
                ));
            }
        }

        self.state = MountState::Mounting;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        let result = catch_unwind(AssertUnwindSafe(|| self.start_mount(builder, generation)));

        match result {
            Ok(Ok(session)) => {
                self.session = Some(session);
                self.state = MountState::Mounted;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = if error.can_retry() {
                    MountState::Ready
                } else {
                    MountState::Poisoned
                };
                Err(error)
            }
            Err(panic) => {
                self.state = MountState::Poisoned;
                Err(MountError::poisoned(panic_error("mount operation", panic)))
            }
        }
    }

    /// Whether the current committed session is active.
    pub fn is_active(&self) -> SilexResult<bool> {
        if self.state != MountState::Mounted {
            return Ok(false);
        }
        match self.session.as_ref() {
            Some(session) => session.root.is_active(),
            None => Err(SilexError::fatal(SilexErrorKind::Framework(
                "mounted state has no active session".to_string(),
            ))),
        }
    }

    /// Whether this handle can no longer create a session.
    pub fn is_poisoned(&self) -> bool {
        self.state == MountState::Poisoned
    }

    /// Return the caller-supplied host node, even while no session is active.
    pub fn host(&self) -> Node {
        self.host.clone()
    }

    /// Dispose the current session. A clean or already-ready handle remains reusable.
    pub fn dispose(&mut self) -> Result<(), DisposeError> {
        if self.state == MountState::Poisoned {
            return Ok(());
        }
        if self.state == MountState::Mounting || self.state == MountState::Disposing {
            self.state = MountState::Poisoned;
            return Ok(());
        }

        let Some(session) = self.session.take() else {
            self.state = MountState::Ready;
            return Ok(());
        };

        self.state = MountState::Disposing;
        let mut root = Some(session.root);
        let mut boundary = Some(session.boundary);
        let report = cleanup_parts(&mut root, &mut boundary);
        if report.is_clean() {
            self.state = MountState::Ready;
            Ok(())
        } else {
            self.state = MountState::Poisoned;
            Err(DisposeError::new(report))
        }
    }

    fn poisoned_mount_error() -> MountError {
        MountError::poisoned(SilexError::fatal(SilexErrorKind::Framework(
            "mounted app handle is poisoned".to_string(),
        )))
    }

    fn start_mount<F>(&mut self, builder: F, generation: u64) -> Result<MountSession, MountError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        let root = match self.runtime.owner() {
            Ok(root) => root,
            Err(primary) => return Err(MountError::new(primary, CleanupReport::new())),
        };
        let provisional_failures = Rc::new(RefCell::new(Vec::new()));
        let mut attempt = MountAttempt::new(
            root,
            self.cleanup_sink.clone(),
            provisional_failures,
            generation,
        );
        match MountBoundary::new(self.host.clone()) {
            Ok(boundary) => {
                attempt.boundary = Some(boundary);
                attempt.run(builder)
            }
            Err(primary) => attempt.fail(primary, Vec::new()),
        }
    }
}

impl Drop for MountedApp {
    fn drop(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };

        let mut root = Some(session.root);
        let mut boundary = Some(session.boundary);
        let report = cleanup_parts(&mut root, &mut boundary);
        record_drop_report(&self.cleanup_sink, report);
    }
}

fn cleanup_parts(
    root: &mut Option<OwnerHandle>,
    boundary: &mut Option<MountBoundary>,
) -> CleanupReport {
    let mut cleanup_failures = Vec::new();

    if let Some(root) = root.take()
        && let Some(failure) = close_root_safely(root)
    {
        cleanup_failures.push(failure);
    }

    let boundary_errors = if let Some(mut boundary) = boundary.take() {
        match catch_unwind(AssertUnwindSafe(|| boundary.dispose())) {
            Ok(errors) => errors,
            Err(panic) => {
                cleanup_failures.push(CleanupFailure::new(
                    CleanupOrigin::MountBoundary,
                    CloseError::from_panic(panic),
                ));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    CleanupReport::from_parts(cleanup_failures, boundary_errors)
}

fn close_root_safely(root: OwnerHandle) -> Option<CleanupFailure> {
    match catch_unwind(AssertUnwindSafe(|| root.close())) {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(CleanupFailure::new(CleanupOrigin::Root, error)),
        Err(panic) => Some(CleanupFailure::new(
            CleanupOrigin::Root,
            CloseError::from_panic(panic),
        )),
    }
}

fn panic_error(operation: &str, panic: Box<dyn Any + Send>) -> SilexError {
    let close_error = CloseError::from_panic(panic);
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "{operation} panicked: {}",
        close_error.diagnostic().message()
    )))
}

fn record_drop_report(sink: &CleanupSink, report: CleanupReport) {
    if report.is_clean() {
        return;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let (cleanup_failures, boundary_errors) = report.into_parts();
        let cleanup_failures = cleanup_failures
            .into_iter()
            .map(|failure| {
                let (origin, error) = failure.into_parts();
                CleanupFailureDiagnostic::new(origin, error.into_diagnostic())
            })
            .collect();
        sink.record(DropFailureReport::from_parts(
            cleanup_failures,
            boundary_errors,
        ));
    }));

    if result.is_err() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            console_error("Silex cleanup sink panicked");
        }));
    }
}

struct MountAttempt {
    root: Option<OwnerHandle>,
    boundary: Option<MountBoundary>,
    cleanup_sink: CleanupSink,
    provisional_failures: Rc<RefCell<Vec<CleanupFailure>>>,
    generation: u64,
}

impl MountAttempt {
    fn new(
        root: OwnerHandle,
        cleanup_sink: CleanupSink,
        provisional_failures: Rc<RefCell<Vec<CleanupFailure>>>,
        generation: u64,
    ) -> Self {
        Self {
            root: Some(root),
            boundary: None,
            cleanup_sink,
            provisional_failures,
            generation,
        }
    }

    fn run<F>(mut self, builder: F) -> Result<MountSession, MountError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        let root = self.root.as_ref().expect("mount attempt root must exist");
        let parent = self
            .boundary
            .as_ref()
            .expect("mount attempt boundary must exist")
            .staging_parent();
        let mount_result = catch_unwind(AssertUnwindSafe(|| {
            root.with_access(|access| {
                let transaction = MountTransaction::new();
                let ctx = MountContext::with_cleanup_failures(
                    access,
                    parent,
                    self.provisional_failures.clone(),
                    transaction.clone(),
                );
                match builder(&ctx) {
                    Ok(()) => {
                        if let Err(error) = self
                            .boundary
                            .as_mut()
                            .expect("mount attempt boundary must exist")
                            .finish_staging()
                        {
                            let _ = transaction.rollback();
                            return Err(error);
                        }
                        if let Err(error) = self
                            .boundary
                            .as_mut()
                            .expect("mount attempt boundary must exist")
                            .commit()
                        {
                            let _ = transaction.rollback();
                            return Err(error);
                        }
                        let _ = transaction.commit();
                        Ok(())
                    }
                    Err(error) => {
                        let _ = transaction.rollback();
                        Err(error)
                    }
                }
            })
        }));
        let provisional_failures = self.take_provisional_failures();

        match mount_result {
            Ok(Ok(())) => self.publish(),
            Ok(Err(primary)) => self.fail(primary, provisional_failures),
            Err(panic) => {
                self.fail_panic(panic_error("mount builder", panic), provisional_failures)
            }
        }
    }

    fn fail(
        mut self,
        primary: SilexError,
        provisional_failures: Vec<CleanupFailure>,
    ) -> Result<MountSession, MountError> {
        let rollback = self.abort(provisional_failures);
        Err(MountError::new(primary, rollback))
    }

    fn fail_panic(
        mut self,
        primary: SilexError,
        provisional_failures: Vec<CleanupFailure>,
    ) -> Result<MountSession, MountError> {
        let rollback = self.abort(provisional_failures);
        Err(MountError::poisoned_with_report(primary, rollback))
    }

    fn abort(&mut self, provisional_failures: Vec<CleanupFailure>) -> CleanupReport {
        let report = cleanup_parts(&mut self.root, &mut self.boundary);
        let (mut cleanup_failures, boundary_errors) = report.into_parts();
        cleanup_failures.extend(provisional_failures);
        CleanupReport::from_parts(cleanup_failures, boundary_errors)
    }

    fn publish(mut self) -> Result<MountSession, MountError> {
        Ok(MountSession {
            root: self.root.take().expect("published attempt root must exist"),
            boundary: self
                .boundary
                .take()
                .expect("published attempt boundary must exist"),
            generation: self.generation,
        })
    }

    fn take_provisional_failures(&self) -> Vec<CleanupFailure> {
        std::mem::take(&mut *self.provisional_failures.borrow_mut())
    }
}

impl Drop for MountAttempt {
    fn drop(&mut self) {
        if self.root.is_none() && self.boundary.is_none() {
            return;
        }
        let provisional_failures = self.take_provisional_failures();
        let report = self.abort(provisional_failures);
        record_drop_report(&self.cleanup_sink, report);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn root_with_cleanup_panic(message: &'static str) -> OwnerHandle {
        let mut runtime = Runtime::new();
        let root = runtime.owner().expect("root should be created");
        root.with_access(|owner| {
            let handler = owner
                .error_handler(|_: SilexError| {})
                .expect("error handler should register");
            owner
                .on_cleanup(
                    move || -> SilexResult<()> { panic!("{message}") },
                    handler.view(),
                )
                .expect("cleanup should register");
        });
        root
    }

    #[test]
    fn cleanup_parts_preserves_root_cleanup_error() {
        let mut root = Some(root_with_cleanup_panic("mounted root cleanup"));
        let mut boundary = None;
        let report = cleanup_parts(&mut root, &mut boundary);

        assert!(root.is_none());
        assert!(boundary.is_none());
        assert_eq!(report.cleanup_failures().len(), 1);
        assert_eq!(report.cleanup_failures()[0].origin, CleanupOrigin::Root);
        assert_eq!(
            report.cleanup_failures()[0].error.diagnostic().message(),
            "mounted root cleanup"
        );
        assert!(report.boundary_errors().is_empty());
    }

    #[test]
    fn drop_sink_panic_is_contained_after_diagnostic_conversion() {
        let mut root = Some(root_with_cleanup_panic("drop root cleanup"));
        let mut boundary = None;
        let report = cleanup_parts(&mut root, &mut boundary);
        let sink = CleanupSink::new(|_| panic!("sink failure"));

        let result = catch_unwind(AssertUnwindSafe(|| record_drop_report(&sink, report)));
        assert!(result.is_ok());
    }
}
