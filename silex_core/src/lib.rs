pub mod callback;
pub mod error;
pub mod log;
pub mod reactivity;

pub use callback::Callback;
pub use error::{SilexError, SilexResult};

/// `rx!` 宏：简化创建响应式闭包的语法。
///
/// 等同于 `move || { ... }`。
///
/// # 示例
/// ```rust
/// use silex_core::rx;
/// use silex_core::prelude::*;
///
/// let (count, set_count) = create_signal(0);
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
    pub use crate::reactivity::{
        Accessor, ReadSignal, Resource, RwSignal, WriteSignal, create_effect, create_memo,
        create_resource, create_rw_signal, create_scope, create_signal, expect_context, on_cleanup,
        provide_context, use_context,
    };
    pub use crate::{SilexError, SilexResult};
}
