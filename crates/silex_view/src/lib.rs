//! Silex 的高层 View kernel。
//!
//! 本 crate 只依赖通用 runtime (`silex_core`) 和物理 DOM backend
//! (`silex_dom`)。所有节点、属性、事件和宿主资源都通过显式注入的
//! [`DomContext`] 操作，View 层不携带 browser concrete type。

pub mod app;
pub mod flow;
mod kernel;
pub mod lifecycle;

pub mod attributes {
    pub use crate::kernel::attributes::{
        ApplyTarget, ApplyToDom, AriaAttributes, Attr, AttrData, AttrOp, AttrPhase, AttrUpdate,
        AttributeBuilder, AttributeGroup, CombinedClasses, CombinedStyles, GlobalAttributes,
        GlobalEventAttributes, IntoStorable, KnownProp, ReactiveBinding, ReactiveBindingContext,
        ReactiveBindingPlan, ReactiveBindingTarget, consolidate_attributes, group, set_class_value,
        update_class_tokens,
    };
}

pub mod elements {
    pub use crate::kernel::elements::{
        AnchorTag, AnyView, Element, FormTag, LabelTag, MediaTag, OpenTag, SvgTag, TableCellTag,
        TableHeaderTag, Tag, TagMetadata, TagNamespace, TextTag, TypedElement, text,
    };
}

pub mod errors {
    //! View mount 生命周期错误。

    pub use silex_core::error::view::{
        DisposeError, MountAvailability, MountError, RollbackError, ViewError,
    };
}

pub mod events {
    pub use crate::kernel::events::{
        DomEvent, DomRectData, Event, EventDescriptor, EventHandler, EventKind, EventSpec,
        MouseEventData, PointerEventData, WindowEventRequest, WithEventArg, WithoutEventArg,
        bind_event, bind_window_event, blur, change, click, dblclick, event_target, focus, input,
        keydown, keyup, mouseenter, mouseleave, pointercancel, pointerdown, pointermove, pointerup,
        submit, wheel,
    };
}

pub mod mount {
    pub use crate::kernel::{
        MountAncestry, MountContext, MountDomAction, MountTarget, MountTransaction,
        MountTransactionState,
    };
    pub use crate::kernel::{
        MountInstance, Prop, PropFixed, PropInto, PropMissing, View, ViewCons, ViewNil,
    };
}

/// 高层 View API 的推荐导入集合。
pub mod prelude {
    pub use crate::app::{MountBuilderContext, MountedApp};
    pub use crate::attributes::{
        AriaAttributes, Attr, AttrOp, AttributeBuilder, AttributeGroup, GlobalAttributes,
        GlobalEventAttributes, IntoStorable, KnownProp,
    };
    pub use crate::chain;
    pub use crate::elements::{
        AnyView, Element, Tag, TagMetadata, TagNamespace, TypedElement, text,
    };
    pub use crate::errors::{
        DisposeError, MountAvailability, MountError, RollbackError, ViewError,
    };
    pub use crate::events::{
        DomEvent, DomRectData, Event, EventDescriptor, EventHandler, EventKind, EventSpec,
        MouseEventData, PointerEventData,
    };
    pub use crate::flow::{
        AutoReactiveView, BranchEvaluation, BranchRenderContext, DynamicRenderer, IndexedListView,
        RenderOnlyKeyedListView, RowUpdater, StableBranch, StatefulKeyedListView,
    };
    pub use crate::lifecycle::{
        MountCleanup, MountEffect, MountErrorHandler, MountOwner, MountOwnerContext,
        MountOwnerToken, MountState, SharedCell,
    };
    pub use crate::mount::{
        MountAncestry, MountContext, MountDomAction, MountInstance, MountTarget, MountTransaction,
        MountTransactionState, Prop, PropFixed, PropMissing, View, ViewCons, ViewNil,
    };
}

#[doc(hidden)]
pub mod __private {
    pub use crate::attributes::{ApplyTarget, ApplyToDom, AttrOp, AttributeGroup, IntoStorable};
    pub use crate::attributes::{ReactiveBindingPlan, ReactiveBindingTarget};
    pub use crate::elements::{
        AnchorTag, FormTag, LabelTag, MediaTag, OpenTag, SvgTag, TableCellTag, TableHeaderTag, Tag,
        TagMetadata, TagNamespace, TextTag, TypedElement,
    };
    pub use crate::events::{EventDescriptor, EventHandler, bind_event};
    pub use crate::mount::{View, ViewCons, ViewNil};
}

#[macro_export]
macro_rules! group {
    ($($attr:expr),* $(,)?) => {
        $crate::attributes::AttributeGroup::new(vec![
            $($crate::__private::ApplyToDom::into_op(
                $attr,
                $crate::__private::ApplyTarget::Apply,
            )),*
        ])
    };
}
