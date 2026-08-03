use crate::attribute::PendingAttribute;
use crate::view::{ApplyAttributes, View, ViewOwner};
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

impl<'scope, 'run, V: ApplyAttributes<'scope, 'run>> ApplyAttributes<'scope, 'run>
    for ScopeView<V>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        self.view.apply_attributes(attrs);
    }
}

impl<'scope, 'run, V: View<'scope, 'run>> View<'scope, 'run> for ScopeView<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        self.view.mount(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        self.view.mount_owned(owner, parent, attrs);
    }
}
