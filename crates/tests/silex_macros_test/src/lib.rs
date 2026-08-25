#[allow(unused_extern_crates)]
extern crate self as silex;

pub mod core {
    pub use silex_core::*;
}

pub mod css {
    pub use silex_css::*;
}

pub mod html {
    pub use silex_html::*;
}

pub mod macros {
    pub use silex_macros::*;
}

pub mod reexports {
    pub use web_sys;
}
