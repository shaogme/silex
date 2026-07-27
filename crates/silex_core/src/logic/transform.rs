use crate::{
    Rx, RxValueKind,
    reactivity::{Memo, StaticMapPayload},
    traits::{RxBase, RxRead},
};

/// 允许从当前信号创建一个衍生信号。
pub trait Map: RxBase {
    /// 基于当前信号派生出一个新信号。
    fn map<U, F>(self, f: F) -> Rx<U, RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static;

    /// 使用静态函数指针派生出一个新信号（零成本，无闭包分配）。
    fn map_fn<U>(self, f: fn(&Self::Value) -> U) -> Rx<U, RxValueKind>
    where
        U: 'static;
}

impl<S> Map for S
where
    S: RxRead + Clone + RxBase + 'static,
    S::Value: Sized + 'static,
{
    fn map<U, F>(self, f: F) -> Rx<U, RxValueKind>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: 'static,
    {
        if self.rx_is_constant()
            && let Some(res) = self.rx_try_with_untracked(|v| Rx::new_constant(f(v)))
        {
            return res;
        }
        Rx::derive(Box::new(move || self.with(|v| f(v))))
    }

    fn map_fn<U>(self, f: fn(&Self::Value) -> U) -> Rx<U, RxValueKind>
    where
        U: 'static,
    {
        if self.rx_is_constant()
            && let Some(res) = self.rx_try_with_untracked(|v| Rx::new_constant(f(v)))
        {
            return res;
        }
        if let Some(id) = self.id() {
            let op = StaticMapPayload::new1(id, f, false);
            Rx::new_op(op)
        } else {
            Rx::derive(Box::new(move || self.with(f)))
        }
    }
}

/// 允许将一个信号转换为自带缓存的记忆化 (Memoize) 信号。
pub trait Memoize: RxRead {
    /// 对该信号的值进行记忆化缓存。
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + Sized + 'static;
}

impl<T, M> Memoize for Rx<T, M>
where
    T: PartialEq + Clone + 'static,
    M: 'static,
{
    fn memo(self) -> Memo<T> {
        Memo::new(move |_| self.with(Clone::clone))
    }
}
