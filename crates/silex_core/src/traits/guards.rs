use crate::NodeRef;
use std::ops::Deref;

/// 内部辅助 Trait，用于抹平所有权存储与借用目标之间的 Deref 差异。
pub trait GuardStorage<T: ?Sized> {
    fn borrow_storage(&self) -> &T;
}

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

/// 统一大一统的响应式守卫。
///
/// - 'a: 借用生命周期。
/// - T: 逻辑值类型（支持 ?Sized）。
/// - S: 内部存储类型（必须 Sized，默认为 ()，当需要 Owned 变体时应指定具体的类型）。
pub enum RxGuard<'a, T: ?Sized, S = ()> {
    /// 借用变体：可以是来自 Arena 的信号引用，也可以是来自 Constant 的静态引用。
    Borrowed {
        value: &'a T,
        token: Option<NodeRef>,
    },
    /// 所有权变体：持有计算结果或内联值。
    Owned(S),
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
