extern crate self as silex;

pub mod components;
pub mod flow;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "persistence")]
pub mod persist;
pub mod router;
pub mod store;

pub use components::*;
pub use silex_core::error::{SilexError, SilexResult};

pub mod reexports {
    #[cfg(feature = "net")]
    pub use gloo_timers;
    pub use js_sys;
    #[cfg(feature = "json")]
    pub use serde_json;
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

pub mod css {
    pub use silex_css::*;
}

#[cfg(feature = "macros")]
pub mod macros {
    pub use silex_macros::*;
}

pub mod dom {
    pub use silex_dom::*;
}

pub mod hash {
    pub use silex_hash::*;
}

pub mod prelude {
    pub use crate::components::*;
    pub use crate::flow::*;
    #[cfg(feature = "net")]
    pub use crate::net::*;
    #[cfg(feature = "persistence")]
    pub use crate::persist::*;
    pub use crate::router::*;
    pub use crate::store::*;
    pub use crate::{SilexError, SilexResult};
    pub use silex_core::prelude::*;
    pub use silex_css::prelude::*;
    pub use silex_dom::prelude::*;
    pub use silex_html::*;
    #[cfg(feature = "macros")]
    pub use silex_macros::*;

    // Resolve ambiguous glob re-exports
    pub use crate::components::Center;
    pub use crate::core::prelude::{Map, RxWrite};
    pub use crate::flow::Switch;
    pub use crate::router::Link;
    pub use silex_css::prelude::{Style, linear_gradient, radial_gradient};
    pub use silex_dom::prelude::{View, text};
    pub use silex_html::{Em, em};
    #[cfg(feature = "macros")]
    pub use silex_macros::{define_theme, style};
}
