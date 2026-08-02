//! Scoped lazy memo and derived values.

use crate::{
    ReactiveError, ReactiveResult,
    handle::{DerivedId, Handle, MemoId},
    internal::value::MemoThunk,
    runtime,
    scope::Scope,
};
use std::marker::PhantomData;

pub struct Memo<'scope, 'run, T> {
    pub(crate) handle: MemoId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
}

pub struct Derived<'scope, 'run, T> {
    pub(crate) handle: DerivedId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for Memo<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Memo<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for Derived<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Derived<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create a lazy memo whose dependents are notified only when its value
    /// changes according to `PartialEq`.
    pub fn memo<T, F>(&self, f: F) -> Memo<'scope, 'run, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        let thunk = MemoThunk::new::<T, F>(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与闭包，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_memo(thunk, false);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Memo {
            handle,
            marker: PhantomData,
        }
    }

    /// Create a lazy derived value without equality gating.
    pub fn derived<T, F>(&self, f: F) -> Derived<'scope, 'run, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        let thunk = MemoThunk::new_derived::<T, F>(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与闭包，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_memo(thunk, true);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Derived {
            handle,
            marker: PhantomData,
        }
    }
}

impl<'scope, 'run, T: 'scope> Memo<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 scoped memo 失败")
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped memo 失败")
    }

    pub fn try_with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        self.try_with_untracked(f)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> Derived<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 scoped derived 失败")
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .expect("读取 scoped derived 的类型不匹配")
        })
        .expect("读取 scoped derived 失败")
    }

    pub fn try_with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        self.try_with_untracked(f)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
