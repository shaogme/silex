use silex_dom::attribute::{AttributeBuilder, IntoStorable};

macro_rules! define_attribute_group {
    ($trait_name:ident { $($method:ident => $name:literal),* $(,)? }) => {
        pub trait $trait_name<'scope>: AttributeBuilder<'scope> {
            $(
                fn $method(self, value: impl IntoStorable<'scope>) -> Self {
                    self.attr($name, value)
                }
            )*
        }

        impl<'scope, T: AttributeBuilder<'scope>> $trait_name<'scope> for T {}
    };
}

define_attribute_group!(FormAttributes {
    type_ => "type",
    value => "value",
    checked => "checked",
    disabled => "disabled",
    readonly => "readonly",
    required => "required",
    placeholder => "placeholder",
    name => "name",
    autocomplete => "autocomplete",
    autofocus => "autofocus",
    min => "min",
    max => "max",
    step => "step",
    pattern => "pattern",
    multiple => "multiple",
    accept => "accept",
    selected => "selected",
    rows => "rows",
    cols => "cols",
    action => "action",
    method => "method",
    form => "form",
    novalidate => "novalidate",
    formaction => "formaction",
    formenctype => "formenctype",
    formmethod => "formmethod",
    formnovalidate => "formnovalidate",
    formtarget => "formtarget"
});

define_attribute_group!(LabelAttributes { for_ => "for" });

define_attribute_group!(AnchorAttributes {
    href => "href",
    target => "target",
    rel => "rel",
    download => "download"
});

define_attribute_group!(MediaAttributes {
    src => "src",
    alt => "alt",
    width => "width",
    height => "height",
    autoplay => "autoplay",
    controls => "controls",
    loop_ => "loop",
    muted => "muted",
    poster => "poster",
    preload => "preload",
    srcset => "srcset",
    sizes => "sizes",
    loading => "loading",
    decoding => "decoding",
    crossorigin => "crossorigin"
});

define_attribute_group!(OpenAttributes { open => "open" });

define_attribute_group!(TableCellAttributes {
    colspan => "colspan",
    rowspan => "rowspan",
    headers => "headers"
});

define_attribute_group!(TableHeaderAttributes {
    scope => "scope",
    abbr => "abbr"
});

define_attribute_group!(DataAttributes {
    data_slot => "data-slot",
    data_state => "data-state",
    data_orientation => "data-orientation",
    data_disabled => "data-disabled",
    data_value => "data-value",
    data_side => "data-side",
    data_align => "data-align",
    data_active => "data-active",
    data_open => "data-open"
});

define_attribute_group!(PopoverAttributes {
    popover => "popover",
    popovertarget => "popovertarget",
    popovertargetaction => "popovertargetaction"
});
