use wasm_bindgen::JsCast;
use web_sys::{DocumentFragment, HtmlElement, Node, Text};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::node::{DomDocument, DomElement, DomNode, ElementSpec, NodeKind},
    runtime::tree::{InsertRequest, RangeMoveRequest, RangeRequest},
};

use super::backend::BrowserBackend;

pub(super) fn document(backend: &BrowserBackend) -> DomResult<DomDocument> {
    DomDocument::from_node(backend.handle(backend.document_node(), NodeKind::Document))
}

pub(super) fn document_body(backend: &BrowserBackend) -> DomResult<Option<DomElement>> {
    backend
        .raw_document()
        .body()
        .map(|body| {
            let node: Node = body.into();
            DomElement::from_node(backend.handle(node, NodeKind::Element))
        })
        .transpose()
}

pub(super) fn create_element(
    backend: &BrowserBackend,
    spec: &ElementSpec,
) -> DomResult<DomElement> {
    let document = backend.raw_document();
    let element = match spec.namespace().uri() {
        Some(uri) => document
            .create_element_ns(Some(uri), spec.name())
            .map_err(|error| BrowserBackend::error("create_element", error))?,
        None => document
            .create_element(spec.name())
            .map_err(|error| BrowserBackend::error("create_element", error))?,
    };
    DomElement::from_node(backend.handle(element.into(), NodeKind::Element))
}

pub(super) fn create_text(backend: &BrowserBackend, value: &str) -> DomResult<DomNode> {
    Ok(backend.handle(
        backend.raw_document().create_text_node(value).into(),
        NodeKind::Text,
    ))
}

pub(super) fn set_text(backend: &BrowserBackend, node: &DomNode, value: &str) -> DomResult<()> {
    backend
        .validate_node(node)?
        .dyn_into::<Text>()
        .map_err(|_| DomError::WrongNodeKind {
            expected: NodeKind::Text.label(),
            actual: node.kind().label(),
        })?
        .set_data(value);
    Ok(())
}

pub(super) fn create_comment(backend: &BrowserBackend, value: &str) -> DomResult<DomNode> {
    Ok(backend.handle(
        backend.raw_document().create_comment(value).into(),
        NodeKind::Comment,
    ))
}

pub(super) fn create_fragment(backend: &BrowserBackend) -> DomResult<DomNode> {
    Ok(backend.handle(
        backend.raw_document().create_document_fragment().into(),
        NodeKind::Fragment,
    ))
}

pub(super) fn append(backend: &BrowserBackend, parent: &DomNode, child: &DomNode) -> DomResult<()> {
    backend
        .validate_node(parent)?
        .append_child(&backend.validate_node(child)?)
        .map(|_| ())
        .map_err(|error| BrowserBackend::error("append", error))
}

pub(super) fn insert_before(backend: &BrowserBackend, request: &InsertRequest) -> DomResult<()> {
    let parent = backend.validate_node(&request.parent)?;
    let child = backend.validate_node(&request.node)?;
    let reference = request
        .reference
        .as_ref()
        .map(|node| backend.validate_node(node))
        .transpose()?;
    parent
        .insert_before(&child, reference.as_ref())
        .map(|_| ())
        .map_err(|error| BrowserBackend::error("insert_before", error))
}

pub(super) fn remove(backend: &BrowserBackend, node: &DomNode) -> DomResult<()> {
    let node_value = backend.validate_node(node)?;
    let parent = node_value.parent_node().ok_or(DomError::NoParent)?;
    parent
        .remove_child(&node_value)
        .map(|_| ())
        .map_err(|error| BrowserBackend::error("remove", error))
}

pub(super) fn parent(backend: &BrowserBackend, node: &DomNode) -> DomResult<Option<DomNode>> {
    let parent = backend.validate_node(node)?.parent_node();
    parent
        .map(|parent| {
            let document_node = backend.document_node();
            let kind = if parent.is_same_node(Some(&document_node)) {
                NodeKind::Document
            } else {
                BrowserBackend::kind(&parent)?
            };
            Ok(backend.handle(parent, kind))
        })
        .transpose()
}

pub(super) fn children(backend: &BrowserBackend, node: &DomNode) -> DomResult<Vec<DomNode>> {
    let children = backend.validate_node(node)?.child_nodes();
    let mut result = Vec::with_capacity(children.length() as usize);
    for index in 0..children.length() {
        if let Some(child) = children.item(index) {
            let kind = BrowserBackend::kind(&child)?;
            result.push(backend.handle(child, kind));
        }
    }
    Ok(result)
}

pub(super) fn validate_range(backend: &BrowserBackend, request: &RangeRequest) -> DomResult<()> {
    let parent = backend.validate_node(&request.parent)?;
    let start = backend.validate_node(&request.start)?;
    let end = backend.validate_node(&request.end)?;
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

pub(super) fn move_range(backend: &BrowserBackend, request: &RangeMoveRequest) -> DomResult<()> {
    validate_range(backend, &request.source)?;
    let source_parent = backend.validate_node(&request.source.parent)?;
    let start = backend.validate_node(&request.source.start)?;
    let end = backend.validate_node(&request.source.end)?;
    let target_parent = backend.validate_node(&request.target_parent)?;
    let reference = backend.validate_node(&request.reference)?;
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
    let fragment: DocumentFragment = backend.raw_document().create_document_fragment();
    for node in moving {
        fragment
            .append_child(&node)
            .map_err(|error| BrowserBackend::error("move_range.append", error))?;
    }
    target_parent
        .insert_before(&fragment, Some(&reference))
        .map(|_| ())
        .map_err(|error| BrowserBackend::error("move_range.insert", error))
}

pub(super) fn focus(backend: &BrowserBackend, element: &DomElement) -> DomResult<()> {
    let node = backend.node(element.node())?;
    if !node.is_connected() {
        return Err(DomError::Detached {
            kind: NodeKind::Element.label(),
        });
    }
    node.dyn_into::<HtmlElement>()
        .map_err(|_| DomError::WrongNodeKind {
            expected: "focusable html element",
            actual: NodeKind::Element.label(),
        })?
        .focus()
        .map_err(|error| BrowserBackend::error("focus", error))
}

pub(super) fn active_element(backend: &BrowserBackend) -> DomResult<Option<DomElement>> {
    backend
        .raw_document()
        .active_element()
        .map(|element| {
            let node = backend.handle(element.into(), NodeKind::Element);
            DomElement::from_node(node)
        })
        .transpose()
}

pub(super) fn contains(
    backend: &BrowserBackend,
    parent: &DomElement,
    child: &DomNode,
) -> DomResult<bool> {
    Ok(backend
        .element(parent.node())?
        .contains(Some(&backend.validate_node(child)?)))
}

pub(super) fn document_hidden(backend: &BrowserBackend) -> DomResult<Option<bool>> {
    Ok(Some(backend.raw_document().hidden()))
}
