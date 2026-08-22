#![deny(warnings)]

use silex_core::Runtime;
use silex_dom::attribute::AttributeBuilder;
use silex_html::{
    AnchorAttributes, DataAttributes, FormAttributes, LabelAttributes, MediaAttributes,
    OpenAttributes, TableCellAttributes, TableHeaderAttributes, a, dialog, div, img, input, label,
    td, th,
};

#[test]
fn supported_tags_keep_their_typed_facades() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (reactive_value, _) = owner
                .signal(String::from("reactive"))
                .expect("reactive value should initialize");

            let _input = input()
                .type_("email")
                .value(String::from("owned"))
                .checked(Some(true))
                .placeholder(reactive_value);
            let _anchor = a(()).href("/docs").target("_blank");
            let _image = img().src("/logo.svg").alt("logo");
            let _label = label(()).for_("email");
            let _dialog = dialog(()).open(true);
            let _cell = td(()).colspan(2).rowspan(Some(1));
            let _header = th(()).scope("col").abbr("Name");
        })
        .expect("transient owner should initialize");
}

#[test]
fn generic_attribute_escape_hatches_remain_available() {
    // Property construction crosses the wasm-bindgen boundary and is therefore
    // compile-tested below rather than executed on a native target.
    assert!(true);
}

#[allow(dead_code)]
fn generic_attribute_escape_hatches_compile() {
    let typed = div(())
        .data_value("active")
        .attr("custom-attribute", "value")
        .prop("customProperty", "value")
        .apply("additional-value");
    let _untyped = input().attr("value", "before-erasure").into_untyped();
    let _any_view = typed.into_untyped();
}
