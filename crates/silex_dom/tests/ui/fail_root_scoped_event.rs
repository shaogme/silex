use silex_dom::{
    attribute::GlobalEventAttributes,
    element::Element,
    view::{RootViewOwner, View},
};
use web_sys::Node;

fn root_mount(owner: &RootViewOwner, parent: &Node, borrowed_ref: &str) {
    let view = Element::new("button").on_click(move |_| {
        assert!(!borrowed_ref.is_empty());
    });
    view.mount_owned(owner, parent, Vec::new());
}

fn main() {}
