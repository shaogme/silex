pub mod dom;
pub mod error;
pub mod flow;
pub mod logging;
pub mod reactivity;

pub mod router;

pub use error::{SilexError, SilexResult};
pub use silex_macros::{Store, component};

pub mod prelude {
    pub use crate::Store;
    pub use crate::component;
    pub use crate::dom::*;
    pub use crate::error::{SilexError, SilexResult};
    pub use crate::flow::*;
    pub use crate::reactivity::{
        ReadSignal, Resource, RwSignal, WriteSignal, create_effect, create_memo, create_resource,
        create_rw_signal, create_scope, create_signal, expect_context, on_cleanup, provide_context,
        use_context,
    };
    pub use crate::router::*;
}
