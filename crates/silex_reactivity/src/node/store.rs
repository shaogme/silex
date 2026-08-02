//! Scope-owned, non-reactive values.

use crate::{
    ReactiveError, ReactiveResult,
    handle::{Handle, StoredId},
    internal::value::AnyValue,
    runtime,
    scope::Scope,
};
use std::{marker::PhantomData, mem::transmute};

pub struct StoredValue<'scope, 'run, T> {
    pub(crate) handle: StoredId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for StoredValue<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for StoredValue<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Store a non-reactive value under this scope.
    pub fn stored<T: 'scope>(&self, value: T) -> StoredValue<'scope, 'run, T> {
        let value = AnyValue::new(value);
        // SAFETY: `value` 存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与 stored value，
        // 因此将 `value` 的生命周期延伸至 `'run` 是 Sound 的。
        let value = unsafe { transmute::<AnyValue<'scope>, AnyValue<'run>>(value) };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_stored(value);
        StoredValue {
            handle: Handle::new(self.frame, raw),
            marker: PhantomData,
        }
    }
}

impl<'scope, 'run, T: 'scope> StoredValue<'scope, 'run, T> {
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_stored(&self.handle.state(), self.handle.raw(), |value| {
            value
                .downcast_ref::<T>()
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped stored value 失败")
    }

    pub fn try_update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        runtime::update_stored(&self.handle.state(), self.handle.raw(), |value| {
            value
                .downcast_mut::<T>()
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        self.try_update(f).expect("更新 scoped stored value 失败")
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
