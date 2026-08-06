use crate::{Rx, RxValueKind, Scope};
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

impl<'scope, T: 'scope> Memo<'scope, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::Memo<'scope, T>,
        scope: Scope<'scope>,
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
