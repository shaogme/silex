use crate::traits::{GuardStorage, RxGuard};
use std::ops::Deref;

impl<T: Sized> GuardStorage<T> for T {
    #[inline(always)]
    fn borrow_storage(&self) -> &T {
        self
    }
}

impl GuardStorage<str> for String {
    #[inline(always)]
    fn borrow_storage(&self) -> &str {
        self.as_str()
    }
}

impl<'a, T: ?Sized, S: GuardStorage<T>> Deref for RxGuard<'a, T, S> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed { value, .. } => value,
            Self::Owned(s) => s.borrow_storage(),
        }
    }
}

impl<'a, T: ?Sized, S> RxGuard<'a, T, S> {
    /// 投影借用守卫持有的引用。
    /// 仅在当前守卫为 Borrowed 时有效，否则返回 None。
    #[inline(always)]
    pub fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Option<RxGuard<'a, U, ()>> {
        match self {
            Self::Borrowed { value, token } => Some(RxGuard::Borrowed {
                value: f(value),
                token,
            }),
            Self::Owned(_) => None,
        }
    }
}
