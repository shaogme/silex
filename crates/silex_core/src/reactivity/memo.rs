use crate::{Rx, RxValueKind, Scope};
use std::fmt;

/// Equality-gated computed value.
pub struct Memo<'scope, 'run, T> {
    pub(crate) inner: silex_reactivity::Memo<'scope, 'run, T>,
    pub(crate) scope: Scope<'scope, 'run>,
}

impl<'scope, 'run, T> Copy for Memo<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Memo<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for Memo<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memo").finish_non_exhaustive()
    }
}

impl<'scope, 'run, T: 'scope> Memo<'scope, 'run, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::Memo<'scope, 'run, T>,
        scope: Scope<'scope, 'run>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.get()
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner.with(f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner.with_untracked(f).expect("读取 scoped memo 失败")
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    pub fn map<U, F>(self, f: F) -> Rx<'scope, 'run, U>
    where
        U: 'run,
        F: Fn(&T) -> U + 'scope,
    {
        let scope = self.scope;
        scope.derived(move || self.with(|value| f(value)))
    }

    pub fn into_rx(self) -> Rx<'scope, 'run, T, RxValueKind> {
        Rx::from_memo(self)
    }
}
