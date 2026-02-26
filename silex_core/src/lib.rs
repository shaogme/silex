pub mod callback;
pub mod error;
pub mod log;
pub mod node_ref;
pub mod reactivity;

pub mod logic;
pub mod traits;

pub use callback::Callback;
pub use error::{SilexError, SilexResult};
pub use node_ref::NodeRef;

pub struct RxValueKind;
pub struct RxEffectKind;

/// 响应式计算单元或事件处理器（类型擦除版）。
/// Rx 现在对返回值 T 是泛型的，从而解决了闭包导致的单态化膨胀问题。
pub struct Rx<T, M = RxValueKind> {
    pub(crate) inner: RxInner<T>,
    pub(crate) _marker: ::core::marker::PhantomData<M>,
}

impl<T: 'static> Rx<T, RxValueKind> {
    /// 从已包装的闭包创建一个派生计算节点 (池化存储)。
    /// 宏 `rx!` 的核心后端逻辑。通过接受 Box 来最小化单态化膨胀。
    pub fn derive(f: Box<dyn Fn() -> T>) -> Self {
        let id = silex_reactivity::untrack(|| {
            silex_reactivity::register_closure(Box::new(f) as Box<dyn std::any::Any>)
        });
        Self {
            inner: RxInner::Closure(id),
            _marker: ::core::marker::PhantomData,
        }
    }
}

impl<T: 'static> Rx<T, RxEffectKind> {
    /// 存储一个响应式值或回调（直接存储）。
    pub fn effect(val: T) -> Self {
        let id = silex_reactivity::untrack(|| silex_reactivity::store_value(val));
        Self::new_stored(id)
    }
}

pub enum RxInner<T> {
    Constant(T),
    Signal(crate::reactivity::NodeId),
    Closure(crate::reactivity::NodeId),
    Op1(crate::reactivity::NodeId),
    Op2(crate::reactivity::NodeId),
    /// 直接存储的值（不通过工厂函数，直接借用）
    Stored(crate::reactivity::NodeId),
}

impl<T: Clone> Clone for RxInner<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Constant(v) => Self::Constant(v.clone()),
            Self::Signal(id) => Self::Signal(*id),
            Self::Closure(id) => Self::Closure(*id),
            Self::Op1(id) => Self::Op1(*id),
            Self::Op2(id) => Self::Op2(*id),
            Self::Stored(id) => Self::Stored(*id),
        }
    }
}

impl<T: Copy> Copy for RxInner<T> {}

impl<T: 'static, M> Rx<T, M> {
    pub fn new_op1(op: crate::reactivity::OpPayload<T, 1>) -> Self {
        const { assert!(std::mem::size_of::<crate::reactivity::OpPayload<T, 1>>() <= 32) };
        let id = silex_reactivity::untrack(|| {
            let mut storage = [0u8; 32];
            unsafe {
                std::ptr::write(
                    storage.as_mut_ptr() as *mut crate::reactivity::OpPayload<T, 1>,
                    op,
                );
            }
            silex_reactivity::register_op1(storage)
        });
        Self {
            inner: RxInner::Op1(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub fn new_op2(op: crate::reactivity::OpPayload<T, 2>) -> Self {
        const { assert!(std::mem::size_of::<crate::reactivity::OpPayload<T, 2>>() <= 48) };
        let id = silex_reactivity::untrack(|| {
            let mut storage = [0u8; 48];
            unsafe {
                std::ptr::write(
                    storage.as_mut_ptr() as *mut crate::reactivity::OpPayload<T, 2>,
                    op,
                );
            }
            silex_reactivity::register_op2(storage)
        });
        Self {
            inner: RxInner::Op2(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_constant(val: T) -> Self {
        Self {
            inner: RxInner::Constant(val),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_signal(id: crate::reactivity::NodeId) -> Self {
        Self {
            inner: RxInner::Signal(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_pooled(id: crate::reactivity::NodeId) -> Self {
        // We assume new_pooled is used for Closure, as it was previously for Pooled
        Self {
            inner: RxInner::Closure(id),
            _marker: ::core::marker::PhantomData,
        }
    }

    pub const fn new_stored(id: crate::reactivity::NodeId) -> Self {
        Self {
            inner: RxInner::Stored(id),
            _marker: ::core::marker::PhantomData,
        }
    }
}

impl<T: Clone, M> Clone for Rx<T, M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: self._marker,
        }
    }
}

impl<T: Copy, M> Copy for Rx<T, M> {}

pub use silex_rx::rx as __internal_rx;

#[macro_export]
macro_rules! rx {
    ($($body:tt)*) => {
        $crate::__internal_rx!($crate; $($body)*)
    };
}

pub mod prelude {
    pub use crate::callback::Callback;
    pub use crate::log::*;
    pub use crate::logic::*;
    pub use crate::node_ref::NodeRef;
    pub use crate::reactivity::*;
    pub use crate::traits::*;
    pub use crate::{SilexError, SilexResult};
    pub use crate::{batch_read, batch_read_untracked, rx};
}

/// Multi-signal batch read macro for zero-copy access to multiple signals.
///
/// This macro provides a way to access multiple signals without cloning, by nesting
/// the closures internally. All signals will be tracked for reactive updates.
///
/// # Example
/// ```rust,ignore
/// let name = signal("Alice".to_string());
/// let age = signal(42);
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
