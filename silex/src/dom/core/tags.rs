// Marker traits and types for HTML tags
// This file defines the type-safe markers used by TypedElement<T>

/// Root trait for all tag markers
pub trait Tag {}

// --- Group Traits (corresponding to props groups) ---

/// Tags that support form attributes (value, checked, type, etc.)
pub trait FormTag: Tag {}

/// Tags that support label attributes (for)
pub trait LabelTag: Tag {}

/// Tags that support anchor attributes (href, target, rel)
pub trait AnchorTag: Tag {}

/// Tags that support media attributes (src, alt, width, height)
pub trait MediaTag: Tag {}

// --- Tag Markers ---

macro_rules! define_tags {
    // Basic tags (no special traits besides Global)
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
    Nav, Main, Footer, Aside, Header, Article, Section,
    Br, Hr, Table, Thead, Tbody, Tr, Td, Pre, Code,
    Em, Strong, S, Blockquote, Figure, Figcaption,
    Time
);

// 2. Form Tags
define_tags!(@basic Input, Button, Form, Select, Textarea, OptionTag); // Option is a keyword, use OptionTag
define_tags!(@impl FormTag for Input, Button, Form, Select, Textarea, OptionTag);

// 3. Label Tag
define_tags!(@basic Label);
define_tags!(@impl LabelTag for Label);

// 4. Anchor Tags
define_tags!(@basic A, Area, Link);
define_tags!(@impl AnchorTag for A, Area, Link);

// 5. Media Tags
define_tags!(@basic Img, Video, Audio, Source, Iframe);
define_tags!(@impl MediaTag for Img, Video, Audio, Source, Iframe);

// 6. SVG Tags (Just treating them as generic tags for now, or we can add SvgTag marker)
pub trait SvgTag: Tag {}
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
