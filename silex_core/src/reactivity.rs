pub use silex_reactivity::NodeId;
pub use silex_reactivity::{
    batch, create_scope, dispose, on_cleanup, provide_context, use_context,
};

mod effect;
mod memo;
mod mutation;
mod resource;
mod signal;
mod slice;
mod stored_value;

pub use effect::*;
pub use memo::*;
pub use mutation::*;
pub use resource::*;
pub use signal::*;
pub use slice::*;
pub use stored_value::*;

// --- Context ---

pub fn expect_context<T: Clone + 'static>() -> T {
    match use_context::<T>() {
        Some(v) => v,
        None => {
            let type_name = std::any::type_name::<T>();
            let msg = format!(
                "Expected context `{}` but none found. Did you forget to wrap your component in a Provider?",
                type_name
            );
            crate::log::console_error(&msg);
            panic!("{}", msg);
        }
    }
}
