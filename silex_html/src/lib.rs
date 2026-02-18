mod tags {
    pub mod html;
    pub mod svg;
}

pub use tags::{html, svg};

pub use tags::html::*;
pub use tags::svg::*;
