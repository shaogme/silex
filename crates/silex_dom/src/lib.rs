pub mod backend;
pub mod context;
pub mod error;
pub mod host;
pub mod log;
pub mod node_ref;
pub mod tree;

mod attribute_backend;
mod event_backend;

/// Low-level attribute/property request API.
pub mod attribute {
    pub use crate::attribute_backend::*;
}

/// Backend-neutral physical event API.
pub mod event {
    pub use crate::event_backend::*;
}

#[cfg(feature = "browser")]
pub mod browser;
pub mod ssr;

pub use context::DomContext;
pub use error::{
    CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink, DomError,
    DomResult, DropFailureReport,
};
pub use tree::{
    DomDocument, DomElement, DomNode, DomRange, ElementSpec, Namespace, NodeKind, RangeMoveRequest,
    RangeRequest,
};

pub mod prelude {
    pub use crate::error::{
        CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink,
        DropFailureReport,
    };
    pub use crate::log;
    pub use crate::{
        DomError, DomResult,
        attribute::{
            AttributeRequest, AttributeTarget, AttributeValue, PropertyRequest, PropertyValue,
        },
        backend::{BackendId, DomBackend},
        context::DomContext,
        event::{
            DomEvent, DomEventBridge, EventDescriptor, EventKind, EventOptions, EventSpec,
            PhysicalEventRequest,
        },
        host::{HostCapability, HostResourceState},
        tree::{
            DomDocument, DomElement, DomNode, DomRange, ElementSpec, Namespace, NodeKind,
            RangeMoveRequest, RangeRequest,
        },
    };
}
