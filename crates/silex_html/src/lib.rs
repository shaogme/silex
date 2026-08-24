mod tags {
    pub mod html;
    pub mod svg;
}

pub use silex_view::chain;
pub use silex_view::{ViewCons, ViewNil};

pub mod attributes;
pub use attributes::*;

pub use tags::{html, svg};

pub use tags::html::*;
pub use tags::svg::*;
