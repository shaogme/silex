//! Scoped effects.

use crate::{
    handle::{EffectId, Handle},
    internal::value::EffectThunk,
    runtime,
    scope::Scope,
};

pub struct Effect<'scope, 'run> {
    pub(crate) handle: EffectId<'scope, 'run>,
}

impl Copy for Effect<'_, '_> {}

impl Clone for Effect<'_, '_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create an effect owned by this scope and run it once immediately.
    pub fn effect<F>(&self, f: F) -> Effect<'scope, 'run>
    where
        F: FnMut() + 'scope,
    {
        let thunk = EffectThunk::new(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时（包括正常退出和 panic 恢复），`ScopeFrame::dispose` 会被强制调用
        // 并销毁所有节点与闭包，因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_effect(thunk);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Effect { handle }
    }

    /// Register an effect and intentionally discard its diagnostic handle.
    pub fn watch<F>(&self, f: F)
    where
        F: FnMut() + 'scope,
    {
        let _ = self.effect(f);
    }
}

impl Effect<'_, '_> {
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
