pub mod dom;
pub mod error;
pub mod flow;
pub mod log;
pub mod reactivity;

pub mod css;
pub mod router;

pub use error::{SilexError, SilexResult};

pub mod prelude {
    pub use crate::dom::*;
    pub use crate::flow::*;
    pub use crate::log::*;
    pub use crate::reactivity::{
        Accessor, ReadSignal, Resource, RwSignal, WriteSignal, create_effect, create_memo,
        create_resource, create_rw_signal, create_scope, create_signal, expect_context, on_cleanup,
        provide_context, use_context,
    };
    pub use crate::router::*;
    pub use crate::view_match;
    pub use crate::{SilexError, SilexResult};
}
