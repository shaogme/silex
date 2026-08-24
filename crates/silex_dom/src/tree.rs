use std::{any::Any, fmt, rc::Rc};

use crate::{
    backend::BackendId,
    context::DomContext,
    error::{DomError, DomResult},
};

/// Node categories shared by browser and SSR implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element,
    Text,
    Comment,
    Fragment,
}

impl NodeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Element => "element",
            Self::Text => "text",
            Self::Comment => "comment",
            Self::Fragment => "fragment",
        }
    }
}

/// HTML/XML namespace metadata used by element creation and serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    Html,
    Svg,
    MathMl,
    Custom(String),
}

impl Namespace {
    pub fn uri(&self) -> Option<&str> {
        match self {
            Self::Html => None,
            Self::Svg => Some("http://www.w3.org/2000/svg"),
            Self::MathMl => Some("http://www.w3.org/1998/Math/MathML"),
            Self::Custom(uri) => Some(uri),
        }
    }
}

/// Backend-independent element metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementSpec {
    name: String,
    namespace: Namespace,
    void: bool,
}

impl ElementSpec {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let void = is_html_void_name(&name);
        Self {
            name,
            namespace: Namespace::Html,
            void,
        }
    }

    pub fn namespaced(name: impl Into<String>, namespace: Namespace, void: bool) -> Self {
        Self {
            name: name.into(),
            namespace,
            void,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn is_void(&self) -> bool {
        self.void
    }
}

fn is_html_void_name(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

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

/// Request for inserting a node before an optional reference child.
#[derive(Clone, Debug)]
pub struct InsertRequest {
    pub parent: DomNode,
    pub node: DomNode,
    pub reference: Option<DomNode>,
}

impl InsertRequest {
    pub fn append(parent: &DomNode, node: &DomNode) -> Self {
        Self {
            parent: parent.clone(),
            node: node.clone(),
            reference: None,
        }
    }

    pub fn before(parent: &DomNode, node: &DomNode, reference: &DomNode) -> Self {
        Self {
            parent: parent.clone(),
            node: node.clone(),
            reference: Some(reference.clone()),
        }
    }
}

/// Request for a contiguous inclusive node range.
#[derive(Clone, Debug)]
pub struct RangeRequest {
    pub parent: DomNode,
    pub start: DomNode,
    pub end: DomNode,
}

/// Request to move one validated inclusive range before a reference node.
#[derive(Clone, Debug)]
pub struct RangeMoveRequest {
    pub source: RangeRequest,
    pub target_parent: DomNode,
    pub reference: DomNode,
}

/// A context-bound inclusive range. It does not expose concrete backend data.
#[derive(Clone)]
pub struct DomRange {
    context: DomContext,
    parent: DomNode,
    start: DomNode,
    end: DomNode,
}

impl DomRange {
    pub(crate) fn new(context: DomContext, parent: DomNode, start: DomNode, end: DomNode) -> Self {
        Self {
            context,
            parent,
            start,
            end,
        }
    }

    pub fn nodes(&self) -> DomResult<Vec<DomNode>> {
        let children = self.context.children(&self.parent)?;
        let start = children
            .iter()
            .position(|node| node == &self.start)
            .ok_or(DomError::ReferenceNotChild)?;
        let end = children
            .iter()
            .position(|node| node == &self.end)
            .ok_or(DomError::ReferenceNotChild)?;
        if start > end {
            return Err(DomError::ParentMismatch);
        }
        Ok(children[start..=end].to_vec())
    }

    pub fn remove(&self) -> DomResult<()> {
        for node in self.nodes()? {
            self.context.remove(&node)?;
        }
        Ok(())
    }

    /// Move the complete range as one ordered DOM operation.
    pub fn move_before(&self, target_parent: &DomNode, reference: &DomNode) -> DomResult<()> {
        self.context.move_range(RangeMoveRequest {
            source: RangeRequest {
                parent: self.parent.clone(),
                start: self.start.clone(),
                end: self.end.clone(),
            },
            target_parent: target_parent.clone(),
            reference: reference.clone(),
        })
    }
}
