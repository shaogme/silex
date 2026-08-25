use std::{cell::Cell, rc::Rc};

use js_sys::{Reflect, WeakMap};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    CssStyleDeclaration, Document, Element, Event, EventTarget, HtmlElement, Node, SvgElement,
};

use crate::{
    attribute::{AttributeRequest, AttributeValue, PropertyRequest, PropertyValue},
    backend::{BackendId, DomBackend},
    context::DomContext,
    error::{DomError, DomResult},
    event::{
        DomEvent, DomEventControl, DomRectData, MouseEventData, PhysicalEventRequest,
        PointerEventData, WindowEventRequest,
    },
    host::HostResource,
    tree::{
        DomDocument, DomElement, DomNode, ElementSpec, InsertRequest, NodeKind, RangeMoveRequest,
        RangeRequest,
    },
};

struct BrowserHandle(Node);

struct BrowserEventControl {
    event: Event,
}

impl DomEventControl for BrowserEventControl {
    fn prevent_default(&self) {
        self.event.prevent_default();
    }

    fn mouse_data(&self) -> Option<MouseEventData> {
        let event = self.event.dyn_ref::<web_sys::MouseEvent>()?;
        Some(MouseEventData::new(
            event.button(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
            event.alt_key(),
        ))
    }

    fn input_value(&self) -> Option<String> {
        let target = self.event.target()?;
        if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
            return Some(input.value());
        }
        if let Some(input) = target.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            return Some(input.value());
        }
        target
            .dyn_ref::<web_sys::HtmlSelectElement>()
            .map(web_sys::HtmlSelectElement::value)
    }

    fn key(&self) -> Option<String> {
        self.event
            .dyn_ref::<web_sys::KeyboardEvent>()
            .map(web_sys::KeyboardEvent::key)
    }

    fn pointer_data(&self) -> Option<PointerEventData> {
        let event = self.event.dyn_ref::<web_sys::PointerEvent>()?;
        Some(PointerEventData::new(
            event.client_x() as f64,
            event.client_y() as f64,
            event.pointer_id(),
        ))
    }

    fn rect(&self) -> Option<DomRectData> {
        // A bubbling event may originate from a dynamically sized child. Layout
        // calculations for the listener must use the listener's own element.
        let target = self
            .event
            .current_target()
            .or_else(|| self.event.target())?;
        let element = target.dyn_into::<web_sys::Element>().ok()?;
        let rect = element.get_bounding_client_rect();
        Some(DomRectData::new(
            rect.top(),
            rect.left(),
            rect.width(),
            rect.height(),
        ))
    }

    fn focus_target(&self) -> DomResult<()> {
        let target = self
            .event
            .current_target()
            .or_else(|| self.event.target())
            .ok_or(DomError::Unsupported {
                capability: "focus",
            })?;
        target
            .dyn_into::<web_sys::HtmlElement>()
            .map_err(|_| DomError::Unsupported {
                capability: "focus",
            })?
            .focus()
            .map_err(|error| BrowserBackend::error("focus", error))
    }
}

/// Browser adapter. JS casts are deliberately contained in this module; the
/// shared backend/context/tree API only sees opaque handles.
pub struct BrowserDom {
    context: DomContext,
    backend: Rc<BrowserBackend>,
}

struct BrowserBackend {
    id: BackendId,
    document: Document,
    next_identity: Cell<u64>,
    identities: WeakMap,
}

impl BrowserDom {
    pub fn new(document: Document) -> Self {
        let backend = Rc::new(BrowserBackend {
            id: next_backend_id(),
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
        let window = web_sys::window().ok_or(DomError::Backend {
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

fn next_backend_id() -> BackendId {
    BackendId::fresh()
}

impl BrowserBackend {
    fn node(&self, node: &DomNode) -> DomResult<Node> {
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

    fn handle(&self, node: Node, kind: NodeKind) -> DomNode {
        let identity = self.identity_for(&node);
        DomNode::from_raw_with_identity(self.id, kind, identity, Rc::new(BrowserHandle(node)))
    }

    fn kind(node: &Node) -> DomResult<NodeKind> {
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

    fn document_node(&self) -> Node {
        self.document.clone().into()
    }

    fn document_identity(&self) -> u64 {
        self.identity_for(&self.document_node())
    }

    fn validate_document(&self, node: &Node) -> DomResult<()> {
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

    fn element(&self, node: &DomNode) -> DomResult<Element> {
        self.node(node)?
            .dyn_into::<Element>()
            .map_err(|_| DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: node.kind().label(),
            })
    }

    fn style(&self, element: &DomElement) -> DomResult<CssStyleDeclaration> {
        let element = self.element(element.node())?;
        element
            .dyn_ref::<HtmlElement>()
            .map(HtmlElement::style)
            .or_else(|| element.dyn_ref::<SvgElement>().map(SvgElement::style))
            .ok_or(DomError::Unsupported {
                capability: "style property",
            })
    }

    fn error(operation: &'static str, error: JsValue) -> DomError {
        DomError::Backend {
            operation,
            message: format!("{error:?}"),
        }
    }

    fn validate_node(&self, node: &DomNode) -> DomResult<Node> {
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
        let node: Node = self.document.clone().into();
        DomDocument::from_node(self.handle(node, NodeKind::Document))
    }

    fn document_body(&self) -> DomResult<Option<DomElement>> {
        self.document
            .body()
            .map(|body| {
                let node: Node = body.into();
                DomElement::from_node(self.handle(node, NodeKind::Element))
            })
            .transpose()
    }

    fn create_element(&self, spec: &ElementSpec) -> DomResult<DomElement> {
        let element = match spec.namespace().uri() {
            Some(uri) => self
                .document
                .create_element_ns(Some(uri), spec.name())
                .map_err(|error| Self::error("create_element", error))?,
            None => self
                .document
                .create_element(spec.name())
                .map_err(|error| Self::error("create_element", error))?,
        };
        let node = self.handle(element.into(), NodeKind::Element);
        DomElement::from_node(node)
    }

    fn create_text(&self, value: &str) -> DomResult<DomNode> {
        Ok(self.handle(self.document.create_text_node(value).into(), NodeKind::Text))
    }

    fn set_text(&self, node: &DomNode, value: &str) -> DomResult<()> {
        self.validate_node(node)?
            .dyn_into::<web_sys::Text>()
            .map_err(|_| DomError::WrongNodeKind {
                expected: NodeKind::Text.label(),
                actual: node.kind().label(),
            })?
            .set_data(value);
        Ok(())
    }

    fn create_comment(&self, value: &str) -> DomResult<DomNode> {
        Ok(self.handle(
            self.document.create_comment(value).into(),
            NodeKind::Comment,
        ))
    }

    fn create_fragment(&self) -> DomResult<DomNode> {
        Ok(self.handle(
            self.document.create_document_fragment().into(),
            NodeKind::Fragment,
        ))
    }

    fn append(&self, parent: &DomNode, child: &DomNode) -> DomResult<()> {
        self.validate_node(parent)?
            .append_child(&self.validate_node(child)?)
            .map(|_| ())
            .map_err(|error| Self::error("append", error))
    }

    fn insert_before(&self, request: &InsertRequest) -> DomResult<()> {
        let parent = self.validate_node(&request.parent)?;
        let child = self.validate_node(&request.node)?;
        let reference = request
            .reference
            .as_ref()
            .map(|node| self.validate_node(node))
            .transpose()?;
        parent
            .insert_before(&child, reference.as_ref())
            .map(|_| ())
            .map_err(|error| Self::error("insert_before", error))
    }

    fn remove(&self, node: &DomNode) -> DomResult<()> {
        let node_value = self.validate_node(node)?;
        let parent = node_value.parent_node().ok_or(DomError::NoParent)?;
        parent
            .remove_child(&node_value)
            .map(|_| ())
            .map_err(|error| Self::error("remove", error))
    }

    fn parent(&self, node: &DomNode) -> DomResult<Option<DomNode>> {
        let parent = self.validate_node(node)?.parent_node();
        parent
            .map(|parent| {
                let document_node: Node = self.document.clone().into();
                let kind = if parent.is_same_node(Some(&document_node)) {
                    NodeKind::Document
                } else {
                    Self::kind(&parent)?
                };
                Ok(self.handle(parent, kind))
            })
            .transpose()
    }

    fn children(&self, node: &DomNode) -> DomResult<Vec<DomNode>> {
        let children = self.validate_node(node)?.child_nodes();
        let mut result = Vec::with_capacity(children.length() as usize);
        for index in 0..children.length() {
            if let Some(child) = children.item(index) {
                let kind = Self::kind(&child)?;
                result.push(self.handle(child, kind));
            }
        }
        Ok(result)
    }

    fn validate_range(&self, request: &RangeRequest) -> DomResult<()> {
        let parent = self.validate_node(&request.parent)?;
        let start = self.validate_node(&request.start)?;
        let end = self.validate_node(&request.end)?;
        let start_parent = start.parent_node().ok_or(DomError::ReferenceNotChild)?;
        let end_parent = end.parent_node().ok_or(DomError::ReferenceNotChild)?;
        if !start_parent.is_same_node(Some(&parent)) || !end_parent.is_same_node(Some(&parent)) {
            return Err(DomError::ReferenceNotChild);
        }
        let children = parent.child_nodes();
        let start_index = (0..children.length())
            .find(|index| {
                children
                    .item(*index)
                    .is_some_and(|node| node.is_same_node(Some(&start)))
            })
            .ok_or(DomError::ReferenceNotChild)?;
        let end_index = (0..children.length())
            .find(|index| {
                children
                    .item(*index)
                    .is_some_and(|node| node.is_same_node(Some(&end)))
            })
            .ok_or(DomError::ReferenceNotChild)?;
        if start_index > end_index {
            return Err(DomError::ParentMismatch);
        }
        Ok(())
    }

    fn move_range(&self, request: &RangeMoveRequest) -> DomResult<()> {
        self.validate_range(&request.source)?;
        let source_parent = self.validate_node(&request.source.parent)?;
        let start = self.validate_node(&request.source.start)?;
        let end = self.validate_node(&request.source.end)?;
        let target_parent = self.validate_node(&request.target_parent)?;
        let reference = self.validate_node(&request.reference)?;
        if !reference
            .parent_node()
            .is_some_and(|parent| parent.is_same_node(Some(&target_parent)))
        {
            return Err(DomError::ReferenceNotChild);
        }
        let children = source_parent.child_nodes();
        let start_index = (0..children.length())
            .find(|index| {
                children
                    .item(*index)
                    .is_some_and(|node| node.is_same_node(Some(&start)))
            })
            .ok_or(DomError::ReferenceNotChild)?;
        let end_index = (0..children.length())
            .find(|index| {
                children
                    .item(*index)
                    .is_some_and(|node| node.is_same_node(Some(&end)))
            })
            .ok_or(DomError::ReferenceNotChild)?;
        let mut moving = Vec::new();
        for index in start_index..=end_index {
            if let Some(node) = children.item(index) {
                if node.is_same_node(Some(&reference)) {
                    return Err(DomError::ParentMismatch);
                }
                moving.push(node);
            }
        }
        let fragment = self.document.create_document_fragment();
        for node in moving {
            fragment
                .append_child(&node)
                .map_err(|error| Self::error("move_range.append", error))?;
        }
        target_parent
            .insert_before(&fragment, Some(&reference))
            .map(|_| ())
            .map_err(|error| Self::error("move_range.insert", error))
    }

    fn set_attribute(&self, request: &AttributeRequest) -> DomResult<()> {
        let element = self.element(request.element.node())?;
        let name = request.target.name();
        if name.is_empty() {
            return Err(DomError::AttributeNameEmpty);
        }
        match &request.value {
            AttributeValue::Removed => element
                .remove_attribute(name)
                .map_err(|error| Self::error("remove_attribute", error)),
            AttributeValue::Empty => element
                .set_attribute(name, "")
                .map_err(|error| Self::error("set_attribute", error)),
            AttributeValue::Text(value) => element
                .set_attribute(name, value)
                .map_err(|error| Self::error("set_attribute", error)),
            AttributeValue::ClassTokens { add, remove } => {
                let class_list = element.class_list();
                for class_name in add {
                    class_list
                        .add_1(class_name)
                        .map_err(|error| Self::error("class_list.add", error))?;
                }
                for class_name in remove {
                    class_list
                        .remove_1(class_name)
                        .map_err(|error| Self::error("class_list.remove", error))?;
                }
                Ok(())
            }
        }
    }

    fn set_property(&self, request: &PropertyRequest) -> DomResult<()> {
        let element = self.element(request.element.node())?;
        if request.name.is_empty() {
            return Err(DomError::AttributeNameEmpty);
        }
        let value = match &request.value {
            PropertyValue::Removed => {
                return Reflect::delete_property(&element, &JsValue::from_str(&request.name))
                    .map(|_| ())
                    .map_err(|error| Self::error("remove_property", error));
            }
            PropertyValue::String(value) => JsValue::from_str(value),
            PropertyValue::Bool(value) => JsValue::from_bool(*value),
            PropertyValue::Number(value) => JsValue::from_f64(*value),
        };
        Reflect::set(&element, &JsValue::from_str(&request.name), &value)
            .map(|_| ())
            .map_err(|error| Self::error("set_property", error))
    }

    fn set_style_property(
        &self,
        element: &DomElement,
        name: &str,
        value: Option<&str>,
    ) -> DomResult<()> {
        if name.is_empty() {
            return Err(DomError::AttributeNameEmpty);
        }
        let style = self.style(element)?;
        match value {
            Some(value) => style
                .set_property(name, value)
                .map_err(|error| Self::error("set_style_property", error)),
            None => style
                .remove_property(name)
                .map(|_| ())
                .map_err(|error| Self::error("remove_style_property", error)),
        }
    }

    fn document_hidden(&self) -> DomResult<Option<bool>> {
        Ok(Some(self.document.hidden()))
    }

    fn get_attribute(&self, element: &DomElement, name: &str) -> DomResult<Option<String>> {
        Ok(self.element(element.node())?.get_attribute(name))
    }

    fn focus(&self, element: &DomElement) -> DomResult<()> {
        let node = self.node(element.node())?;
        if !node.is_connected() {
            return Err(DomError::Detached {
                kind: NodeKind::Element.label(),
            });
        }
        node.dyn_into::<web_sys::HtmlElement>()
            .map_err(|_| DomError::WrongNodeKind {
                expected: "focusable html element",
                actual: NodeKind::Element.label(),
            })?
            .focus()
            .map_err(|error| Self::error("focus", error))
    }

    fn active_element(&self) -> DomResult<Option<DomElement>> {
        self.document
            .active_element()
            .map(|element| {
                let node = self.handle(element.into(), NodeKind::Element);
                DomElement::from_node(node)
            })
            .transpose()
    }

    fn contains(&self, parent: &DomElement, child: &DomNode) -> DomResult<bool> {
        Ok(self
            .element(parent.node())?
            .contains(Some(&self.validate_node(child)?)))
    }

    fn listen(&self, request: &PhysicalEventRequest) -> DomResult<HostResource<'static>> {
        request.validate()?;
        let target: EventTarget = self.element(request.target.node())?.into();
        let name = request.spec.name().to_string();
        let spec = request.spec.clone();
        let target_node = request.target.node().clone();
        let bridge = request.bridge.clone();
        let callback: Closure<dyn FnMut(Event)> =
            Closure::wrap_assert_unwind_safe(Box::new(move |event: Event| {
                if let Some(bridge) = &bridge {
                    let control = Rc::new(BrowserEventControl {
                        event: event.clone(),
                    });
                    let _ = bridge.dispatch(DomEvent::new_with_control(
                        spec.clone(),
                        target_node.clone(),
                        Some(control),
                    ));
                }
            }));
        let options = web_sys::AddEventListenerOptions::new();
        options.set_capture(request.options.capture);
        options.set_once(request.options.once);
        options.set_passive(request.options.passive);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                &name,
                callback.as_ref().unchecked_ref(),
                &options,
            )
            .map_err(|error| Self::error("listen", error))?;
        let cancel_target = target.clone();
        let cancel_name = name.clone();
        Ok(HostResource::with_cancel(move || {
            cancel_target
                .remove_event_listener_with_callback(
                    &cancel_name,
                    callback.as_ref().unchecked_ref(),
                )
                .map_err(|error| Self::error("cancel_listener", error))
        }))
    }

    fn listen_window(&self, request: &WindowEventRequest) -> DomResult<HostResource<'static>> {
        request.validate()?;
        let window = self
            .document
            .default_view()
            .ok_or_else(|| Self::error("listen_window", JsValue::from_str("window unavailable")))?;
        let name = request.spec.name().to_string();
        let spec = request.spec.clone();
        let target_node = self.handle(self.document.clone().into(), NodeKind::Document);
        let bridge = request.bridge.clone();
        let callback: Closure<dyn FnMut(Event)> =
            Closure::wrap_assert_unwind_safe(Box::new(move |event: Event| {
                if let Some(bridge) = &bridge {
                    let control = Rc::new(BrowserEventControl {
                        event: event.clone(),
                    });
                    let _ = bridge.dispatch(DomEvent::new_with_control(
                        spec.clone(),
                        target_node.clone(),
                        Some(control),
                    ));
                }
            }));
        let options = web_sys::AddEventListenerOptions::new();
        options.set_capture(request.options.capture);
        options.set_once(request.options.once);
        options.set_passive(request.options.passive);
        window
            .add_event_listener_with_callback_and_add_event_listener_options(
                &name,
                callback.as_ref().unchecked_ref(),
                &options,
            )
            .map_err(|error| Self::error("listen_window", error))?;
        let cancel_window = window.clone();
        let cancel_name = name.clone();
        Ok(HostResource::with_cancel(move || {
            cancel_window
                .remove_event_listener_with_callback(
                    &cancel_name,
                    callback.as_ref().unchecked_ref(),
                )
                .map_err(|error| Self::error("cancel_window_listener", error))
        }))
    }
}
