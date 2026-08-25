use std::{cell::Cell, rc::Rc};

use js_sys::WeakMap;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Document, Element, HtmlElement, Node, SvgElement};

#[cfg(target_arch = "wasm32")]
use web_sys::window;

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
        context::DomContext,
        host::HostResource,
        tree::{InsertRequest, RangeMoveRequest, RangeRequest},
    },
};

use super::{attribute, listener, tree};

pub struct BrowserDom {
    context: DomContext,
    backend: Rc<BrowserBackend>,
}

pub(super) struct BrowserBackend {
    id: BackendId,
    document: Document,
    next_identity: Cell<u64>,
    identities: WeakMap,
}

struct BrowserHandle(Node);

impl BrowserDom {
    pub fn new(document: Document) -> Self {
        let backend = Rc::new(BrowserBackend {
            id: BackendId::fresh(),
            document,
            next_identity: Cell::new(1),
            identities: WeakMap::new(),
        });
        let erased: Rc<dyn DomBackend> = backend.clone();
        Self {
            context: DomContext::from_backend(erased),
            backend,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_window() -> DomResult<Self> {
        let window = window().ok_or(DomError::Backend {
            operation: "from_window",
            message: String::from("window is unavailable"),
        })?;
        let document = window.document().ok_or(DomError::Backend {
            operation: "from_window",
            message: String::from("document is unavailable"),
        })?;
        Ok(Self::new(document))
    }

    pub fn context(&self) -> DomContext {
        self.context.clone()
    }

    pub fn document(&self) -> DomResult<DomDocument> {
        self.context.document()
    }

    pub fn from_web_sys_node(&self, node: Node) -> DomResult<DomNode> {
        self.backend.validate_document(&node)?;
        let kind = BrowserBackend::kind(&node)?;
        Ok(self.backend.handle(node, kind))
    }
}

impl BrowserBackend {
    pub(super) fn node(&self, node: &DomNode) -> DomResult<Node> {
        if node.backend_id() != self.id {
            return Err(DomError::CrossContext {
                expected: self.id.value(),
                actual: node.backend_id().value(),
            });
        }
        let expected_kind = node.kind();
        let node = node
            .raw
            .as_ref()
            .downcast_ref::<BrowserHandle>()
            .map(|handle| handle.0.clone())
            .ok_or(DomError::InvalidHandle {
                backend: self.id.value(),
                kind: expected_kind.label(),
            })?;
        self.validate_document(&node)?;
        if Self::kind(&node)? != expected_kind {
            return Err(DomError::InvalidHandle {
                backend: self.id.value(),
                kind: "node kind metadata",
            });
        }
        Ok(node)
    }

    pub(super) fn handle(&self, node: Node, kind: NodeKind) -> DomNode {
        let identity = self.identity_for(&node);
        DomNode::from_raw_with_identity(self.id, kind, identity, Rc::new(BrowserHandle(node)))
    }

    pub(super) fn kind(node: &Node) -> DomResult<NodeKind> {
        match node.node_type() {
            1 => Ok(NodeKind::Element),
            3 => Ok(NodeKind::Text),
            8 => Ok(NodeKind::Comment),
            9 => Ok(NodeKind::Document),
            11 => Ok(NodeKind::Fragment),
            _ => Err(DomError::Unsupported {
                capability: "browser node kind",
            }),
        }
    }

    pub(super) fn raw_document(&self) -> Document {
        self.document.clone()
    }

    pub(super) fn document_node(&self) -> Node {
        self.document.clone().into()
    }

    fn document_identity(&self) -> u64 {
        self.identity_for(&self.document_node())
    }

    pub(super) fn validate_document(&self, node: &Node) -> DomResult<()> {
        let expected = self.document_node();
        let actual_document = if node.node_type() == 9 {
            Some(node.clone())
        } else {
            node.owner_document().map(Into::into)
        };
        let same_document = actual_document
            .as_ref()
            .is_some_and(|document| document.is_same_node(Some(&expected)));
        if same_document {
            return Ok(());
        }
        Err(DomError::CrossContext {
            expected: self.document_identity(),
            actual: actual_document.as_ref().map_or_else(
                || self.identity_for(node),
                |document| self.identity_for(document),
            ),
        })
    }

    fn identity_for(&self, node: &Node) -> u64 {
        if let Some(identity) = self.identities.get(node.as_ref()).as_f64() {
            return identity as u64;
        }
        let identity = self.next_identity.get();
        self.next_identity.set(identity.saturating_add(1));
        self.identities
            .set(node.as_ref(), &JsValue::from_f64(identity as f64));
        identity
    }

    pub(super) fn element(&self, node: &DomNode) -> DomResult<Element> {
        self.node(node)?
            .dyn_into::<Element>()
            .map_err(|_| DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: node.kind().label(),
            })
    }

    pub(super) fn style(&self, element: &DomElement) -> DomResult<CssStyleDeclaration> {
        let element = self.element(element.node())?;
        element
            .dyn_ref::<HtmlElement>()
            .map(HtmlElement::style)
            .or_else(|| element.dyn_ref::<SvgElement>().map(SvgElement::style))
            .ok_or(DomError::Unsupported {
                capability: "style property",
            })
    }

    pub(super) fn error(operation: &'static str, error: JsValue) -> DomError {
        DomError::Backend {
            operation,
            message: format!("{error:?}"),
        }
    }

    pub(super) fn validate_node(&self, node: &DomNode) -> DomResult<Node> {
        self.node(node)
    }
}

impl DomBackend for BrowserBackend {
    fn backend_id(&self) -> BackendId {
        self.id
    }

    fn check_node(&self, node: &DomNode) -> DomResult<()> {
        self.validate_node(node).map(|_| ())
    }

    fn document(&self) -> DomResult<DomDocument> {
        tree::document(self)
    }

    fn document_body(&self) -> DomResult<Option<DomElement>> {
        tree::document_body(self)
    }

    fn create_element(&self, spec: &ElementSpec) -> DomResult<DomElement> {
        tree::create_element(self, spec)
    }

    fn create_text(&self, value: &str) -> DomResult<DomNode> {
        tree::create_text(self, value)
    }

    fn set_text(&self, node: &DomNode, value: &str) -> DomResult<()> {
        tree::set_text(self, node, value)
    }

    fn create_comment(&self, value: &str) -> DomResult<DomNode> {
        tree::create_comment(self, value)
    }

    fn create_fragment(&self) -> DomResult<DomNode> {
        tree::create_fragment(self)
    }

    fn append(&self, parent: &DomNode, child: &DomNode) -> DomResult<()> {
        tree::append(self, parent, child)
    }

    fn insert_before(&self, request: &InsertRequest) -> DomResult<()> {
        tree::insert_before(self, request)
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

    fn get_attribute(&self, element: &DomElement, name: &str) -> DomResult<Option<String>> {
        attribute::get_attribute(self, element, name)
    }

    fn focus(&self, element: &DomElement) -> DomResult<()> {
        tree::focus(self, element)
    }

    fn active_element(&self) -> DomResult<Option<DomElement>> {
        tree::active_element(self)
    }

    fn contains(&self, parent: &DomElement, child: &DomNode) -> DomResult<bool> {
        tree::contains(self, parent, child)
    }

    fn document_hidden(&self) -> DomResult<Option<bool>> {
        tree::document_hidden(self)
    }

    fn listen(&self, request: &PhysicalEventRequest) -> DomResult<HostResource<'static>> {
        listener::listen(self, request)
    }

    fn listen_window(&self, request: &WindowEventRequest) -> DomResult<HostResource<'static>> {
        listener::listen_window(self, request)
    }
}
