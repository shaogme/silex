extern crate self as silex;

pub mod components;
pub mod flow;
#[cfg(feature = "tw")]
pub mod ui;

pub use components::*;
pub use silex_core::error::{
    ErrorHandler, ErrorHandlerToken, ErrorReporter, SilexError, SilexResult,
};
pub use silex_core::{
    Callback, CloseError, NodeRef, OwnerAccess, OwnerHandle, ReactiveError, Runtime, Rx,
    SilexErrorKind, unwind_safe,
};

pub mod reexports {
    pub use js_sys;
    #[cfg(feature = "json")]
    pub use serde_json;
    #[cfg(feature = "net")]
    pub use silex_net::reexports::gloo_timers;
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

pub mod core {
    pub use silex_core::*;
}

pub mod html {
    pub use silex_html::*;
}

pub mod css {
    pub use silex_css::*;
}

pub mod macros {
    pub use silex_macros::*;
}

pub mod dom {
    pub use silex_dom::*;
}

pub mod view {
    pub use silex_view::{
        app, attributes, elements, errors, events, flow, lifecycle, mount, prelude,
    };
    pub use silex_view::{
        app::{MountBuilderContext, MountedApp},
        attributes::{
            ApplyTarget, Attr, AttrOp, AttributeBuilder, AttributeGroup, IntoStorable, KnownProp,
            update_class_tokens,
        },
        elements::{AnyView, Element, Tag, TagMetadata, TagNamespace, TypedElement, text},
        errors::{DisposeError, MountAvailability, MountError, RollbackError, ViewError},
        events::{
            DomEvent, DomRectData, Event, EventDescriptor, EventHandler, EventKind, EventSpec,
            MouseEventData, PointerEventData, WithEventArg, WithoutEventArg, bind_window_event,
        },
        flow::{
            AutoReactiveView, BranchEvaluation, BranchRenderContext, DynamicRenderer,
            IndexedListView, RenderOnlyKeyedListView, RowUpdater, StableBranch,
            StatefulKeyedListView,
        },
        lifecycle::{
            MountCleanup, MountEffect, MountErrorHandler, MountOwner, MountOwnerContext,
            MountOwnerToken, MountState, SharedCell,
        },
        mount::{
            MountAncestry, MountContext, MountDomAction, MountInstance, MountTarget,
            MountTransaction, MountTransactionState, Prop, PropFixed, PropInto, PropMissing, View,
            ViewCons, ViewNil,
        },
    };
    pub use silex_view::{chain, define_tag, group, view_match};
}

#[cfg(feature = "bootstrap")]
pub mod bootstrap {
    pub use silex_bootstrap::*;
}

pub mod hash {
    pub use silex_hash::*;
}

pub mod router {
    pub use silex_router::*;
}

#[cfg(feature = "persistence")]
pub mod persist {
    pub use silex_persist::*;
}

#[cfg(feature = "net")]
pub mod net {
    pub use silex_net::*;
}

#[cfg(feature = "i18n")]
pub mod i18n {
    pub use silex_i18n::*;
}

#[cfg(feature = "i18n")]
pub use crate::i18n::*;

pub mod prelude {
    pub use crate::components::*;
    pub use crate::flow::*;
    #[cfg(feature = "i18n")]
    pub use crate::i18n::*;
    #[cfg(feature = "net")]
    pub use crate::net::*;
    #[cfg(feature = "persistence")]
    pub use crate::persist::*;
    pub use crate::{ReactiveError, SilexError, SilexResult};
    pub use silex_core::prelude::*;
    pub use silex_css::prelude::*;
    pub use silex_html::*;
    pub use silex_macros::*;
    pub use silex_router::*;
    pub use silex_view::prelude::{
        AnyView, AriaAttributes, Attr, AttrOp, AttributeBuilder, AttributeGroup, BranchEvaluation,
        BranchRenderContext, DomEvent, DomRectData, Element, Event, EventDescriptor, EventKind,
        EventSpec, GlobalAttributes, GlobalEventAttributes, IntoStorable, MountAncestry,
        MountBuilderContext, MountContext, MountDomAction, MountInstance, MountOwner,
        MountOwnerToken, MountState, MountedApp, PointerEventData, Prop, PropFixed, PropMissing,
        RowUpdater, Tag, TypedElement, View, ViewCons, ViewNil, chain,
    };

    // Resolve ambiguous glob re-exports
    #[cfg(feature = "css")]
    pub use crate::components::Center;
    pub use crate::core::prelude::{Map, RxWrite};
    pub use crate::flow::Switch;
    #[cfg(feature = "net")]
    pub use crate::net::reexports;
    pub use silex_css::prelude::{Style, linear_gradient, radial_gradient};
    #[cfg(feature = "tw")]
    pub use silex_css::prelude::{VariantSchema, declare_variants};
    pub use silex_html::{Em, em};
    #[cfg(feature = "css")]
    pub use silex_macros::{global, inject_css, styled, theme};
    #[cfg(feature = "tw")]
    pub use silex_macros::{tw, tw_variants, tw_verbose};
    pub use silex_router::Link;
    pub use silex_view::elements::text;
}
