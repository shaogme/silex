use crate::{ReactiveResult, Rx, RxValueKind, Scope};
use std::fmt;

/// Equality-gated computed value.
pub struct Memo<'scope, T> {
    pub(crate) inner: silex_reactivity::Memo<'scope, T>,
    pub(crate) scope: Scope<'scope>,
}

impl<'scope, T> Copy for Memo<'scope, T> {}

impl<'scope, T> Clone for Memo<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for Memo<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memo").finish_non_exhaustive()
    }
}

impl<'scope, T> PartialEq for Memo<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.scope == other.scope
    }
}

impl<'scope, T> Eq for Memo<'scope, T> {}

impl<'scope, T: 'scope> Memo<'scope, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::Memo<'scope, T>,
        scope: Scope<'scope>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.inner.try_get()
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get()
            .unwrap_or_else(|error| panic!("读取 scoped memo 失败: {error}"))
    }

    pub fn try_get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.inner.try_get_untracked()
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.try_get_untracked()
            .unwrap_or_else(|error| panic!("读取 scoped memo 失败: {error}"))
    }

    pub fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.try_with(f)
            .unwrap_or_else(|error| panic!("读取 scoped memo 失败: {error}"))
    }

    pub fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with_untracked(f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.try_with_untracked(f)
            .unwrap_or_else(|error| panic!("读取 scoped memo 失败: {error}"))
    }

    pub fn map<U, F>(self, f: F) -> Rx<'scope, U>
    where
        U: 'scope,
        F: Fn(&T) -> U + 'scope,
    {
        let scope = self.scope;
        let inputs = silex_reactivity::RuntimeInputs::single(self.inner.runtime_input());
        scope.derived_from(inputs, move || self.with(|value| f(value)))
    }

    pub fn into_rx(self) -> Rx<'scope, T, RxValueKind> {
        Rx::from_memo(self)
    }
}
