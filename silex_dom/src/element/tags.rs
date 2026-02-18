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
    ($struct_name:ident, $tag_name:literal, $fn_name:ident, $constructor:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::tags::Tag for $struct_name {}
        $( impl $crate::tags::$traits for $struct_name {} )*

        pub fn $fn_name() -> $crate::TypedElement<$struct_name> {
            $crate::TypedElement::$constructor($tag_name)
        }
    };

    ($struct_name:ident, $tag_name:literal, $fn_name:ident, $constructor:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::tags::Tag for $struct_name {}
        $( impl $crate::tags::$traits for $struct_name {} )*

        pub fn $fn_name<V: $crate::view::View>(child: V) -> $crate::TypedElement<$struct_name> {
            let el = $crate::TypedElement::$constructor($tag_name);
            child.mount(&el.element.dom_element);
            el
        }
    };
}
