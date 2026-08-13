//! Application-level DOM mount transaction and ownership contracts.

use crate::{
    attribute::PendingAttribute,
    view::{CleanupReporter, ScopedMountOwner, View},
};
use silex_core::{
    CleanupDiagnostic, CleanupError, ErrorReporter, RootHandle, Runtime, Scope, SilexError,
    SilexErrorKind, SilexResult, log::console_error,
};
use std::{
    cell::RefCell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

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
    availability: MountAvailability,
}

impl MountError {
    pub fn new(primary: SilexError, rollback: CleanupReport) -> Self {
        let availability = MountAvailability::from_report(&rollback);
        Self {
            primary,
            rollback,
            availability,
        }
    }

    fn poisoned(primary: SilexError) -> Self {
        Self {
            primary,
            rollback: CleanupReport::new(),
            availability: MountAvailability::Poisoned,
        }
    }

    pub fn primary(&self) -> &SilexError {
        &self.primary
    }

    pub fn rollback(&self) -> &CleanupReport {
        &self.rollback
    }

    pub fn availability(&self) -> MountAvailability {
        self.availability
    }

    pub fn can_retry(&self) -> bool {
        self.availability == MountAvailability::Retryable
    }

    pub fn is_poisoned(&self) -> bool {
        self.availability == MountAvailability::Poisoned
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport, MountAvailability) {
        (self.primary, self.rollback, self.availability)
    }
}

/// Describes whether the handle can start another mount after a failed attempt.
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

/// The scoped context exposed to an application mount builder.
///
/// A builder receives this context for the duration of one root-scope borrow.
/// It can mount multiple views, but cannot return the context, a scoped view, or
/// a scoped error handler from the callback.
pub struct MountContext<'scope> {
    scope: Scope<'scope>,
    parent: Node,
    cleanup_reporter: CleanupReporter,
}

impl<'scope> MountContext<'scope> {
    fn with_cleanup_failures(
        scope: Scope<'scope>,
        parent: Node,
        cleanup_failures: Rc<RefCell<Vec<CleanupFailure>>>,
    ) -> Self {
        let failures_for_reporter = cleanup_failures.clone();
        let cleanup_reporter: CleanupReporter = Rc::new(move |error| {
            failures_for_reporter
                .borrow_mut()
                .push(CleanupFailure::new(CleanupOrigin::ProvisionalOwner, error));
        });
        Self {
            scope,
            parent,
            cleanup_reporter,
        }
    }

    /// Borrow the explicit scope capability used by this transaction.
    pub fn scope(&self) -> Scope<'scope> {
        self.scope
    }

    /// Return the detached staging parent for advanced view adapters.
    pub fn parent(&self) -> &Node {
        &self.parent
    }

    /// Create an owner adapter for this mount scope.
    pub fn owner(&self) -> ScopedMountOwner<'scope> {
        ScopedMountOwner::with_cleanup_reporter(self.scope, self.cleanup_reporter.clone())
    }

    /// Mount one owned view into the transaction staging parent.
    pub fn mount<V>(&self, view: V, error_handler: ErrorReporter<'scope>) -> SilexResult<()>
    where
        V: View<'scope> + 'scope,
    {
        self.mount_with_attributes(view, Vec::new(), error_handler)
    }

    /// Mount one owned view with top-level pending attributes.
    pub fn mount_with_attributes<V>(
        &self,
        view: V,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<()>
    where
        V: View<'scope> + 'scope,
    {
        let owner = self.owner();
        view.mount_owned(&owner, &self.parent, attrs, error_handler)
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
    root: RootHandle,
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
                resume_unwind(panic)
            }
        }
    }

    /// Whether the current committed session is active.
    pub fn is_active(&self) -> bool {
        self.state == MountState::Mounted
            && self
                .session
                .as_ref()
                .is_some_and(|session| session.root.is_active())
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
        let root = match self.runtime.run() {
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
    root: &mut Option<RootHandle>,
    boundary: &mut Option<MountBoundary>,
) -> CleanupReport {
    let mut cleanup_failures = Vec::new();

    if let Some(root) = root.take()
        && let Some(failure) = dispose_root_safely(root)
    {
        cleanup_failures.push(failure);
    }

    let boundary_errors = if let Some(mut boundary) = boundary.take() {
        match catch_unwind(AssertUnwindSafe(|| boundary.dispose())) {
            Ok(errors) => errors,
            Err(panic) => {
                cleanup_failures.push(CleanupFailure::new(
                    CleanupOrigin::MountBoundary,
                    CleanupError::from_panic(panic),
                ));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    CleanupReport::from_parts(cleanup_failures, boundary_errors)
}

fn dispose_root_safely(root: RootHandle) -> Option<CleanupFailure> {
    match catch_unwind(AssertUnwindSafe(|| root.dispose())) {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(CleanupFailure::new(CleanupOrigin::Root, error)),
        Err(panic) => Some(CleanupFailure::new(
            CleanupOrigin::Root,
            CleanupError::from_panic(panic),
        )),
    }
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
    root: Option<RootHandle>,
    boundary: Option<MountBoundary>,
    cleanup_sink: CleanupSink,
    provisional_failures: Rc<RefCell<Vec<CleanupFailure>>>,
    generation: u64,
}

impl MountAttempt {
    fn new(
        root: RootHandle,
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
        let mount_result = root.with_scope(|scope| {
            let context = MountContext::with_cleanup_failures(
                scope,
                parent,
                self.provisional_failures.clone(),
            );
            builder(&context)
        });
        let provisional_failures = self.take_provisional_failures();

        match mount_result {
            Ok(()) => {
                if let Err(primary) = self
                    .boundary
                    .as_mut()
                    .expect("mount attempt boundary must exist")
                    .finish_staging()
                {
                    return self.fail(primary, provisional_failures);
                }
                if let Err(primary) = self
                    .boundary
                    .as_mut()
                    .expect("mount attempt boundary must exist")
                    .commit()
                {
                    return self.fail(primary, provisional_failures);
                }
                self.publish()
            }
            Err(primary) => self.fail(primary, provisional_failures),
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

    fn root_with_cleanup_panic(message: &'static str) -> RootHandle {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should be created");
        root.with_scope(|scope| {
            scope
                .on_cleanup(
                    move || panic!("{message}"),
                    scope
                        .error_handler(|_: SilexError| {})
                        .expect("error handler should register"),
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
