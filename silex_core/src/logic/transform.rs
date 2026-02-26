use crate::reactivity::Memo;
use crate::traits::{RxBase, RxRead};

/// 允许从当前信号创建一个衍生信号。
pub trait Map: RxBase + Sized {
    /// 基于当前信号派生出一个新信号。
    fn map<U, F>(self, f: F) -> crate::Rx<U, crate::RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static;
}

impl<S> Map for S
where
    S: RxRead + Clone + 'static,
{
    fn map<U, F>(self, f: F) -> crate::Rx<U, crate::RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static,
    {
        crate::Rx::derive(Box::new(move || self.with(|v| f(v))))
    }
}

/// 允许将一个信号转换为自带缓存的记忆化 (Memoize) 信号。
///
/// 要求 `Value: Clone + Sized`，因为记忆化需要克隆并存储缓存值。
pub trait Memoize: RxRead + Clone + 'static
where
    Self::Value: Sized,
{
    /// 对该信号的值进行记忆化缓存。
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: Clone + PartialEq + 'static;
}

impl<T> Memoize for T
where
    T: RxRead + Clone + 'static,
    T::Value: Clone + Sized,
{
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| this.with(Clone::clone))
    }
}
