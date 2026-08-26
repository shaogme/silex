use super::types::{CleanupReporter, MountErrorHandler};
use silex_core::{ErrorHandlerAnchor, OwnerAccess, ReactiveError, SilexError, SilexResult};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeOwnershipMode {
    Shared,
    BranchContent,
}

/// 共享一个 View 生命周期树的 runtime access 和 handler anchors。
pub struct MountOwnerContext<'scope> {
    pub(crate) access: OwnerAccess<'scope>,
    pub(crate) anchors: RefCell<Vec<ErrorHandlerAnchor<'scope>>>,
    pub(crate) cleanup_reporter: Option<CleanupReporter>,
    pub(crate) runtime_mode: RuntimeOwnershipMode,
}

impl<'scope> MountOwnerContext<'scope> {
    pub(crate) fn new(
        access: OwnerAccess<'scope>,
        reporter: Option<CleanupReporter>,
        mode: RuntimeOwnershipMode,
    ) -> Self {
        Self {
            access,
            anchors: RefCell::new(Vec::new()),
            cleanup_reporter: reporter,
            runtime_mode: mode,
        }
    }

    pub(crate) fn access(&self) -> OwnerAccess<'scope> {
        self.access
    }

    pub(crate) fn owns_runtime_handles(&self) -> bool {
        self.runtime_mode == RuntimeOwnershipMode::Shared
    }

    pub(crate) fn handler(
        &self,
        requested: MountErrorHandler<'scope>,
    ) -> SilexResult<MountErrorHandler<'scope>> {
        if let Some(anchor) = self
            .anchors
            .borrow()
            .iter()
            .find(|anchor| anchor.view().is_same_handler(&requested))
        {
            return Ok(anchor.view());
        }
        let anchor = requested
            .anchor()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        let handler = anchor.view();
        self.anchors.borrow_mut().push(anchor);
        Ok(handler)
    }
}
