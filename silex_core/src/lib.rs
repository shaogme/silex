pub mod callback;
pub mod error;
pub mod log;
pub mod node_ref;
pub mod reactivity;

pub mod traits;

pub use callback::Callback;
pub use error::{SilexError, SilexResult};
pub use node_ref::NodeRef;

/// `rx!` 宏：简化创建响应式闭包的语法。
///
/// 等同于 `move || { ... }`。
///
/// # 示例
/// ```rust
/// use silex_core::rx;
/// use silex_core::prelude::*;
///
/// let (count, set_count) = signal(0);
/// let double = rx!(count.get() * 2);
/// ```
#[macro_export]
macro_rules! rx {
    ($($expr:tt)*) => {
        move || { $($expr)* }
    };
}

pub mod prelude {
    pub use crate::log::*;
    pub use crate::node_ref::NodeRef;
    pub use crate::reactivity::*;
    pub use crate::traits::*;
    pub use crate::{SilexError, SilexResult};
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
    // Two signals
    ($s1:expr, $s2:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty| $body:expr) => {{
        use $crate::traits::With;
        ($s1).with(|$p1: $t1| ($s2).with(|$p2: $t2| $body))
    }};
    // Three signals
    ($s1:expr, $s2:expr, $s3:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty, $p3:ident: $t3:ty| $body:expr) => {{
        use $crate::traits::With;
        ($s1).with(|$p1: $t1| ($s2).with(|$p2: $t2| ($s3).with(|$p3: $t3| $body)))
    }};
    // Four signals
    ($s1:expr, $s2:expr, $s3:expr, $s4:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty, $p3:ident: $t3:ty, $p4:ident: $t4:ty| $body:expr) => {{
        use $crate::traits::With;
        ($s1).with(|$p1: $t1| {
            ($s2).with(|$p2: $t2| ($s3).with(|$p3: $t3| ($s4).with(|$p4: $t4| $body)))
        })
    }};
}

/// Untracked version of batch_read - does not subscribe to signal changes.
#[macro_export]
macro_rules! batch_read_untracked {
    // Two signals
    ($s1:expr, $s2:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty| $body:expr) => {{
        use $crate::traits::WithUntracked;
        ($s1).with_untracked(|$p1: $t1| ($s2).with_untracked(|$p2: $t2| $body))
    }};
    // Three signals
    ($s1:expr, $s2:expr, $s3:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty, $p3:ident: $t3:ty| $body:expr) => {{
        use $crate::traits::WithUntracked;
        ($s1).with_untracked(|$p1: $t1| {
            ($s2).with_untracked(|$p2: $t2| ($s3).with_untracked(|$p3: $t3| $body))
        })
    }};
    // Four signals
    ($s1:expr, $s2:expr, $s3:expr, $s4:expr => |$p1:ident: $t1:ty, $p2:ident: $t2:ty, $p3:ident: $t3:ty, $p4:ident: $t4:ty| $body:expr) => {{
        use $crate::traits::WithUntracked;
        ($s1).with_untracked(|$p1: $t1| {
            ($s2).with_untracked(|$p2: $t2| {
                ($s3).with_untracked(|$p3: $t3| ($s4).with_untracked(|$p4: $t4| $body))
            })
        })
    }};
}
