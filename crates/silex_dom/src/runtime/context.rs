use std::rc::Rc;

use crate::{
    diagnostics::error::DomResult,
    model::{
        attribute::{AttributeRequest, PropertyRequest},
        event::{PhysicalEventRequest, WindowEventRequest},
        identity::BackendId,
        node::{DomDocument, DomElement, DomNode, ElementSpec},
    },
};

use super::{
    backend::DomBackend,
    host::HostResource,
    range::DomRange,
    tree::{InsertRequest, RangeMoveRequest, RangeRequest},
};

/// Explicitly injected, cheaply cloneable DOM context.
#[derive(Clone)]
pub struct DomContext {
    backend: Rc<dyn DomBackend>,
}

impl DomContext {
    pub fn from_backend(backend: Rc<dyn DomBackend>) -> Self {
        Self { backend }
    }

    pub fn backend_id(&self) -> BackendId {
        self.backend.backend_id()
    }

    pub fn same_backend(&self, other: &Self) -> bool {
        self.backend_id() == other.backend_id()
    }

    pub fn validate_node(&self, node: &DomNode) -> DomResult<()> {
        self.backend.check_node(node)
    }

    pub fn document(&self) -> DomResult<DomDocument> {
        self.backend.document()
    }

    pub fn document_body(&self) -> DomResult<Option<DomElement>> {
        self.backend.document_body()
    }

    pub fn create_element(&self, spec: ElementSpec) -> DomResult<DomElement> {
        self.backend.create_element(&spec)
    }

    /// Convert an opaque node into an element handle after validating its
    /// backend identity and node kind.
    pub fn element(&self, node: &DomNode) -> DomResult<DomElement> {
        self.validate_node(node)?;
        DomElement::from_node(node.clone())
    }

    pub fn create_text(&self, value: impl AsRef<str>) -> DomResult<DomNode> {
        self.backend.create_text(value.as_ref())
    }

    pub fn set_text(&self, node: &DomNode, value: impl AsRef<str>) -> DomResult<()> {
        self.backend.set_text(node, value.as_ref())
    }

    pub fn create_comment(&self, value: impl AsRef<str>) -> DomResult<DomNode> {
        self.backend.create_comment(value.as_ref())
    }

    pub fn create_fragment(&self) -> DomResult<DomNode> {
        self.backend.create_fragment()
    }

    pub fn append(&self, parent: &DomNode, child: &DomNode) -> DomResult<()> {
        self.backend.append(parent, child)
    }

    pub fn insert_before(&self, request: InsertRequest) -> DomResult<()> {
        self.backend.insert_before(&request)
    }

    pub fn remove(&self, node: &DomNode) -> DomResult<()> {
        self.backend.remove(node)
    }

    pub fn parent(&self, node: &DomNode) -> DomResult<Option<DomNode>> {
        self.backend.parent(node)
    }

    pub fn children(&self, node: &DomNode) -> DomResult<Vec<DomNode>> {
        self.backend.children(node)
    }

    pub fn range(&self, request: RangeRequest) -> DomResult<DomRange> {
        self.backend.validate_range(&request)?;
        Ok(DomRange::new(
            self.clone(),
            request.parent,
            request.start,
            request.end,
        ))
    }

    pub(crate) fn move_range(&self, request: RangeMoveRequest) -> DomResult<()> {
        self.backend.move_range(&request)
    }

    pub fn set_attribute(&self, request: AttributeRequest) -> DomResult<()> {
        self.backend.set_attribute(&request)
    }

    pub fn set_property(&self, request: PropertyRequest) -> DomResult<()> {
        self.backend.set_property(&request)
    }

    pub fn set_style_property(
        &self,
        element: &DomElement,
        name: &str,
        value: Option<&str>,
    ) -> DomResult<()> {
        self.backend.set_style_property(element, name, value)
    }

    pub fn get_attribute(
        &self,
        element: &DomElement,
        name: impl AsRef<str>,
    ) -> DomResult<Option<String>> {
        self.backend.get_attribute(element, name.as_ref())
    }

    pub fn focus(&self, element: &DomElement) -> DomResult<()> {
        self.validate_node(element.node())?;
        self.backend.focus(element)
    }

    pub fn active_element(&self) -> DomResult<Option<DomElement>> {
        self.backend.active_element()
    }

    pub fn contains(&self, parent: &DomElement, child: &DomNode) -> DomResult<bool> {
        self.backend.contains(parent, child)
    }

    pub fn document_hidden(&self) -> DomResult<Option<bool>> {
        self.backend.document_hidden()
    }

    pub fn listen(&self, request: PhysicalEventRequest) -> DomResult<HostResource<'static>> {
        self.backend.listen(&request)
    }

    pub fn listen_window(&self, request: WindowEventRequest) -> DomResult<HostResource<'static>> {
        self.backend.listen_window(&request)
    }
}
