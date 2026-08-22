use silex_dom::view::AnyView;
use silex_html::{PopoverAttributes, div};

fn main() {
    let _ = AnyView::new(div(()))
        .popover("auto")
        .popovertarget("dialog")
        .popovertargetaction("show");
}
