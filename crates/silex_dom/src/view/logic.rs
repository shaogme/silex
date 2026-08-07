use crate::attribute::PendingAttribute;
use crate::view::{ApplyAttributes, View, ViewOwner};
use silex_core::SilexResult;
use web_sys::Node;

/// View wrapper reserved for an explicit lexical owner supplied by the caller.
/// The wrapper itself never creates a detached or global scope.
pub struct ScopeView<V> {
    view: V,
}

impl<V> ScopeView<V> {
    pub fn new(view: V) -> Self {
        Self { view }
    }
}

impl<'scope, V: ApplyAttributes<'scope>> ApplyAttributes<'scope> for ScopeView<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.view.apply_attributes(attrs);
    }
}

impl<'scope, V: View<'scope>> View<'scope> for ScopeView<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        self.view.mount(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.view.mount_owned(owner, parent, attrs)
    }
}
