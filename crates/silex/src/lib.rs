extern crate self as silex;

pub mod components;
pub mod flow;
#[cfg(feature = "tw")]
pub mod ui;

pub use components::*;
pub use silex_core::error::{SilexError, SilexResult};
pub use silex_core::{
    Callback, NodeRef, OwnedScope, ReactiveError, ReactiveResult, RootHandle, Runtime, Rx, Scope,
};

pub mod reexports {
    pub use js_sys;
    #[cfg(feature = "json")]
    pub use serde_json;
    #[cfg(feature = "net")]
    pub use silex_net::reexports::gloo_timers;
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

pub mod macros {
    pub use silex_macros::*;
}

pub mod dom {
    pub use silex_dom::*;
}

pub mod hash {
    pub use silex_hash::*;
}

pub mod router {
    pub use silex_router::*;
}

#[cfg(feature = "persistence")]
pub mod persist {
    pub use silex_persist::*;
}

#[cfg(feature = "net")]
pub mod net {
    pub use silex_net::*;
}

#[cfg(feature = "i18n")]
pub mod i18n {
    pub use silex_i18n::*;
}

#[cfg(feature = "i18n")]
pub use crate::i18n::*;

pub mod prelude {
    pub use crate::components::*;
    pub use crate::flow::*;
    #[cfg(feature = "i18n")]
    pub use crate::i18n::*;
    #[cfg(feature = "net")]
    pub use crate::net::*;
    #[cfg(feature = "persistence")]
    pub use crate::persist::*;
    pub use crate::{ReactiveError, ReactiveResult, SilexError, SilexResult};
    pub use silex_core::prelude::*;
    pub use silex_css::prelude::*;
    pub use silex_dom::prelude::*;
    pub use silex_html::*;
    pub use silex_macros::*;
    pub use silex_router::*;

    // Resolve ambiguous glob re-exports
    #[cfg(feature = "css")]
    pub use crate::components::Center;
    pub use crate::core::prelude::{Map, RxWrite};
    pub use crate::flow::Switch;
    #[cfg(feature = "net")]
    pub use crate::net::reexports;
    pub use silex_css::prelude::{Style, linear_gradient, radial_gradient};
    #[cfg(feature = "tw")]
    pub use silex_css::prelude::{VariantSchema, declare_variants};
    pub use silex_dom::prelude::{ApplyAttributes, View, text};
    pub use silex_html::{Em, em};
    #[cfg(feature = "css")]
    pub use silex_macros::{global, inject_css, styled, theme};
    #[cfg(feature = "tw")]
    pub use silex_macros::{tw, tw_variants, tw_verbose};
    pub use silex_router::Link;
}
