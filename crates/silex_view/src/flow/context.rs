use crate::lifecycle::{MountErrorHandler, MountOwnerToken};
use silex_core::OwnerAccess;

/// 动态分支和 row renderer 共享的最小 owner capability。
#[derive(Clone)]
pub struct BranchRenderContext<'scope> {
    content_owner: MountOwnerToken<'scope>,
    owner: OwnerAccess<'scope>,
    error_handler: MountErrorHandler<'scope>,
}

impl<'scope> BranchRenderContext<'scope> {
    pub(crate) fn new(
        content_owner: MountOwnerToken<'scope>,
        owner: OwnerAccess<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self {
            content_owner,
            owner,
            error_handler,
        }
    }

    pub fn owner(&self) -> OwnerAccess<'scope> {
        self.owner
    }

    pub fn content_owner(&self) -> MountOwnerToken<'scope> {
        self.content_owner.clone()
    }

    pub fn error_handler(&self) -> MountErrorHandler<'scope> {
        self.error_handler
    }
}
