//! Scope-owned host object references.

use crate::{
    AnyValue, ReactiveResult,
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
    pub fn node_ref<T: 'scope>(&self) -> NodeRef<'scope, 'run, T> {
        let value = AnyValue::new(Option::<T>::None);
        // SAFETY: `value` 存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与 node_ref 内部值，
        // 因此将 `value` 的生命周期延伸至 `'run` 是 Sound 的。
        let value = unsafe { std::mem::transmute::<AnyValue<'scope>, AnyValue<'run>>(value) };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_node_ref(value);
        NodeRef {
            handle: Handle::new(self.frame, raw),
            marker: PhantomData,
        }
    }
}

impl<'scope, 'run, T: Clone + 'scope> NodeRef<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<Option<T>> {
        runtime::node_ref_get(&self.handle.state(), self.handle.raw())
    }

    pub fn get(&self) -> Option<T> {
        self.try_get().ok().flatten()
    }
}

impl<'scope, 'run, T: 'scope> NodeRef<'scope, 'run, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::node_ref_set(&self.handle.state(), self.handle.raw(), value)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
