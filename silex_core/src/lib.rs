pub mod callback;
pub mod error;
pub mod log;
pub mod node_ref;
pub mod reactivity;

pub mod traits;

pub use callback::Callback;
pub use error::{SilexError, SilexResult};
pub use node_ref::NodeRef;

pub struct RxValue;
pub struct RxEffect;

/// 响应式计算单元或事件处理器。
/// Rx 始终不应该要求实现 Clone trait 或 Copy trait。
pub struct Rx<F, M = RxValue>(pub F, pub ::core::marker::PhantomData<M>);

impl<F: Clone, M> Clone for Rx<F, M> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

impl<F: Copy, M> Copy for Rx<F, M> {}

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
