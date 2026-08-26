mod erased;
mod tag;
mod typed;
mod untyped;

pub use erased::AnyView;
pub use tag::text;
pub use tag::{
    AnchorTag, FormTag, LabelTag, MediaTag, OpenTag, SvgTag, TableCellTag, TableHeaderTag, Tag,
    TagMetadata, TagNamespace, TextTag,
};
pub use typed::TypedElement;
pub use untyped::Element;
