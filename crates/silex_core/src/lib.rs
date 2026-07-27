pub mod callback;
pub mod error;
pub mod log;
pub mod logic;
pub mod macros_helper;
pub mod node_ref;
pub mod reactivity;
pub mod traits;

pub use callback::Callback;
pub use error::{SilexError, SilexResult};
pub use node_ref::NodeRef;
use reactivity::{RawId, StoredId};

pub struct RxValueKind;
pub struct RxEffectKind;

/// 响应式计算单元或事件处理器（类型擦除版）。
/// Rx 现在对返回值 T 是泛型的，从而解决了闭包导致的单态化膨胀问题。
pub struct Rx<T, M = RxValueKind> {
    pub(crate) inner: RxInner,
    pub(crate) _marker: ::core::marker::PhantomData<(T, M)>,
}

impl<T: 'static> Rx<T, RxValueKind> {
    /// 从已包装的闭包创建一个派生计算节点 (池化存储)。
    /// 宏 `rx!` 的核心后端逻辑。通过接受 Box 来最小化单态化膨胀。
    ///
    /// 保管的类型就是 `Box<dyn Fn() -> T>` 本身。从前是把它再包一层
    /// `Box<dyn Any>` 交给 `register_closure`，读的时候多一次装箱与一次解引用
    /// （审计报告 §2.6 —— `ExtraData::Closure` 已随之删除）。
    pub fn derive(f: Box<dyn Fn() -> T>) -> Self {
        Self::from_closure(f)
    }

    /// 从纯函数指针创建一个派生计算节点。
    /// 相比 `derive`，它不涉及闭包类型生成的代码膨胀。
    pub fn derive_fn(f: fn() -> T) -> Self {
        Self::from_closure(Box::new(f))
    }

    fn from_closure(f: Box<dyn Fn() -> T>) -> Self {
        // `untrack` 只关依赖追踪，不动所有权（AUDIT 二轮 §1.1）。
        let id = silex_reactivity::scope::untrack(|| silex_reactivity::store::create(f));
        Self {
            inner: RxInner::Closure(id),
            _marker: ::core::marker::PhantomData,
        }
    }
}

impl<T: 'static> Rx<T, RxEffectKind> {
    /// 存储一个响应式值或回调（直接存储）。
    pub fn effect(val: T) -> Self {
        let id = silex_reactivity::scope::untrack(|| silex_reactivity::store::create(val));
        Self::new_stored(id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RxInner {
    InlineConstant(u64),
    Signal(RawId),
    Closure(StoredId),
    Op(StoredId),
    /// 直接存储的值（不通过工厂函数，直接借用）
    Stored(StoredId),
}

/// 非泛型的响应式节点类型，用于 Trampoline 模式优化。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RxNodeKind {
    Signal,
    Closure,
    Op,
    Stored,
}

impl RxInner {
    /// 将泛型枚举转换为非泛型的擦除句柄和类型标识。
    /// 用于将逻辑分发到非泛型函数中。
    #[inline(always)]
    pub fn as_raw_parts(&self) -> Option<(RawId, RxNodeKind)> {
        match self {
            Self::InlineConstant(_) => None,
            Self::Signal(id) => Some((*id, RxNodeKind::Signal)),
            Self::Closure(id) => Some((id.raw(), RxNodeKind::Closure)),
            Self::Op(id) => Some((id.raw(), RxNodeKind::Op)),
            Self::Stored(id) => Some((id.raw(), RxNodeKind::Stored)),
        }
    }
}

impl<T: 'static, M> Rx<T, M> {
    /// 保管一段类型擦除的操作载荷。
    ///
    /// 从前这里走的是 `silex_reactivity::register_op` 与一个 64 字节的
    /// `RawOpBuffer`：`[MaybeUninit<u8>; 64] + Copy`，节点销毁时只是丢掉 64 字节
    /// 原始内存，**载荷自己的析构函数永远不会运行**。今天不出事是因为所有载荷
    /// 恰好都是 POD，但那从来没有被强制过，而且既然是 `Copy`，安全代码可以把
    /// 借到的缓冲区复制出任意份 —— 一旦将来支持析构就直接是 double-free
    /// （审计报告 §2.4）。
    ///
    /// 现在直接用 `store_value`：`AnyValue` 自带 SOO（小载荷同样内联，不进堆）、
    /// 类型检查与正确的析构，代价只是读的时候多一次 `TypeId` 比较。
    /// 64 字节 / 16 对齐 / `!needs_drop` 三条限制随之一起消失。
    pub fn new_op<P: 'static>(op: P) -> Self {
        // `untrack` 只关依赖追踪，不动所有权：节点照旧挂在当前 owner 下面，
        // 随它一起销毁（AUDIT 二轮 §1.1）。
        let id = silex_reactivity::scope::untrack(|| silex_reactivity::store::create(op));
        Self {
            inner: RxInner::Op(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub fn new_constant(val: T) -> Self {
        #[allow(clippy::manual_is_variant_and)]
        if const {
            std::mem::size_of::<T>() <= std::mem::size_of::<u64>() && !std::mem::needs_drop::<T>()
        } {
            unsafe {
                let mut storage = 0u64;
                std::ptr::copy_nonoverlapping(
                    &val as *const T as *const u8,
                    &mut storage as *mut u64 as *mut u8,
                    std::mem::size_of::<T>(),
                );
                std::mem::forget(val);
                Self {
                    inner: RxInner::InlineConstant(storage),
                    _marker: ::core::marker::PhantomData,
                }
            }
        } else {
            let id =
                silex_reactivity::scope::create_detached(|| silex_reactivity::store::create(val)).1;
            Self {
                inner: RxInner::Stored(id),
                _marker: ::core::marker::PhantomData,
            }
        }
    }

    /// Internal helper to unpack an inlined value
    pub(crate) unsafe fn unpack_inline(storage: u64) -> T {
        unsafe {
            let mut value = std::mem::MaybeUninit::<T>::uninit();
            std::ptr::copy_nonoverlapping(
                &storage as *const u64 as *const u8,
                value.as_mut_ptr() as *mut u8,
                std::mem::size_of::<T>(),
            );
            value.assume_init()
        }
    }

    pub const fn new_signal(id: RawId) -> Self {
        Self {
            inner: RxInner::Signal(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_pooled(id: StoredId) -> Self {
        // We assume new_pooled is used for Closure, as it was previously for Pooled
        Self {
            inner: RxInner::Closure(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_stored(id: StoredId) -> Self {
        Self {
            inner: RxInner::Stored(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_inline_constant(storage: u64) -> Self {
        Self {
            inner: RxInner::InlineConstant(storage),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_closure(id: StoredId) -> Self {
        Self {
            inner: RxInner::Closure(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_op_raw(id: RawId) -> Self {
        Self {
            inner: RxInner::Op(StoredId::from_raw_unchecked(id)),
            _marker: ::core::marker::PhantomData,
        }
    }
}

impl<T, M> Clone for Rx<T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, M> PartialEq for Rx<T, M> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T, M> Copy for Rx<T, M> {}

pub use silex_rx::rx as __internal_rx;

#[macro_export]
macro_rules! rx {
    ($($body:tt)*) => {
        $crate::__internal_rx!($crate; $($body)*)
    };
}

pub mod prelude {
    pub use crate::{
        Callback, NodeRef, Rx, SilexError, SilexResult, batch_read, batch_read_untracked, log::*,
        logic::*, reactivity::*, rx, traits::*,
    };
}

/// Multi-signal batch read macro for zero-copy access to multiple signals.
///
/// This macro provides a way to access multiple signals without cloning, by nesting
/// the closures internally. All signals will be tracked for reactive updates.
///
/// # Example
/// ```rust,ignore
/// let name = Signal::pair("Alice".to_string());
/// let age = Signal::pair(42);
///
/// // Zero-copy access - no cloning!
/// batch_read!(name, age => |n: &String, a: &i32| {
///     println!("{} is {} years old", n, a);
/// });
///
/// // Returns a value
/// let greeting = batch_read!(name, age => |n: &String, a: &i32| {
///     format!("Hello, {} (age {})", n, a)
/// });
/// ```
#[macro_export]
macro_rules! batch_read {
    // 转发给递归实现
    ($($s:expr),+ => |$($p:ident: $t:ty),+| $body:expr) => {
        $crate::batch_read_recurse!([$($s),+] => [$($p: $t),+] => $body)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! batch_read_recurse {
    ([$s1:expr] => [$p1:ident: $t1:ty] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($s1).with(|$p1: $t1| $body)
    }};
    ([$s1:expr, $($ss:expr),+] => [$p1:ident: $t1:ty, $($ps:ident: $ts:ty),+] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($s1).with(|$p1: $t1| $crate::batch_read_recurse!([$($ss),+] => [$($ps: $ts),+] => $body))
    }};
}

/// Untracked version of batch_read - does not subscribe to signal changes.
#[macro_export]
macro_rules! batch_read_untracked {
    // 递归实现
    ([$s1:expr] => [$p1:ident: $t1:ty] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($s1).with_untracked(|$p1: $t1| $body)
    }};
    ([$s1:expr, $($ss:expr),+] => [$p1:ident: $t1:ty, $($ps:ident: $ts:ty),+] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($s1).with_untracked(|$p1: $t1| $crate::batch_read_untracked!([$($ss),+] => [$($ps: $ts),+] => $body))
    }};
    // 包装器，支持外部调用的逗号分隔语法
    ($($s:expr),+ => |$($p:ident: $t:ty),+| $body:expr) => {
        $crate::batch_read_untracked!([$($s),+] => [$($p: $t),+] => $body)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Rx` 的相等性看的是内部句柄，不是 `T`。
    ///
    /// 从前这个用例直接捏造擦除句柄 —— 字段现在
    /// 是私有的（审计报告 §3.4：伪造的巨大 index 会让二级表 `resize_with` 出
    /// 巨量内存），所以改成用真实创建的句柄。
    #[test]
    fn rx_equality_tracks_inner_identity() {
        let one = silex_reactivity::signal::create(0u8).raw();
        let two = silex_reactivity::signal::create(0u8).raw();

        let a = Rx::<(), RxValueKind>::new_signal(one);
        let b = Rx::<(), RxValueKind>::new_signal(one);
        let c = Rx::<(), RxValueKind>::new_signal(two);

        assert!(a == b);
        assert!(a != c);
    }
}
