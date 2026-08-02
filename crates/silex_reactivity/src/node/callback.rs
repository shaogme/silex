//! Scope-owned type-erased callbacks.

use crate::{
    ReactiveResult,
    handle::{CallbackId, Handle},
    internal::value::{AnyValue, CallbackThunk},
    runtime,
    scope::Scope,
};

pub struct Callback<'scope, 'run> {
    pub(crate) handle: CallbackId<'scope, 'run>,
}

impl Copy for Callback<'_, '_> {}

impl Clone for Callback<'_, '_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Register a type-erased callback under this scope.
    pub fn callback<F>(&self, f: F) -> Callback<'scope, 'run>
    where
        F: FnMut(AnyValue<'scope>) + 'scope,
    {
        let thunk = CallbackThunk::new(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与回调，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_callback(thunk);
        Callback {
            handle: Handle::new(self.frame, raw),
        }
    }
}

impl<'scope, 'run> Callback<'scope, 'run> {
    pub fn invoke(&self, arg: AnyValue<'scope>) -> ReactiveResult<()> {
        // SAFETY: `arg` 仅在 `invoke_callback` 同步执行期间传给 `CallbackThunk`。
        // `CallbackThunk` 已被 extend_lifetime 存为 `'run`，但其内部闭包实际在 `'scope` 销毁。
        // 将 `arg` 提升至 `'run` 生命周期在同步调用内部是 sound 的。
        let arg = unsafe { std::mem::transmute::<AnyValue<'scope>, AnyValue<'run>>(arg) };
        runtime::invoke_callback(&self.handle.state(), self.handle.raw(), arg)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
