mod tags {
    pub mod html;
    pub mod svg;
}

pub use silex_dom::view::{ViewCons, ViewNil};
pub use silex_dom::view_chain;

pub mod attributes;
pub use attributes::*;

pub use tags::{html, svg};

pub use tags::html::*;
pub use tags::svg::*;
