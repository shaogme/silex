mod tags {
    pub mod html;
    pub mod svg;
}

pub mod attributes;
pub use attributes::*;

pub use tags::{html, svg};

pub use tags::html::*;
pub use tags::svg::*;
