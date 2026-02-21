pub mod components;
pub mod css;
pub mod flow;
pub mod router;
pub mod store;

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

#[cfg(feature = "macros")]
pub mod macros {
    pub use silex_macros::*;
}

pub mod dom {
    pub use silex_dom::*;
}

pub mod prelude {
    pub use crate::components::*;
    pub use crate::core::prelude::*;
    pub use crate::core::*;
    pub use crate::flow::*;
    pub use crate::router::*;
    pub use crate::store::*;
    pub use crate::{SilexError, SilexResult};
    pub use silex_core::rx;
    pub use silex_dom::*;
    pub use silex_html::*;
    #[cfg(feature = "macros")]
    pub use silex_macros::*;

    // Export CSS types for easier use in styled! / css! macros
    pub use crate::css::types::{
        AlignItemsKeyword, BorderStyleKeyword, BorderValue, CursorKeyword, DisplayKeyword,
        FlexDirectionKeyword, FlexWrapKeyword, FontWeightKeyword, Hex, Hsl, JustifyContentKeyword,
        OverflowKeyword, Percent, PointerEventsKeyword, PositionKeyword, Px, Rem, Rgba,
        TextAlignKeyword, UnsafeCss, Url, Vh, VisibilityKeyword, Vw, border, hex, hsl, margin,
        padding, pct, px, rem, rgba, url, vh, vw,
    };

    pub use crate::css::builder::{Style, sty};

    // Resolve ambiguous glob re-exports
    pub use crate::core::prelude::{Map, Set, Track};
    pub use crate::flow::Switch;
    pub use crate::router::Link;
    pub use silex_dom::text;
    #[cfg(feature = "macros")]
    pub use silex_macros::style;
}
