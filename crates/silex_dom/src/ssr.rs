use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use crate::{
    attribute::{AttributeRequest, AttributeValue, PropertyRequest, PropertyValue},
    backend::{BackendId, DomBackend},
    context::DomContext,
    error::{DomError, DomResult},
    event::{EventRecord, PhysicalEventRequest},
    host::HostResource,
    tree::{
        DomDocument, DomElement, DomNode, ElementSpec, InsertRequest, Namespace, NodeKind,
        RangeMoveRequest, RangeRequest,
    },
};

pub use crate::event::HydrationRecord;

type NodeId = u64;

#[derive(Clone)]
struct SsrHandle(NodeId);

struct NodeRecord {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    name: Option<String>,
    namespace: Option<Namespace>,
    void: bool,
    text: Option<String>,
    attributes: BTreeMap<String, String>,
    properties: BTreeMap<String, PropertyValue>,
}

impl NodeRecord {
    fn document() -> Self {
        Self {
            kind: NodeKind::Document,
            parent: None,
            children: Vec::new(),
            name: None,
            namespace: None,
            void: false,
            text: None,
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
        }
    }

    fn element(spec: &ElementSpec) -> Self {
        Self {
            kind: NodeKind::Element,
            parent: None,
            children: Vec::new(),
            name: Some(spec.name().to_string()),
            namespace: Some(spec.namespace().clone()),
            void: spec.is_void(),
            text: None,
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
        }
    }

    fn leaf(kind: NodeKind, text: String) -> Self {
        Self {
            kind,
            parent: None,
            children: Vec::new(),
            name: None,
            namespace: None,
            void: false,
            text: Some(text),
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
        }
    }

    fn fragment() -> Self {
        Self {
            kind: NodeKind::Fragment,
            parent: None,
            children: Vec::new(),
            name: None,
            namespace: None,
            void: false,
            text: None,
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
        }
    }
}

struct SsrState {
    next_id: NodeId,
    nodes: BTreeMap<NodeId, NodeRecord>,
    events: Vec<EventRecord>,
}

impl SsrState {
    fn new() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(0, NodeRecord::document());
        Self {
            next_id: 1,
            nodes,
            events: Vec::new(),
        }
    }
}

/// Deterministic in-memory DOM backend for server rendering and tests.
pub struct SsrDom {
    backend: Rc<SsrBackend>,
    context: DomContext,
}

struct SsrBackend {
    id: BackendId,
    state: RefCell<SsrState>,
}

impl SsrDom {
    pub fn new() -> Self {
        let backend = Rc::new(SsrBackend {
            id: BackendId::fresh(),
            state: RefCell::new(SsrState::new()),
        });
        let erased: Rc<dyn DomBackend> = backend.clone();
        Self {
            backend,
            context: DomContext::from_backend(erased),
        }
    }

    pub fn context(&self) -> DomContext {
        self.context.clone()
    }

    pub fn document(&self) -> DomResult<DomDocument> {
        self.context.document()
    }

    pub fn serialize(&self, options: SerializeOptions) -> DomResult<String> {
        self.serialize_node(self.document()?.node(), options)
    }

    pub fn serialize_node(&self, node: &DomNode, options: SerializeOptions) -> DomResult<String> {
        let id = self.backend.node_id(node)?;
        let state = self.backend.state.borrow();
        let mut output = String::new();
        serialize_node(&state, id, &options, None, &mut output)?;
        Ok(output)
    }

    pub fn event_records(&self) -> Vec<EventRecord> {
        self.backend.state.borrow().events.clone()
    }

    pub fn hydration_records(&self) -> Vec<HydrationRecord> {
        self.event_records()
    }
}

impl Default for SsrDom {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialization policy. Raw HTML is deliberately not representable here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerializeOptions {
    pub include_comments: bool,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            include_comments: true,
        }
    }
}

impl SsrBackend {
    fn node(&self, id: NodeId, kind: NodeKind) -> DomNode {
        DomNode::from_raw_with_identity(self.id, kind, id + 1, Rc::new(SsrHandle(id)))
    }

    fn node_id(&self, node: &DomNode) -> DomResult<NodeId> {
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

    fn record<'a>(&self, state: &'a SsrState, id: NodeId) -> DomResult<&'a NodeRecord> {
        state.nodes.get(&id).ok_or(DomError::InvalidHandle {
            backend: self.id.value(),
            kind: "node",
        })
    }

    fn record_mut<'a>(&self, state: &'a mut SsrState, id: NodeId) -> DomResult<&'a mut NodeRecord> {
        state.nodes.get_mut(&id).ok_or(DomError::InvalidHandle {
            backend: self.id.value(),
            kind: "node",
        })
    }

    fn validate_node(&self, state: &SsrState, node: &DomNode) -> DomResult<NodeId> {
        let id = self.node_id(node)?;
        self.record(state, id)?;
        Ok(id)
    }

    fn validate_parent(&self, state: &SsrState, id: NodeId) -> DomResult<()> {
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

    fn is_descendant(state: &SsrState, ancestor: NodeId, mut node: NodeId) -> bool {
        loop {
            if ancestor == node {
                return true;
            }
            let Some(parent) = state.nodes.get(&node).and_then(|record| record.parent) else {
                return false;
            };
            node = parent;
        }
    }

    fn detach(state: &mut SsrState, child: NodeId) -> DomResult<()> {
        let old_parent = state
            .nodes
            .get(&child)
            .ok_or(DomError::InvalidHandle {
                backend: self_id_placeholder(),
                kind: "node",
            })?
            .parent;
        if let Some(parent) = old_parent {
            let parent_record = state
                .nodes
                .get_mut(&parent)
                .ok_or(DomError::ParentMismatch)?;
            let index = parent_record
                .children
                .iter()
                .position(|id| *id == child)
                .ok_or(DomError::ParentMismatch)?;
            parent_record.children.remove(index);
        }
        if let Some(record) = state.nodes.get_mut(&child) {
            record.parent = None;
        }
        Ok(())
    }

    fn insert_one(
        state: &mut SsrState,
        parent: NodeId,
        child: NodeId,
        index: usize,
    ) -> DomResult<()> {
        let parent_record = state
            .nodes
            .get_mut(&parent)
            .ok_or(DomError::ParentMismatch)?;
        let index = index.min(parent_record.children.len());
        parent_record.children.insert(index, child);
        let child_record = state
            .nodes
            .get_mut(&child)
            .ok_or(DomError::ParentMismatch)?;
        child_record.parent = Some(parent);
        Ok(())
    }

    fn move_node(
        &self,
        state: &mut SsrState,
        parent: NodeId,
        child: NodeId,
        index: usize,
    ) -> DomResult<()> {
        if child == parent || Self::is_descendant(state, child, parent) {
            return Err(DomError::Cycle);
        }
        let kind = self.record(state, child)?.kind;
        if kind == NodeKind::Document {
            return Err(DomError::WrongNodeKind {
                expected: "insertable node",
                actual: NodeKind::Document.label(),
            });
        }
        Self::detach(state, child)?;
        Self::insert_one(state, parent, child, index)
    }

    fn insert_request(&self, request: &InsertRequest) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let parent = self.validate_node(&state, &request.parent)?;
        self.validate_parent(&state, parent)?;
        let child = self.validate_node(&state, &request.node)?;
        let reference = request
            .reference
            .as_ref()
            .map(|node| self.validate_node(&state, node))
            .transpose()?;
        let index = if let Some(reference) = reference {
            let parent_record = self.record(&state, parent)?;
            parent_record
                .children
                .iter()
                .position(|id| *id == reference)
                .ok_or(DomError::ReferenceNotChild)?
        } else {
            self.record(&state, parent)?.children.len()
        };
        if let Some(reference) = reference
            && self.record(&state, reference)?.parent != Some(parent)
        {
            return Err(DomError::ReferenceNotChild);
        }

        if self.record(&state, child)?.kind == NodeKind::Fragment {
            let children = self.record(&state, child)?.children.clone();
            for moved in children {
                let current_index = self
                    .record(&state, parent)?
                    .children
                    .iter()
                    .position(|id| *id == reference.unwrap_or(moved))
                    .unwrap_or_else(|| {
                        self.record(&state, parent)
                            .map(|record| record.children.len())
                            .unwrap_or(0)
                    });
                self.move_node(&mut state, parent, moved, current_index)?;
            }
            Ok(())
        } else {
            self.move_node(&mut state, parent, child, index)
        }
    }
}

fn self_id_placeholder() -> u64 {
    0
}

impl DomBackend for SsrBackend {
    fn backend_id(&self) -> BackendId {
        self.id
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
        self.insert_request(&InsertRequest::append(parent, child))
    }

    fn insert_before(&self, request: &InsertRequest) -> DomResult<()> {
        self.insert_request(request)
    }

    fn remove(&self, node: &DomNode) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let id = self.validate_node(&state, node)?;
        if id == 0 {
            return Err(DomError::CannotRemoveDocument);
        }
        if self.record(&state, id)?.parent.is_none() {
            return Err(DomError::NoParent);
        }
        Self::detach(&mut state, id)
    }

    fn parent(&self, node: &DomNode) -> DomResult<Option<DomNode>> {
        let state = self.state.borrow();
        let id = self.validate_node(&state, node)?;
        let parent = self.record(&state, id)?.parent;
        Ok(parent.map(|parent| {
            let kind = state
                .nodes
                .get(&parent)
                .map(|record| record.kind)
                .unwrap_or(NodeKind::Document);
            self.node(parent, kind)
        }))
    }

    fn children(&self, node: &DomNode) -> DomResult<Vec<DomNode>> {
        let state = self.state.borrow();
        let id = self.validate_node(&state, node)?;
        let children = self.record(&state, id)?.children.clone();
        Ok(children
            .into_iter()
            .filter_map(|child| {
                state
                    .nodes
                    .get(&child)
                    .map(|record| self.node(child, record.kind))
            })
            .collect())
    }

    fn validate_range(&self, request: &RangeRequest) -> DomResult<()> {
        let state = self.state.borrow();
        let parent = self.validate_node(&state, &request.parent)?;
        self.validate_parent(&state, parent)?;
        let start = self.validate_node(&state, &request.start)?;
        let end = self.validate_node(&state, &request.end)?;
        if self.record(&state, start)?.parent != Some(parent)
            || self.record(&state, end)?.parent != Some(parent)
        {
            return Err(DomError::ReferenceNotChild);
        }
        let children = &self.record(&state, parent)?.children;
        let start_index = children.iter().position(|id| *id == start);
        let end_index = children.iter().position(|id| *id == end);
        if start_index.is_none() || end_index.is_none() || start_index > end_index {
            return Err(DomError::ParentMismatch);
        }
        Ok(())
    }

    fn move_range(&self, request: &RangeMoveRequest) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let source_parent = self.validate_node(&state, &request.source.parent)?;
        let start = self.validate_node(&state, &request.source.start)?;
        let end = self.validate_node(&state, &request.source.end)?;
        let target_parent = self.validate_node(&state, &request.target_parent)?;
        let reference = self.validate_node(&state, &request.reference)?;
        self.validate_parent(&state, target_parent)?;
        if self.record(&state, reference)?.parent != Some(target_parent) {
            return Err(DomError::ReferenceNotChild);
        }
        let children = self.record(&state, source_parent)?.children.clone();
        let start_index = children
            .iter()
            .position(|id| *id == start)
            .ok_or(DomError::ReferenceNotChild)?;
        let end_index = children
            .iter()
            .position(|id| *id == end)
            .ok_or(DomError::ReferenceNotChild)?;
        if start_index > end_index {
            return Err(DomError::ParentMismatch);
        }
        let moving = children[start_index..=end_index].to_vec();
        if moving.contains(&reference) {
            return Err(DomError::ParentMismatch);
        }
        for child in &moving {
            Self::detach(&mut state, *child)?;
        }
        let target_index = self
            .record(&state, target_parent)?
            .children
            .iter()
            .position(|id| *id == reference)
            .ok_or(DomError::ReferenceNotChild)?;
        for (offset, child) in moving.into_iter().enumerate() {
            Self::insert_one(&mut state, target_parent, child, target_index + offset)?;
        }
        Ok(())
    }

    fn set_attribute(&self, request: &AttributeRequest) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let id = self.validate_node(&state, request.element.node())?;
        let name = request.target.name();
        if name.is_empty() {
            return Err(DomError::AttributeNameEmpty);
        }
        let element = self.record_mut(&mut state, id)?;
        if element.kind != NodeKind::Element {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: element.kind.label(),
            });
        }
        match &request.value {
            AttributeValue::Removed => {
                element.attributes.remove(name);
            }
            AttributeValue::Empty => {
                element.attributes.insert(name.to_string(), String::new());
            }
            AttributeValue::Text(value) => {
                element.attributes.insert(name.to_string(), value.clone());
            }
            AttributeValue::ClassTokens { add, remove } => {
                let mut classes = element
                    .attributes
                    .get(name)
                    .map_or_else(String::new, Clone::clone)
                    .split_whitespace()
                    .map(ToOwned::to_owned)
                    .collect::<std::collections::BTreeSet<_>>();
                classes.extend(add.iter().cloned());
                for class_name in remove {
                    classes.remove(class_name);
                }
                let value = classes.into_iter().collect::<Vec<_>>().join(" ");
                if value.is_empty() {
                    element.attributes.remove(name);
                } else {
                    element.attributes.insert(name.to_string(), value);
                }
            }
        }
        Ok(())
    }

    fn set_property(&self, request: &PropertyRequest) -> DomResult<()> {
        let mut state = self.state.borrow_mut();
        let id = self.validate_node(&state, request.element.node())?;
        if request.name.is_empty() {
            return Err(DomError::AttributeNameEmpty);
        }
        let element = self.record_mut(&mut state, id)?;
        if element.kind != NodeKind::Element {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: element.kind.label(),
            });
        }
        if request.value == PropertyValue::Removed {
            element.properties.remove(&request.name);
        } else {
            element
                .properties
                .insert(request.name.clone(), request.value.clone());
        }
        Ok(())
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
        let mut state = self.state.borrow_mut();
        let id = self.validate_node(&state, element.node())?;
        let record = self.record_mut(&mut state, id)?;
        if record.kind != NodeKind::Element {
            return Err(DomError::WrongNodeKind {
                expected: NodeKind::Element.label(),
                actual: record.kind.label(),
            });
        }
        let current = record.attributes.get("style").cloned().unwrap_or_default();
        let mut declarations = BTreeMap::new();
        for declaration in current.split(';') {
            let Some((property, property_value)) = declaration.split_once(':') else {
                continue;
            };
            let property = property.trim();
            if !property.is_empty() {
                declarations.insert(property.to_string(), property_value.trim().to_string());
            }
        }
        match value {
            Some(value) => {
                declarations.insert(name.to_string(), value.to_string());
            }
            None => {
                declarations.remove(name);
            }
        }
        if declarations.is_empty() {
            record.attributes.remove("style");
        } else {
            let style = declarations
                .into_iter()
                .map(|(property, value)| format!("{property}:{value};"))
                .collect::<String>();
            record.attributes.insert("style".to_string(), style);
        }
        Ok(())
    }

    fn listen(&self, request: &PhysicalEventRequest) -> DomResult<HostResource<'static>> {
        request.validate()?;
        let state = &mut *self.state.borrow_mut();
        let target = self.validate_node(state, request.target.node())?;
        state.events.push(EventRecord {
            target_backend: self.id.value(),
            target_identity: request.target.node().identity(),
            target_kind: self.record(state, target)?.kind.label(),
            spec: request.spec.clone(),
        });
        Ok(HostResource::inert())
    }
}

fn serialize_node(
    state: &SsrState,
    id: NodeId,
    options: &SerializeOptions,
    parent_namespace: Option<&Namespace>,
    output: &mut String,
) -> DomResult<()> {
    let node = state.nodes.get(&id).ok_or(DomError::InvalidHandle {
        backend: 0,
        kind: "node",
    })?;
    match node.kind {
        NodeKind::Document | NodeKind::Fragment => {
            for child in &node.children {
                serialize_node(state, *child, options, parent_namespace, output)?;
            }
        }
        NodeKind::Text => escape_text(node.text.as_deref().unwrap_or_default(), output),
        NodeKind::Comment => {
            if options.include_comments {
                output.push_str("<!--");
                escape_comment(node.text.as_deref().unwrap_or_default(), output);
                output.push_str("-->");
            }
        }
        NodeKind::Element => {
            let name = node.name.as_deref().unwrap_or_default();
            let namespace = node.namespace.as_ref().ok_or(DomError::Backend {
                operation: "serialize",
                message: String::from("element namespace is missing"),
            })?;
            output.push('<');
            escape_name(name, output);
            if parent_namespace != Some(namespace)
                && let Some(uri) = namespace.uri()
            {
                output.push_str(" xmlns=\"");
                escape_attribute(uri, output);
                output.push('"');
            }
            for (key, value) in &node.attributes {
                output.push(' ');
                escape_name(key, output);
                output.push_str("=\"");
                escape_attribute(value, output);
                output.push('"');
            }
            output.push('>');
            if !node.void {
                for child in &node.children {
                    serialize_node(state, *child, options, Some(namespace), output)?;
                }
                output.push_str("</");
                escape_name(name, output);
                output.push('>');
            }
        }
    }
    Ok(())
}

fn escape_text(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn escape_attribute(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
}

fn escape_name(value: &str, output: &mut String) {
    escape_attribute(value, output);
}

fn escape_comment(value: &str, output: &mut String) {
    output.push_str(&value.replace("--", "- -"));
    if value.ends_with('-') {
        output.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::{SerializeOptions, SsrDom};
    use crate::{
        attribute::{
            AttributeRequest, AttributeTarget, AttributeValue, PropertyRequest, PropertyValue,
        },
        event::{EventKind, EventSpec, PhysicalEventRequest},
        tree::{ElementSpec, InsertRequest, Namespace, RangeRequest},
    };

    #[test]
    fn ssr_tree_serializes_deterministically_and_escapes_values() {
        let dom = SsrDom::new();
        let context = dom.context();
        let document = context.document().expect("document should exist");
        let root = context
            .create_element(ElementSpec::new("div"))
            .expect("element should exist");
        let text = context.create_text("<&>").expect("text should exist");
        context
            .set_attribute(AttributeRequest::new(
                &root,
                AttributeTarget::named("data-value"),
                AttributeValue::text("\"<&"),
            ))
            .expect("attribute should be written");
        context
            .set_attribute(AttributeRequest::new(
                &root,
                AttributeTarget::Style,
                AttributeValue::text("color:red<&"),
            ))
            .expect("style should be written");
        context
            .set_property(PropertyRequest::new(
                &root,
                "value",
                PropertyValue::string("not an attribute"),
            ))
            .expect("property should be written");
        context
            .append(root.node(), &text)
            .expect("append should work");
        context
            .append(document.node(), root.node())
            .expect("root should attach");
        assert_eq!(
            dom.serialize(SerializeOptions::default())
                .expect("serialization should work"),
            "<div data-value=\"&quot;&lt;&amp;\" style=\"color:red&lt;&amp;\">&lt;&amp;&gt;</div>"
        );
    }

    #[test]
    fn ssr_style_property_updates_inline_style_and_has_no_body_target() {
        let dom = SsrDom::new();
        let context = dom.context();
        let root = context
            .create_element(ElementSpec::new("div"))
            .expect("element should exist");
        context
            .set_attribute(AttributeRequest::new(
                &root,
                AttributeTarget::Style,
                AttributeValue::text("color:red;"),
            ))
            .expect("style should be written");
        context
            .set_style_property(&root, "--dynamic", Some("blue"))
            .expect("style property should be written");
        assert_eq!(
            dom.serialize_node(&root.node().clone(), SerializeOptions::default())
                .expect("element should serialize"),
            "<div style=\"--dynamic:blue;color:red;\"></div>"
        );
        context
            .set_style_property(&root, "--dynamic", None)
            .expect("style property should be removed");
        assert_eq!(
            dom.serialize_node(&root.node().clone(), SerializeOptions::default())
                .expect("element should serialize"),
            "<div style=\"color:red;\"></div>"
        );
        assert!(
            context
                .document_body()
                .expect("SSR body capability should be queryable")
                .is_none()
        );
    }

    #[test]
    fn ssr_handles_fragments_ranges_void_and_namespaces() {
        let dom = SsrDom::new();
        let context = dom.context();
        let document = context.document().expect("document should exist");
        let parent = context
            .create_element(ElementSpec::new("main"))
            .expect("parent should exist");
        let fragment = context.create_fragment().expect("fragment should exist");
        let first = context
            .create_comment("a--b")
            .expect("comment should exist");
        let second = context
            .create_element(ElementSpec::new("br"))
            .expect("void element should exist");
        context.append(&fragment, &first).expect("fragment append");
        context
            .append(&fragment, second.node())
            .expect("fragment append");
        context
            .append(parent.node(), &fragment)
            .expect("fragment move");
        let range = context
            .range(RangeRequest {
                parent: parent.node().clone(),
                start: first.clone(),
                end: second.node().clone(),
            })
            .expect("range should validate");
        assert_eq!(range.nodes().expect("range should read").len(), 2);
        let svg = context
            .create_element(ElementSpec::namespaced("svg", Namespace::Svg, false))
            .expect("svg should exist");
        context
            .append(parent.node(), svg.node())
            .expect("svg append");
        context
            .append(document.node(), parent.node())
            .expect("document append");
        assert_eq!(
            dom.serialize(SerializeOptions::default())
                .expect("serialization should work"),
            "<main><!--a- -b--><br><svg xmlns=\"http://www.w3.org/2000/svg\"></svg></main>"
        );
        range.remove().expect("range removal should work");
        assert_eq!(context.children(parent.node()).expect("children").len(), 1);
    }

    #[test]
    fn cross_context_and_wrong_parent_operations_are_structured_errors() {
        let first = SsrDom::new();
        let second = SsrDom::new();
        let first_document = first.context().document().expect("document");
        let second_text = second
            .context()
            .create_text("x")
            .expect("text should exist");
        let error = first
            .context()
            .append(first_document.node(), &second_text)
            .expect_err("cross context append should fail");
        assert!(matches!(error, crate::DomError::CrossContext { .. }));

        let detached = first
            .context()
            .create_text("detached")
            .expect("text should exist");
        assert!(matches!(
            first.context().remove(&detached),
            Err(crate::DomError::NoParent)
        ));
    }

    #[test]
    fn handles_keep_identity_across_queries_but_not_contexts() {
        let first = SsrDom::new();
        let first_context = first.context();
        let first_document = first_context.document().expect("document");
        let text = first_context.create_text("x").expect("text");
        first_context
            .append(first_document.node(), &text)
            .expect("append");

        let same_document = first.context().document().expect("document");
        let queried_child = first_context
            .children(first_document.node())
            .expect("children")
            .pop()
            .expect("child");
        assert!(first_context.same_backend(&first.context()));
        assert_eq!(first_document.node(), same_document.node());
        assert_eq!(text, queried_child);

        let second = SsrDom::new();
        let second_document = second.document().expect("document");
        assert_ne!(first_document.node(), second_document.node());
        assert!(!first_context.same_backend(&second.context()));
    }

    #[test]
    fn ssr_listener_is_recorded_but_inert() {
        let dom = SsrDom::new();
        let context = dom.context();
        let element = context
            .create_element(ElementSpec::new("button"))
            .expect("button should exist");
        let resource = context
            .listen(PhysicalEventRequest::new(
                &element,
                EventSpec::new("click", EventKind::Mouse),
            ))
            .expect("SSR listener should be inert");
        assert!(!resource.is_active());
        assert_eq!(dom.event_records().len(), 1);
    }

    #[test]
    fn insert_before_moves_existing_node_once() {
        let dom = SsrDom::new();
        let context = dom.context();
        let parent = context
            .create_element(ElementSpec::new("div"))
            .expect("parent");
        let first = context.create_text("first").expect("first");
        let second = context.create_text("second").expect("second");
        context.append(parent.node(), &first).expect("append");
        context.append(parent.node(), &second).expect("append");
        context
            .insert_before(InsertRequest::before(parent.node(), &second, &first))
            .expect("move before");
        assert_eq!(
            context.children(parent.node()).expect("children"),
            vec![second, first]
        );
    }

    #[test]
    fn range_move_preserves_a_contiguous_block_and_identity() {
        let dom = SsrDom::new();
        let context = dom.context();
        let parent = context
            .create_element(ElementSpec::new("div"))
            .expect("parent");
        let first_start = context.create_comment("first-start").expect("start");
        let first_node = context.create_text("first").expect("first");
        let first_end = context.create_comment("first-end").expect("end");
        let second_start = context.create_comment("second-start").expect("start");
        let second_node = context.create_text("second").expect("second");
        let second_end = context.create_comment("second-end").expect("end");
        let reference = context
            .create_element(ElementSpec::new("hr"))
            .expect("reference");
        for node in [
            &first_start,
            &first_node,
            &first_end,
            &second_start,
            &second_node,
            &second_end,
            reference.node(),
        ] {
            context.append(parent.node(), node).expect("append");
        }
        let range = context
            .range(RangeRequest {
                parent: parent.node().clone(),
                start: first_start.clone(),
                end: first_end.clone(),
            })
            .expect("range");
        range
            .move_before(parent.node(), reference.node())
            .expect("range move");
        assert_eq!(
            context.children(parent.node()).expect("children"),
            vec![
                second_start,
                second_node,
                second_end,
                first_start.clone(),
                first_node,
                first_end,
                reference.node().clone(),
            ]
        );
        let children_before_failed_move = context.children(parent.node()).expect("children");
        let error = range
            .move_before(parent.node(), &first_start)
            .expect_err("a range cannot move before a reference inside itself");
        assert_eq!(error, crate::DomError::ParentMismatch);
        assert_eq!(
            context.children(parent.node()).expect("children"),
            children_before_failed_move
        );
    }
}
