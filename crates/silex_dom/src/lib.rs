pub mod attribute;
pub mod backend;
pub mod context;
pub mod error;
pub mod event;
pub mod host;
pub mod log;
pub mod node_ref;
pub mod tree;

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
