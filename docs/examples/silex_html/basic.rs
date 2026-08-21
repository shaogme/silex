use std::error::Error;

use silex_dom::attribute::{AriaAttributes, AttributeBuilder, GlobalAttributes};
use silex_html::{
    AnchorAttributes, FormAttributes, MediaAttributes, a, button, div, g, img, input, path, svg,
};

pub fn run() -> Result<(), Box<dyn Error>> {
    let form_view = div!(
        input()
            .type_("search")
            .name("query")
            .placeholder("Search")
            .aria_label("Search"),
        button!("Submit"),
    )
    .id("search-panel")
    .class("panel");

    let media_view = img().src("/logo.svg").alt("Silex");
    let link_view = a("Documentation").href("/docs");
    let icon_view = svg(path().attr("d", "M0 0"));
    let macro_icon_view = svg!(path());
    let macro_group_view = g!(path());

    let _ = (
        form_view,
        media_view,
        link_view,
        icon_view,
        macro_icon_view,
        macro_group_view,
    );
    Ok(())
}
