use crate::reactivity::Memo;
use crate::traits::{RxBase, RxRead};

/// 允许从当前信号创建一个衍生信号。
pub trait Map: RxBase + Sized {
    /// 基于当前信号派生出一个新信号。
    fn map<U, F>(self, f: F) -> crate::Rx<U, crate::RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static;

    /// 使用静态函数指针派生出一个新信号（零成本，无闭包分配）。
    fn map_fn<U>(self, f: fn(&Self::Value) -> U) -> crate::Rx<U, crate::RxValueKind>
    where
        U: 'static;
}

impl<S> Map for S
where
    S: crate::traits::RxRead + Clone + RxBase + 'static,
    S::Value: Sized + 'static,
{
    fn map<U, F>(self, f: F) -> crate::Rx<U, crate::RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static,
    {
        if self.rx_is_constant() {
            if let Some(res) = self.rx_try_with_untracked(|v| crate::Rx::new_constant(f(v))) {
                return res;
            }
        }
        crate::Rx::derive(Box::new(move || self.with(|v| f(v))))
    }

    fn map_fn<U>(self, f: fn(&Self::Value) -> U) -> crate::Rx<U, crate::RxValueKind>
    where
        U: 'static,
    {
        if self.rx_is_constant() {
            if let Some(res) = self.rx_try_with_untracked(|v| crate::Rx::new_constant(f(v))) {
                return res;
            }
        }
        if let Some(id) = self.id() {
            let op = crate::reactivity::StaticMapPayload::new(id, f, false);
            crate::Rx::new_op_raw(op)
        } else {
            crate::Rx::derive(Box::new(move || self.with(|v| f(v))))
        }
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

impl<T, M> Memoize for crate::Rx<T, M>
where
    T: Clone + Sized + 'static,
    M: 'static,
{
    fn memo(self) -> Memo<T>
    where
        T: PartialEq + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| this.with(Clone::clone))
    }
}
