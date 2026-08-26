use crate::kernel::View;
/// HTML/SVG tag 的命名空间元数据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagNamespace {
    Html,
    Svg,
}

/// 由 HTML/SVG codegen 生成的稳定 tag 元数据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagMetadata {
    pub name: &'static str,
    pub namespace: TagNamespace,
    pub is_void: bool,
}

impl TagMetadata {
    pub const fn new(name: &'static str, namespace: TagNamespace, is_void: bool) -> Self {
        Self {
            name,
            namespace,
            is_void,
        }
    }
}

/// Tag marker。它只携带 View metadata 和 capability marker，不携带 browser 类型。
pub trait Tag {
    const METADATA: TagMetadata;
}

pub trait FormTag: Tag {}
pub trait LabelTag: Tag {}
pub trait AnchorTag: Tag {}
pub trait MediaTag: Tag {}
pub trait TextTag: Tag {}
pub trait OpenTag: Tag {}
pub trait TableCellTag: Tag {}
pub trait TableHeaderTag: Tag {}
pub trait SvgTag: Tag {}

/// 定义不携带 browser concrete type 的 View tag marker 和 builder。
#[macro_export]
macro_rules! define_tag {
    ($struct_name:ident, $tag_name:literal, html, $fn_name:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::__private::Tag for $struct_name {
            const METADATA: $crate::__private::TagMetadata =
                $crate::__private::TagMetadata::new($tag_name, $crate::__private::TagNamespace::Html, true);
        }
        $(impl $crate::__private::$traits for $struct_name {})*
        pub fn $fn_name<'scope>() -> $crate::__private::TypedElement<'scope, $struct_name> {
            $crate::__private::TypedElement::from_tag()
        }
    };
    ($struct_name:ident, $tag_name:literal, html, $fn_name:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::__private::Tag for $struct_name {
            const METADATA: $crate::__private::TagMetadata =
                $crate::__private::TagMetadata::new($tag_name, $crate::__private::TagNamespace::Html, false);
        }
        $(impl $crate::__private::$traits for $struct_name {})*
        pub fn $fn_name<'scope, V>(child: V) -> $crate::__private::TypedElement<'scope, $struct_name>
        where
            V: $crate::__private::View<'scope> + 'scope,
        {
            $crate::__private::TypedElement::with_child_from_tag(child)
        }
    };
    ($struct_name:ident, $tag_name:literal, svg, $fn_name:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::__private::Tag for $struct_name {
            const METADATA: $crate::__private::TagMetadata =
                $crate::__private::TagMetadata::new($tag_name, $crate::__private::TagNamespace::Svg, true);
        }
        $(impl $crate::__private::$traits for $struct_name {})*
        pub fn $fn_name<'scope>() -> $crate::__private::TypedElement<'scope, $struct_name> {
            $crate::__private::TypedElement::from_tag()
        }
    };
    ($struct_name:ident, $tag_name:literal, svg, $fn_name:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::__private::Tag for $struct_name {
            const METADATA: $crate::__private::TagMetadata =
                $crate::__private::TagMetadata::new($tag_name, $crate::__private::TagNamespace::Svg, false);
        }
        $(impl $crate::__private::$traits for $struct_name {})*
        pub fn $fn_name<'scope, V>(child: V) -> $crate::__private::TypedElement<'scope, $struct_name>
        where
            V: $crate::__private::View<'scope> + 'scope,
        {
            $crate::__private::TypedElement::with_child_from_tag(child)
        }
    };
}

pub fn text<'scope, V: View<'scope>>(content: V) -> V {
    content
}
