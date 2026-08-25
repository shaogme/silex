use std::{cell::RefCell, rc::Rc};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{
        attribute::{AttributeRequest, PropertyRequest},
        event::{PhysicalEventRequest, WindowEventRequest},
        identity::BackendId,
        node::{DomDocument, DomElement, DomNode, ElementSpec, NodeKind},
    },
    runtime::{
        backend::DomBackend,
        host::HostResource,
        tree::{InsertRequest, RangeMoveRequest, RangeRequest},
    },
};

use super::{
    attribute, event,
    state::{NodeId, NodeRecord, SsrHandle, SsrState},
    tree,
};

pub(super) struct SsrBackend {
    pub(super) id: BackendId,
    pub(super) state: Rc<RefCell<SsrState>>,
}

impl SsrBackend {
    pub(super) fn new() -> Self {
        Self {
            id: BackendId::fresh(),
            state: Rc::new(RefCell::new(SsrState::new())),
        }
    }

    pub(super) fn node(&self, id: NodeId, kind: NodeKind) -> DomNode {
        DomNode::from_raw_with_identity(self.id, kind, id + 1, Rc::new(SsrHandle(id)))
    }

    pub(super) fn node_id(&self, node: &DomNode) -> DomResult<NodeId> {
        if node.backend_id() != self.id {
            return Err(DomError::CrossContext {
                expected: self.id.value(),
                actual: node.backend_id().value(),
            });
        }
        node.raw
            .as_ref()
            .downcast_ref::<SsrHandle>()
            .map(|handle| handle.0)
            .ok_or(DomError::InvalidHandle {
                backend: self.id.value(),
                kind: node.kind().label(),
            })
    }

    pub(super) fn record<'a>(&self, state: &'a SsrState, id: NodeId) -> DomResult<&'a NodeRecord> {
        state.nodes.get(&id).ok_or(DomError::InvalidHandle {
            backend: self.id.value(),
            kind: "node",
        })
    }

    pub(super) fn record_mut<'a>(
        &self,
        state: &'a mut SsrState,
        id: NodeId,
    ) -> DomResult<&'a mut NodeRecord> {
        state.nodes.get_mut(&id).ok_or(DomError::InvalidHandle {
            backend: self.id.value(),
            kind: "node",
        })
    }

    pub(super) fn validate_node(&self, state: &SsrState, node: &DomNode) -> DomResult<NodeId> {
        let id = self.node_id(node)?;
        self.record(state, id)?;
        Ok(id)
    }

    pub(super) fn validate_parent(&self, state: &SsrState, id: NodeId) -> DomResult<()> {
        let record = self.record(state, id)?;
        if matches!(
            record.kind,
            NodeKind::Document | NodeKind::Element | NodeKind::Fragment
        ) {
            if record.kind == NodeKind::Element && record.void {
                return Err(DomError::CannotContain {
                    parent: "void element",
                });
            }
            Ok(())
        } else {
            Err(DomError::CannotContain {
                parent: record.kind.label(),
            })
        }
    }
}

impl DomBackend for SsrBackend {
    fn backend_id(&self) -> BackendId {
        self.id
    }

    fn check_node(&self, node: &DomNode) -> DomResult<()> {
        let state = self.state.borrow();
        self.validate_node(&state, node).map(|_| ())
    }

    fn document(&self) -> DomResult<DomDocument> {
        DomDocument::from_node(self.node(0, NodeKind::Document))
    }

    fn create_element(&self, spec: &ElementSpec) -> DomResult<DomElement> {
        if spec.name().is_empty() {
            return Err(DomError::Backend {
                operation: "create_element",
                message: String::from("element name is empty"),
            });
        }
        let mut state = self.state.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;
        state.nodes.insert(id, NodeRecord::element(spec));
        DomElement::from_node(self.node(id, NodeKind::Element))
    }

    fn create_text(&self, value: &str) -> DomResult<DomNode> {
        let mut state = self.state.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;
        state
            .nodes
            .insert(id, NodeRecord::leaf(NodeKind::Text, value.to_string()));
        Ok(self.node(id, NodeKind::Text))
    }

    fn set_text(&self, node: &DomNode, value: &str) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let id = self.validate_node(&state, node)?;
        let record = self.record_mut(&mut state, id)?;
        if record.kind != NodeKind::Text {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Text.label(),
                actual: record.kind.label(),
            });
        }
        record.text = Some(value.to_string());
        Ok(())
    }

    fn create_comment(&self, value: &str) -> DomResult<DomNode> {
        let mut state = self.state.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;
        state
            .nodes
            .insert(id, NodeRecord::leaf(NodeKind::Comment, value.to_string()));
        Ok(self.node(id, NodeKind::Comment))
    }

    fn create_fragment(&self) -> DomResult<DomNode> {
        let mut state = self.state.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;
        state.nodes.insert(id, NodeRecord::fragment());
        Ok(self.node(id, NodeKind::Fragment))
    }

    fn append(&self, parent: &DomNode, child: &DomNode) -> DomResult<()> {
        tree::insert_request(self, &InsertRequest::append(parent, child))
    }

    fn insert_before(&self, request: &InsertRequest) -> DomResult<()> {
        tree::insert_request(self, request)
    }

    fn remove(&self, node: &DomNode) -> DomResult<()> {
        tree::remove(self, node)
    }

    fn parent(&self, node: &DomNode) -> DomResult<Option<DomNode>> {
        tree::parent(self, node)
    }

    fn children(&self, node: &DomNode) -> DomResult<Vec<DomNode>> {
        tree::children(self, node)
    }

    fn validate_range(&self, request: &RangeRequest) -> DomResult<()> {
        tree::validate_range(self, request)
    }

    fn move_range(&self, request: &RangeMoveRequest) -> DomResult<()> {
        tree::move_range(self, request)
    }

    fn set_attribute(&self, request: &AttributeRequest) -> DomResult<()> {
        attribute::set_attribute(self, request)
    }

    fn set_property(&self, request: &PropertyRequest) -> DomResult<()> {
        attribute::set_property(self, request)
    }

    fn set_style_property(
        &self,
        element: &DomElement,
        name: &str,
        value: Option<&str>,
    ) -> DomResult<()> {
        attribute::set_style_property(self, element, name, value)
    }

    fn listen(&self, request: &PhysicalEventRequest) -> DomResult<HostResource<'static>> {
        event::listen(self, request)
    }

    fn listen_window(&self, request: &WindowEventRequest) -> DomResult<HostResource<'static>> {
        let _ = request;
        Err(DomError::Unsupported {
            capability: "window event listener",
        })
    }
}
