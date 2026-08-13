// Marker traits and types for HTML tags
// This file defines the type-safe markers used by TypedElement<T>

/// Root trait for all tag markers
pub trait Tag {
    type DomElement: wasm_bindgen::JsCast
        + AsRef<web_sys::Element>
        + AsRef<web_sys::Node>
        + Clone
        + 'static;
}

// --- Group Traits (corresponding to props groups) ---

/// Tags that support form attributes (value, checked, type, etc.)
pub trait FormTag: Tag {}

/// Tags that support label attributes (for)
pub trait LabelTag: Tag {}

/// Tags that support anchor attributes (href, target, rel)
pub trait AnchorTag: Tag {}

/// Tags that support media attributes (src, alt, width, height)
pub trait MediaTag: Tag {}

/// Tags that can contain text content
pub trait TextTag: Tag {}

/// Tags that support the 'open' attribute (dialog, details)
pub trait OpenTag: Tag {}

/// Tags that are table cells (td, th) supporting colspan, rowspan
pub trait TableCellTag: Tag {}

/// Tags that are table headers (th) supporting scope, abbr
pub trait TableHeaderTag: Tag {}

// --- Tag Markers ---

// --- Tag Markers (Empty in Core) ---

// 6. SVG Tags Marker (Trait only)
pub trait SvgTag: Tag {}

// --- Macros ---

#[macro_export]
macro_rules! define_tag {
    ($struct_name:ident, $dom_elem:ty, $tag_name:literal, $fn_name:ident, $constructor:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::element::tags::Tag for $struct_name {
            type DomElement = $dom_elem;
        }
        $( impl $crate::element::tags::$traits for $struct_name {} )*

        pub fn $fn_name<'scope>() -> $crate::element::TypedElement<'scope, $struct_name> {
            $crate::element::TypedElement::$constructor($tag_name)
        }
    };

    ($struct_name:ident, $dom_elem:ty, $tag_name:literal, $fn_name:ident, $constructor:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::element::tags::Tag for $struct_name {
            type DomElement = $dom_elem;
        }
        $( impl $crate::element::tags::$traits for $struct_name {} )*

        pub fn $fn_name<'scope, V>(child: V) -> $crate::element::TypedElement<'scope, $struct_name>
        where
            V: $crate::view::ViewFactory<'scope> + 'scope,
        {
            $crate::element::TypedElement::with_child($tag_name, child)
        }
    };

    ($struct_name:ident, $tag_name:literal, $fn_name:ident, $constructor:ident, $void_kind:ident, [$($traits:ident),*]) => {
        $crate::define_tag!($struct_name, web_sys::HtmlElement, $tag_name, $fn_name, $constructor, $void_kind, [$($traits),*]);
    };
}
