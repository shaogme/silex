use std::{any::Any, fmt, rc::Rc};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{identity::BackendId, node::spec::NodeKind},
};

/// Opaque node handle. Concrete backend values stay behind `Any`.
pub struct DomNode {
    pub(crate) backend_id: BackendId,
    pub(crate) kind: NodeKind,
    pub(crate) identity: u64,
    pub(crate) raw: Rc<dyn Any>,
}

impl Clone for DomNode {
    fn clone(&self) -> Self {
        Self {
            backend_id: self.backend_id,
            kind: self.kind,
            identity: self.identity,
            raw: self.raw.clone(),
        }
    }
}

impl fmt::Debug for DomNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomNode")
            .field("backend_id", &self.backend_id.value())
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl PartialEq for DomNode {
    fn eq(&self, other: &Self) -> bool {
        self.backend_id == other.backend_id
            && ((self.identity != 0 && self.identity == other.identity)
                || Rc::ptr_eq(&self.raw, &other.raw))
    }
}

impl Eq for DomNode {}

impl DomNode {
    pub(crate) fn from_raw_with_identity(
        backend_id: BackendId,
        kind: NodeKind,
        identity: u64,
        raw: Rc<dyn Any>,
    ) -> Self {
        Self {
            backend_id,
            kind,
            identity,
            raw,
        }
    }

    pub fn backend_id(&self) -> BackendId {
        self.backend_id
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// Stable identity within one backend context, used by hydration records.
    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn is_same_node(&self, other: &Self) -> bool {
        self == other
    }
}

/// Opaque element handle with an element-only API surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomElement {
    node: DomNode,
}

impl DomElement {
    pub(crate) fn from_node(node: DomNode) -> DomResult<Self> {
        if node.kind != NodeKind::Element {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: node.kind.label(),
            });
        }
        Ok(Self { node })
    }

    pub fn node(&self) -> &DomNode {
        &self.node
    }

    pub fn backend_id(&self) -> BackendId {
        self.node.backend_id()
    }
}

/// Opaque document handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomDocument {
    node: DomNode,
}

impl DomDocument {
    pub(crate) fn from_node(node: DomNode) -> DomResult<Self> {
        if node.kind != NodeKind::Document {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Document.label(),
                actual: node.kind.label(),
            });
        }
        Ok(Self { node })
    }

    pub fn node(&self) -> &DomNode {
        &self.node
    }

    pub fn backend_id(&self) -> BackendId {
        self.node.backend_id()
    }
}
