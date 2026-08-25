//! Silex 的高层 View kernel。
//!
//! 本 crate 只依赖通用 runtime (`silex_core`) 和物理 DOM backend
//! (`silex_dom`)。所有节点、属性、事件和宿主资源都通过显式注入的
//! [`DomContext`] 操作，View 层不携带 browser concrete type。

pub mod any;
pub mod attribute;
pub mod context;
pub mod contract;
pub mod dynamic;
pub mod element;
pub mod error;
pub mod event;
pub mod helpers;
pub mod list;
mod mount;
pub mod mounted;
pub mod owner;
pub mod reactive;
pub mod row;

pub use any::AnyView;
pub use attribute::{
    ApplyTarget, Attr, AttrOp, AttributeBuilder, AttributeGroup, IntoStorable, KnownProp,
    update_class_tokens,
};
pub use context::{
    MountAncestry, MountContext, MountDomAction, MountTarget, MountTransaction,
    MountTransactionState,
};
pub use contract::{
    MountInstance, Prop, PropFixed, PropInto, PropMissing, View, ViewCons, ViewNil,
};
pub use dynamic::{BranchEvaluation, BranchRenderContext, DynamicRenderer, StableBranch};
pub use element::{Element, Tag, TagMetadata, TagNamespace, TypedElement, text};
pub use error::{DisposeError, MountAvailability, MountError, RollbackError, ViewError};
pub use event::{
    DomEvent, DomRectData, Event, EventDescriptor, EventHandler, EventKind, EventSpec,
    MouseEventData, PointerEventData, WithEventArg, WithoutEventArg, bind_window_event,
};
pub use list::{IndexedListView, RenderOnlyKeyedListView, StatefulKeyedListView};
pub use mounted::{MountBuilderContext, MountedApp};
pub use owner::{
    MountCleanup, MountEffect, MountErrorHandler, MountOwner, MountOwnerContext, MountOwnerToken,
    MountState, SharedCell,
};
pub use reactive::AutoReactiveView;
pub use row::RowUpdater;

pub use silex_dom::host::HostResource;
pub use silex_dom::ssr::SsrDom;
pub use silex_dom::{DomContext, DomElement, DomNode, DomRange};

/// 高层 View API 的推荐导入集合。
pub mod prelude {
    pub use crate::attribute::{
        AriaAttributes, AttributeGroup, GlobalAttributes, GlobalEventAttributes, IntoStorable,
    };
    pub use crate::{
        AnyView, Attr, AttrOp, AttributeBuilder, BranchEvaluation, BranchRenderContext, DomContext,
        DomElement, DomEvent, DomNode, DomRectData, Element, Event, EventDescriptor, EventKind,
        EventSpec, HostResource, MountAncestry, MountBuilderContext, MountContext, MountDomAction,
        MountInstance, MountOwner, MountOwnerToken, MountState, MountedApp, PointerEventData, Prop,
        PropFixed, PropMissing, RowUpdater, Tag, TypedElement, View, ViewCons, ViewNil, chain,
        event,
    };
}
