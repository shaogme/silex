use silex_view::{
    AttributeBuilder,
    attribute::IntoStorable,
    element::{
        AnchorTag, FormTag, LabelTag, MediaTag, OpenTag, TableCellTag, TableHeaderTag, Tag,
        TypedElement,
    },
};

/// Carries the concrete HTML marker needed by tag-restricted attribute facades.
pub trait HtmlTagCarrier {
    type Tag: Tag;
}

impl<'scope, T> HtmlTagCarrier for TypedElement<'scope, T>
where
    T: Tag,
{
    type Tag = T;
}

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

macro_rules! define_restricted_attribute_group {
    ($trait_name:ident: $tag_trait:ident { $($method:ident => $name:literal),* $(,)? }) => {
        pub trait $trait_name<'scope>: AttributeBuilder<'scope> {
            $(
                fn $method(self, value: impl IntoStorable<'scope>) -> Self {
                    self.attr($name, value)
                }
            )*
        }

        impl<'scope, T> $trait_name<'scope> for T
        where
            T: AttributeBuilder<'scope> + HtmlTagCarrier,
            <T as HtmlTagCarrier>::Tag: $tag_trait,
        {}
    };
}

define_restricted_attribute_group!(FormAttributes: FormTag {
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

define_restricted_attribute_group!(LabelAttributes: LabelTag { for_ => "for" });

define_restricted_attribute_group!(AnchorAttributes: AnchorTag {
    href => "href",
    target => "target",
    rel => "rel",
    download => "download"
});

define_restricted_attribute_group!(MediaAttributes: MediaTag {
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

define_restricted_attribute_group!(OpenAttributes: OpenTag { open => "open" });

define_restricted_attribute_group!(TableCellAttributes: TableCellTag {
    colspan => "colspan",
    rowspan => "rowspan",
    headers => "headers"
});

define_restricted_attribute_group!(TableHeaderAttributes: TableHeaderTag {
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

// Popover target semantics are intentionally generic until a dedicated marker
// taxonomy is introduced; callers can use all three names on any builder.
define_attribute_group!(PopoverAttributes {
    popover => "popover",
    popovertarget => "popovertarget",
    popovertargetaction => "popovertargetaction"
});
