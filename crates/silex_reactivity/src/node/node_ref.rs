//! Scope-owned host object references.

use crate::{
    ReactiveResult,
    handle::{Handle, NodeRefId},
    runtime,
    scope::Scope,
};
use std::marker::PhantomData;

pub struct NodeRef<'scope, 'run, T> {
    pub(crate) handle: NodeRefId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for NodeRef<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for NodeRef<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create an empty host reference.
    pub fn node_ref<T: 'static>(&self) -> NodeRef<'scope, 'run, T> {
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_node_ref::<T>();
        NodeRef {
            handle: Handle::new(self.frame, raw),
            marker: PhantomData,
        }
    }
}

impl<'scope, 'run, T: Clone + 'static> NodeRef<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<Option<T>> {
        runtime::node_ref_get(&self.handle.state(), self.handle.raw())
    }

    pub fn get(&self) -> Option<T> {
        self.try_get().ok().flatten()
    }
}

impl<T: 'static> NodeRef<'_, '_, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::node_ref_set(&self.handle.state(), self.handle.raw(), value)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
