use crate::app::MountBuilderContext;
use crate::kernel::MountTransaction;
use crate::lifecycle::{CleanupReporter, MountOwnerToken};
use silex_core::{CloseError, OwnerHandle, SilexError, SilexErrorKind, SilexResult};
use silex_dom::lifecycle::{CleanupFailure, CleanupOrigin, CleanupReport};
use silex_dom::{model::DomNode, runtime::DomContext};
use std::any::Any;
use std::cell::RefCell;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

pub(crate) struct MountBoundary {
    dom: DomContext,
    host: DomNode,
    staging: DomNode,
    owned_nodes: Vec<DomNode>,
    finished: bool,
    committed: bool,
}

impl MountBoundary {
    pub(crate) fn new(dom: DomContext, host: DomNode) -> SilexResult<Self> {
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

    pub(crate) fn staging(&self) -> DomNode {
        self.staging.clone()
    }

    pub(crate) fn finish_staging(&mut self) -> SilexResult<()> {
        if self.finished {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was finalized twice".into(),
            )));
        }
        self.owned_nodes = self.dom.children(&self.staging)?;
        self.finished = true;
        Ok(())
    }

    pub(crate) fn commit(&mut self) -> SilexResult<()> {
        if self.committed {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mount boundary was committed twice".into(),
            )));
        }
        self.dom.append(&self.host, &self.staging)?;
        self.committed = true;
        Ok(())
    }

    pub(crate) fn dispose(&mut self) -> Vec<SilexError> {
        let parent = if self.committed {
            self.host.clone()
        } else {
            self.staging.clone()
        };
        let mut errors = Vec::new();
        let nodes = if self.finished {
            mem::take(&mut self.owned_nodes)
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

pub(crate) struct MountSession {
    pub(crate) root: OwnerHandle,
    pub(crate) boundary: MountBoundary,
    pub(crate) generation: u64,
}

pub(crate) fn new_builder_context<'scope>(
    access: silex_core::OwnerAccess<'scope>,
    dom: DomContext,
    boundary: &MountBoundary,
    owner: MountOwnerToken<'scope>,
    transaction: MountTransaction<'scope>,
) -> MountBuilderContext<'scope> {
    MountBuilderContext::new(access, dom, boundary.staging(), owner, transaction)
}

pub(crate) fn cleanup_session(session: &mut MountSession) -> CleanupReport {
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

pub(crate) fn cleanup_reporter() -> (CleanupReporter, Rc<RefCell<Vec<CleanupFailure>>>) {
    let failures = Rc::new(RefCell::new(Vec::<CleanupFailure>::new()));
    let failures_for_reporter = failures.clone();
    let reporter: CleanupReporter = Rc::new(move |error| {
        failures_for_reporter
            .borrow_mut()
            .push(CleanupFailure::new(CleanupOrigin::ProvisionalOwner, error));
    });
    (reporter, failures)
}

pub(crate) fn panic_error(operation: &str, panic: Box<dyn Any + Send>) -> SilexError {
    let error = CloseError::from_panic(panic);
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "{operation} panicked: {}",
        error.diagnostic().message()
    )))
}
