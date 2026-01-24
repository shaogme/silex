pub use silex_reactivity::NodeId;
pub use silex_reactivity::{
    batch, create_scope, dispose, on_cleanup, provide_context, use_context,
};

pub mod effect;
pub mod memo;
pub mod resource;
pub mod signal;
pub mod stored_value;

pub use effect::*;
pub use memo::*;
pub use resource::*;
pub use signal::*;
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
