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

/// `rx!` 宏：创建响应式计算单元或事件处理器。
///
/// 支持多种形式：
/// - `rx!(expression)`: 创建无参计算单元。
/// - `rx!(|args| body)`: 创建带参数的计算单元或事件处理器。
#[macro_export]
#[doc(hidden)]
macro_rules! rx_effect {
    // 带类型标注的单参数
    (move | $arg:ident : $ty:ty | $($body:tt)*) => {
        $crate::Rx(move |$arg: $ty| { $($body)* }, ::core::marker::PhantomData::<$crate::RxEffect>)
    };
    (| $arg:ident : $ty:ty | $($body:tt)*) => {
        $crate::Rx(move |$arg: $ty| { $($body)* }, ::core::marker::PhantomData::<$crate::RxEffect>)
    };
    // 不带类型的单参数
    (move | $arg:ident | $($body:tt)*) => {
        $crate::Rx(move |$arg| { $($body)* }, ::core::marker::PhantomData::<$crate::RxEffect>)
    };
    (| $arg:ident | $($body:tt)*) => {
        $crate::Rx(move |$arg| { $($body)* }, ::core::marker::PhantomData::<$crate::RxEffect>)
    };
}

#[macro_export]
macro_rules! rx {
    // 1. 匹配 move || -> Value (Getter)
    (move || $($body:tt)*) => {
        $crate::Rx(move || { $($body)* }, ::core::marker::PhantomData::<$crate::RxValue>)
    };
    // 2. 匹配 || -> Value (Getter)
    (|| $($body:tt)*) => {
        $crate::Rx(move || { $($body)* }, ::core::marker::PhantomData::<$crate::RxValue>)
    };
    // 3. 匹配带 move 的带参数闭包 -> Effect
    (move | $($rest:tt)*) => {
        $crate::rx_effect!(move | $($rest)*)
    };
    // 4. 匹配不带 move 的带参数闭包 -> Effect
    (| $($rest:tt)*) => {
        $crate::rx_effect!(| $($rest)*)
    };
    // 5. 匹配普通表达式 -> Value
    ($($expr:tt)*) => {
        $crate::Rx(move || { $($expr)* }, ::core::marker::PhantomData::<$crate::RxValue>)
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
        use $crate::traits::With;
        ($s1).with(|$p1: $t1| $body)
    }};
    ([$s1:expr, $($ss:expr),+] => [$p1:ident: $t1:ty, $($ps:ident: $ts:ty),+] => $body:expr) => {{
        use $crate::traits::With;
        ($s1).with(|$p1: $t1| $crate::batch_read_recurse!([$($ss),+] => [$($ps: $ts),+] => $body))
    }};
}

/// Untracked version of batch_read - does not subscribe to signal changes.
#[macro_export]
macro_rules! batch_read_untracked {
    // 递归实现
    ([$s1:expr] => [$p1:ident: $t1:ty] => $body:expr) => {{
        use $crate::traits::WithUntracked;
        ($s1).with_untracked(|$p1: $t1| $body)
    }};
    ([$s1:expr, $($ss:expr),+] => [$p1:ident: $t1:ty, $($ps:ident: $ts:ty),+] => $body:expr) => {{
        use $crate::traits::WithUntracked;
        ($s1).with_untracked(|$p1: $t1| $crate::batch_read_untracked!([$($ss),+] => [$($ps: $ts),+] => $body))
    }};
    // 包装器，支持外部调用的逗号分隔语法
    ($($s:expr),+ => |$($p:ident: $t:ty),+| $body:expr) => {
        $crate::batch_read_untracked!([$($s),+] => [$($p: $t),+] => $body)
    };
}
