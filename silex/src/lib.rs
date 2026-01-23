pub mod components;
pub mod css;
pub mod flow;
pub mod router;

pub use components::*;
pub use silex_core::error::{SilexError, SilexResult};

pub mod reexports {
    pub use js_sys;
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

pub mod core {
    pub use silex_core::*;
}

pub mod html {
    pub use silex_html::*;
}

pub mod prelude {
    pub use crate::components::*;
    pub use crate::core::prelude::*;
    pub use crate::core::*;
    pub use crate::flow::*;
    pub use crate::router::Link;
    pub use crate::router::*;
    pub use crate::{SilexError, SilexResult};
    pub use silex_core::rx;
    pub use silex_html::*;
}
