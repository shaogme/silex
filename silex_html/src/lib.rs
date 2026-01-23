use silex_dom::tags::*;
use silex_dom::view::View;
use silex_dom::{Tag, TypedElement};

// --- Tag Definitions (Structs) ---

macro_rules! define_tags {
    // Basic tags
    (@basic $($tag:ident),*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $tag;
            impl Tag for $tag {}
        )*
    };

    // Tags with specific traits
    (@impl $trait:ident for $($tag:ident),*) => {
        $(
            impl $trait for $tag {}
        )*
    };
}

// 1. Basic Structure Tags
define_tags!(@basic
    Div, Span, H1, H2, H3, H4, H5, H6, P, Ul, Ol, Li,
    Nav, Main, Footer, Aside, Header, Article, Section, Address,
    Br, Hr, Table, Thead, Tbody, Tr, Td, Pre, Code,
    Em, Strong, S, Blockquote, Figure, Figcaption,
    Time
);
define_tags!(@impl TextTag for
    Div, Span, H1, H2, H3, H4, H5, H6, P, Ul, Ol, Li,
    Nav, Main, Footer, Aside, Header, Article, Section, Address,
    Table, Thead, Tbody, Tr, Td, Pre, Code,
    Em, Strong, S, Blockquote, Figure, Figcaption,
    Time
);

// 2. Form Tags
define_tags!(@basic Input, Button, Form, Select, Textarea, OptionTag);
define_tags!(@impl FormTag for Input, Button, Form, Select, Textarea, OptionTag);
define_tags!(@impl TextTag for Button, Form, Select, Textarea, OptionTag);

// 3. Label Tag
define_tags!(@basic Label);
define_tags!(@impl LabelTag for Label);
define_tags!(@impl TextTag for Label);

// 4. Anchor Tags
define_tags!(@basic A, Area, Link);
define_tags!(@impl AnchorTag for A, Area, Link);
define_tags!(@impl TextTag for A);

// 5. Media Tags
define_tags!(@basic Img, Video, Audio, Source, Iframe);
define_tags!(@impl MediaTag for Img, Video, Audio, Source, Iframe);
define_tags!(@impl TextTag for Video, Audio, Iframe);

// 6. SVG Tags
define_tags!(@basic
    Svg, Path, Defs, Filter, G, Rect, Circle, Line, Polyline, Polygon,
    FeTurbulence, FeComponentTransfer, FeFuncR, FeFuncG, FeFuncB,
    FeGaussianBlur, FeSpecularLighting, FePointLight, FeComposite, FeDisplacementMap
);
define_tags!(@impl SvgTag for
    Svg, Path, Defs, Filter, G, Rect, Circle, Line, Polyline, Polygon,
    FeTurbulence, FeComponentTransfer, FeFuncR, FeFuncG, FeFuncB,
    FeGaussianBlur, FeSpecularLighting, FePointLight, FeComposite, FeDisplacementMap
);
define_tags!(@impl TextTag for
    Svg, Path, Defs, Filter, G, Rect, Circle, Line, Polyline, Polygon,
    FeTurbulence, FeComponentTransfer, FeFuncR, FeFuncG, FeFuncB,
    FeGaussianBlur, FeSpecularLighting, FePointLight, FeComposite, FeDisplacementMap
);

// --- Functions ---

macro_rules! define_container {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        pub fn $fn_name<V: View>(child: V) -> TypedElement<$tag_type> {
            TypedElement::new($tag_str).child(child)
        }
    };
}

macro_rules! define_void {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        pub fn $fn_name() -> TypedElement<$tag_type> {
            TypedElement::new($tag_str)
        }
    };
}

macro_rules! define_svg_container {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        pub fn $fn_name<V: View>(child: V) -> TypedElement<$tag_type> {
            TypedElement::new_svg($tag_str).child(child)
        }
    };
}

macro_rules! define_svg_void {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        pub fn $fn_name() -> TypedElement<$tag_type> {
            TypedElement::new_svg($tag_str)
        }
    };
}

// HTML Containers
define_container!(div, Div, "div");
define_container!(span, Span, "span");
define_container!(p, P, "p");
define_container!(h1, H1, "h1");
define_container!(h2, H2, "h2");
define_container!(h3, H3, "h3");
define_container!(h4, H4, "h4");
define_container!(h5, H5, "h5");
define_container!(h6, H6, "h6");

define_container!(header, Header, "header");
define_container!(footer, Footer, "footer");
define_container!(main, Main, "main");
define_container!(section, Section, "section");
define_container!(article, Article, "article");
define_container!(aside, Aside, "aside");
define_container!(nav, Nav, "nav");
define_container!(address, Address, "address");

define_container!(ul, Ul, "ul");
define_container!(ol, Ol, "ol");
define_container!(li, Li, "li");

define_container!(a, A, "a");
define_container!(button, Button, "button");
define_container!(label, Label, "label");
define_container!(pre, Pre, "pre");
define_container!(code, Code, "code");
define_container!(blockquote, Blockquote, "blockquote");
define_container!(em, Em, "em");
define_container!(strong, Strong, "strong");
define_container!(s, S, "s");
define_container!(time, Time, "time");
define_container!(figure, Figure, "figure");
define_container!(figcaption, Figcaption, "figcaption");

define_container!(form, Form, "form");
define_container!(select, Select, "select");
define_container!(textarea, Textarea, "textarea");

pub fn option<V: View>(child: V) -> TypedElement<OptionTag> {
    TypedElement::new("option").child(child)
}

define_container!(table, Table, "table");
define_container!(thead, Thead, "thead");
define_container!(tbody, Tbody, "tbody");
define_container!(tr, Tr, "tr");
define_container!(td, Td, "td");

// HTML Voids
define_void!(input, Input, "input");
define_void!(img, Img, "img");
define_void!(br, Br, "br");
define_void!(hr, Hr, "hr");
define_void!(link, Link, "link");

// SVG Containers
define_svg_container!(svg, Svg, "svg");
define_svg_container!(g, G, "g");
define_svg_container!(defs, Defs, "defs");
define_svg_container!(filter, Filter, "filter");

// SVG Voids
define_svg_void!(path, Path, "path");
define_svg_void!(rect, Rect, "rect");
define_svg_void!(circle, Circle, "circle");
define_svg_void!(line, Line, "line");
define_svg_void!(polyline, Polyline, "polyline");
define_svg_void!(polygon, Polygon, "polygon");

define_svg_void!(fe_turbulence, FeTurbulence, "feTurbulence");
define_svg_void!(
    fe_component_transfer,
    FeComponentTransfer,
    "feComponentTransfer"
);
define_svg_void!(fe_func_r, FeFuncR, "feFuncR");
define_svg_void!(fe_func_g, FeFuncG, "feFuncG");
define_svg_void!(fe_func_b, FeFuncB, "feFuncB");
define_svg_void!(fe_gaussian_blur, FeGaussianBlur, "feGaussianBlur");
define_svg_void!(
    fe_specular_lighting,
    FeSpecularLighting,
    "feSpecularLighting"
);
define_svg_void!(fe_point_light, FePointLight, "fePointLight");
define_svg_void!(fe_composite, FeComposite, "feComposite");
define_svg_void!(fe_displacement_map, FeDisplacementMap, "feDisplacementMap");

// --- Macros ---

#[macro_export]
macro_rules! define_tag_macros {
    ($($name:ident),+; $d:tt) => {
        $(
            #[macro_export]
            macro_rules! $name {
                () => {
                    $crate::$name(())
                };
                ($d($d child:expr),+ $d(,)?) => {
                    $crate::$name(($d($d child),+))
                };
            }
        )*
    };
}

define_tag_macros!(
    div, span, p, h1, h2, h3, h4, h5, h6,
    header, footer, main, section, article, aside, nav, address,
    ul, ol, li,
    a, button, label, pre, code, blockquote, em, strong, s, time, figure, figcaption,
    form, select, textarea, option,
    table, thead, tbody, tr, td,
    svg, g, defs, filter
    ; $
);
