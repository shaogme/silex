use crate::kernel::{
    MountAncestry, MountContext, MountDomAction, MountInstance, MountTarget, MountTransaction, View,
};
use crate::lifecycle::MountOwnerToken;
use silex_core::{ErrorHandlerInput, OwnerAccess, SilexResult};
use silex_dom::{model::DomNode, runtime::DomContext};

/// 应用 builder 使用的 context；它不等同于 View kernel 的 `MountContext`。
pub struct MountBuilderContext<'scope> {
    pub(crate) access: OwnerAccess<'scope>,
    pub(crate) dom: DomContext,
    pub(crate) parent: DomNode,
    pub(crate) owner: MountOwnerToken<'scope>,
    pub(crate) transaction: MountTransaction<'scope>,
}

impl<'scope> MountBuilderContext<'scope> {
    pub(crate) fn new(
        access: OwnerAccess<'scope>,
        dom: DomContext,
        parent: DomNode,
        owner: MountOwnerToken<'scope>,
        transaction: MountTransaction<'scope>,
    ) -> Self {
        Self {
            access,
            dom,
            parent,
            owner,
            transaction,
        }
    }

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
