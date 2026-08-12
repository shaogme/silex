use std::fmt;

use crate::ReactiveResult;

/// A scope-owned host object reference.
pub struct NodeRef<'scope, T = ()> {
    pub(crate) inner: silex_reactivity::NodeRef<'scope, T>,
}

impl<'scope, T> Copy for NodeRef<'scope, T> {}

impl<'scope, T> Clone for NodeRef<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for NodeRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef").finish_non_exhaustive()
    }
}

impl<'scope, T: 'scope> NodeRef<'scope, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::NodeRef<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.try_get()
            .unwrap_or_else(|error| panic!("读取 NodeRef 失败: {error}"))
    }

    pub fn try_get(&self) -> ReactiveResult<Option<T>>
    where
        T: Clone,
    {
        self.inner.get()
    }

    pub fn try_load(&self, value: T) -> ReactiveResult<()> {
        self.inner.set(value)
    }

    pub fn load(&self, value: T) {
        self.try_load(value)
            .unwrap_or_else(|error| panic!("写入 NodeRef 失败: {error}"));
    }

    pub fn try_clear(&self) -> ReactiveResult<()> {
        self.inner.clear()
    }

    pub fn clear(&self) {
        self.try_clear()
            .unwrap_or_else(|error| panic!("清理 NodeRef 失败: {error}"));
    }
}
