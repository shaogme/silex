//! Application-level DOM mount transaction and ownership contracts.

use crate::{
    attribute::PendingAttribute,
    view::{ScopedViewOwner, View},
};
use silex_core::{
    CleanupDiagnostic, CleanupError, ErrorReporter, RootHandle, Runtime, Scope, SilexError,
    SilexResult, log::console_error,
};
use std::{
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

/// The scoped context exposed to an application mount builder.
///
/// A builder receives this context for the duration of one root-scope borrow.
/// It can mount multiple views, but cannot return the context, a scoped view, or
/// a scoped error handler from the callback.
pub struct MountContext<'scope> {
    scope: Scope<'scope>,
    parent: Node,
}

impl<'scope> MountContext<'scope> {
    fn new(scope: Scope<'scope>, parent: Node) -> Self {
        Self { scope, parent }
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
    pub fn owner(&self) -> ScopedViewOwner<'scope> {
        ScopedViewOwner::new(self.scope)
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
        staging.append_child(&start)?;
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
            return Err(SilexError::Framework(
                "mount boundary was finalized twice".to_string(),
            ));
        }
        if self.start.parent_node().as_ref() != Some(&self.staging) {
            return Err(SilexError::Dom(
                "mount boundary start anchor was detached".to_string(),
            ));
        }
        let end: Node = crate::document().create_comment("mount-end").into();
        self.staging.append_child(&end)?;
        self.end = Some(end);
        self.owned_nodes = Self::children_of(&self.staging);
        Ok(())
    }

    fn commit(&mut self) -> SilexResult<()> {
        if self.committed {
            return Err(SilexError::Framework(
                "mount boundary was committed twice".to_string(),
            ));
        }
        if self.end.is_none() {
            return Err(SilexError::Framework(
                "mount boundary was committed before finalization".to_string(),
            ));
        }
        self.host.append_child(&self.staging)?;
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
                if let Err(error) = parent.remove_child(&node).map_err(SilexError::from) {
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
enum TransactionState {
    Created,
    Mounting,
    Committed,
    Published,
    Disposing,
    Disposed,
    Aborted,
}

/// A successfully committed application mount.
pub struct MountedApp {
    _runtime: Runtime,
    root: Option<RootHandle>,
    boundary: Option<MountBoundary>,
    cleanup_sink: CleanupSink,
    state: TransactionState,
}

impl fmt::Debug for MountedApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MountedApp")
            .field("active", &self.is_active())
            .finish()
    }
}

impl MountedApp {
    /// Mount an application once and publish a handle only after commit.
    pub fn mount<F>(
        runtime: Runtime,
        host: Node,
        cleanup_sink: CleanupSink,
        builder: F,
    ) -> Result<Self, MountError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        MountTransaction::new(runtime, host, cleanup_sink)?.run(builder)
    }

    /// Whether the committed root is still active.
    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Published
            && self.root.as_ref().is_some_and(RootHandle::is_active)
    }

    /// Return the caller-supplied host node.
    pub fn host(&self) -> Node {
        self.boundary
            .as_ref()
            .expect("published application must own a boundary")
            .host
            .clone()
    }

    /// Dispose the root and outer DOM boundary in a fixed order.
    pub fn dispose(mut self) -> Result<(), DisposeError> {
        let report = self.dispose_inner();
        if report.is_clean() {
            Ok(())
        } else {
            Err(DisposeError::new(report))
        }
    }

    fn dispose_inner(&mut self) -> CleanupReport {
        if self.root.is_none() && self.boundary.is_none() {
            self.state = TransactionState::Disposed;
            return CleanupReport::new();
        }

        self.state = TransactionState::Disposing;
        let report = cleanup_parts(&mut self.root, &mut self.boundary);
        self.state = TransactionState::Disposed;
        report
    }
}

impl Drop for MountedApp {
    fn drop(&mut self) {
        if self.root.is_none() && self.boundary.is_none() {
            return;
        }

        let report = self.dispose_inner();
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

fn cleanup_report_for_root(root: RootHandle) -> CleanupReport {
    CleanupReport::from_parts(dispose_root_safely(root).into_iter().collect(), Vec::new())
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

struct MountTransaction {
    runtime: Option<Runtime>,
    root: Option<RootHandle>,
    boundary: Option<MountBoundary>,
    cleanup_sink: CleanupSink,
    state: TransactionState,
}

impl MountTransaction {
    fn new(runtime: Runtime, host: Node, cleanup_sink: CleanupSink) -> Result<Self, MountError> {
        let mut runtime = runtime;
        let root = match runtime.run() {
            Ok(root) => root,
            Err(primary) => return Err(MountError::new(primary, CleanupReport::new())),
        };
        let boundary = match MountBoundary::new(host) {
            Ok(boundary) => boundary,
            Err(primary) => {
                let rollback = cleanup_report_for_root(root);
                return Err(MountError::new(primary, rollback));
            }
        };
        Ok(Self {
            runtime: Some(runtime),
            root: Some(root),
            boundary: Some(boundary),
            cleanup_sink,
            state: TransactionState::Created,
        })
    }

    fn run<F>(mut self, builder: F) -> Result<MountedApp, MountError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        self.state = TransactionState::Mounting;
        let mount_result = catch_unwind(AssertUnwindSafe(|| {
            let root = self
                .root
                .as_ref()
                .expect("mount transaction root must exist");
            let parent = self
                .boundary
                .as_ref()
                .expect("mount transaction boundary must exist")
                .staging_parent();
            root.with_scope(|scope| {
                let context = MountContext::new(scope, parent);
                builder(&context)
            })
        }));

        match mount_result {
            Ok(Ok(())) => {
                if let Err(primary) = self
                    .boundary
                    .as_mut()
                    .expect("mount transaction boundary must exist")
                    .finish_staging()
                {
                    return self.fail(primary);
                }
                if let Err(primary) = self
                    .boundary
                    .as_mut()
                    .expect("mount transaction boundary must exist")
                    .commit()
                {
                    return self.fail(primary);
                }
                self.state = TransactionState::Committed;
                self.publish()
            }
            Ok(Err(primary)) => self.fail(primary),
            Err(panic) => {
                let report = self.abort();
                record_drop_report(&self.cleanup_sink, report);
                resume_unwind(panic)
            }
        }
    }

    fn fail(mut self, primary: SilexError) -> Result<MountedApp, MountError> {
        let rollback = self.abort();
        let _ = self.runtime.take();
        Err(MountError::new(primary, rollback))
    }

    fn abort(&mut self) -> CleanupReport {
        self.state = TransactionState::Aborted;
        cleanup_parts(&mut self.root, &mut self.boundary)
    }

    fn publish(mut self) -> Result<MountedApp, MountError> {
        self.state = TransactionState::Published;
        Ok(MountedApp {
            _runtime: self
                .runtime
                .take()
                .expect("published transaction runtime must exist"),
            root: Some(
                self.root
                    .take()
                    .expect("published transaction root must exist"),
            ),
            boundary: Some(
                self.boundary
                    .take()
                    .expect("published transaction boundary must exist"),
            ),
            cleanup_sink: self.cleanup_sink.clone(),
            state: self.state,
        })
    }
}

impl Drop for MountTransaction {
    fn drop(&mut self) {
        if matches!(
            self.state,
            TransactionState::Published | TransactionState::Aborted
        ) {
            return;
        }
        if self.root.is_none() && self.boundary.is_none() {
            return;
        }
        let report = self.abort();
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
