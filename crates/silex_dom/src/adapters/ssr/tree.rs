use crate::{
    diagnostics::error::{DomError, DomResult},
    model::node::{DomNode, NodeKind},
    runtime::tree::{InsertRequest, RangeMoveRequest, RangeRequest},
};

use super::{
    backend::SsrBackend,
    state::{NodeId, SsrState},
};

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

fn detach(backend: &SsrBackend, state: &mut SsrState, child: NodeId) -> DomResult<()> {
    let old_parent = backend.record(state, child)?.parent;
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

fn insert_one(state: &mut SsrState, parent: NodeId, child: NodeId, index: usize) -> DomResult<()> {
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
    backend: &SsrBackend,
    state: &mut SsrState,
    parent: NodeId,
    child: NodeId,
    reference: Option<NodeId>,
) -> DomResult<()> {
    if child == parent || is_descendant(state, child, parent) {
        return Err(DomError::Cycle);
    }
    let kind = backend.record(state, child)?.kind;
    if kind == NodeKind::Document {
        return Err(DomError::WrongNodeKind {
            expected: "insertable node",
            actual: NodeKind::Document.label(),
        });
    }
    detach(backend, state, child)?;
    let index = reference
        .map(|reference| {
            backend
                .record(state, parent)?
                .children
                .iter()
                .position(|id| *id == reference)
                .ok_or(DomError::ReferenceNotChild)
        })
        .transpose()?
        .unwrap_or(backend.record(state, parent)?.children.len());
    insert_one(state, parent, child, index)
}

pub(super) fn insert_request(backend: &SsrBackend, request: &InsertRequest) -> DomResult<()> {
    let mut state = backend.state.borrow_mut();
    let parent = backend.validate_node(&state, &request.parent)?;
    backend.validate_parent(&state, parent)?;
    let child = backend.validate_node(&state, &request.node)?;
    let reference = request
        .reference
        .as_ref()
        .map(|node| backend.validate_node(&state, node))
        .transpose()?;
    if let Some(reference) = reference
        && backend.record(&state, reference)?.parent != Some(parent)
    {
        return Err(DomError::ReferenceNotChild);
    }
    if reference == Some(child) {
        return Ok(());
    }

    if backend.record(&state, child)?.kind == NodeKind::Fragment {
        let children = backend.record(&state, child)?.children.clone();
        for moved in children {
            move_node(backend, &mut state, parent, moved, reference)?;
        }
        Ok(())
    } else {
        move_node(backend, &mut state, parent, child, reference)
    }
}

pub(super) fn remove(backend: &SsrBackend, node: &DomNode) -> DomResult<()> {
    let mut state = backend.state.borrow_mut();
    let id = backend.validate_node(&state, node)?;
    if id == 0 {
        return Err(DomError::CannotRemoveDocument);
    }
    if backend.record(&state, id)?.parent.is_none() {
        return Err(DomError::NoParent);
    }
    detach(backend, &mut state, id)
}

pub(super) fn parent(backend: &SsrBackend, node: &DomNode) -> DomResult<Option<DomNode>> {
    let state = backend.state.borrow();
    let id = backend.validate_node(&state, node)?;
    let parent = backend.record(&state, id)?.parent;
    Ok(parent.map(|parent| {
        let kind = state
            .nodes
            .get(&parent)
            .map(|record| record.kind)
            .unwrap_or(NodeKind::Document);
        backend.node(parent, kind)
    }))
}

pub(super) fn children(backend: &SsrBackend, node: &DomNode) -> DomResult<Vec<DomNode>> {
    let state = backend.state.borrow();
    let id = backend.validate_node(&state, node)?;
    let children = backend.record(&state, id)?.children.clone();
    Ok(children
        .into_iter()
        .filter_map(|child| {
            state
                .nodes
                .get(&child)
                .map(|record| backend.node(child, record.kind))
        })
        .collect())
}

pub(super) fn validate_range(backend: &SsrBackend, request: &RangeRequest) -> DomResult<()> {
    let state = backend.state.borrow();
    let parent = backend.validate_node(&state, &request.parent)?;
    backend.validate_parent(&state, parent)?;
    let start = backend.validate_node(&state, &request.start)?;
    let end = backend.validate_node(&state, &request.end)?;
    if backend.record(&state, start)?.parent != Some(parent)
        || backend.record(&state, end)?.parent != Some(parent)
    {
        return Err(DomError::ReferenceNotChild);
    }
    let children = &backend.record(&state, parent)?.children;
    let start_index = children.iter().position(|id| *id == start);
    let end_index = children.iter().position(|id| *id == end);
    if start_index.is_none() || end_index.is_none() || start_index > end_index {
        return Err(DomError::ParentMismatch);
    }
    Ok(())
}

pub(super) fn move_range(backend: &SsrBackend, request: &RangeMoveRequest) -> DomResult<()> {
    let mut state = backend.state.borrow_mut();
    let source_parent = backend.validate_node(&state, &request.source.parent)?;
    let start = backend.validate_node(&state, &request.source.start)?;
    let end = backend.validate_node(&state, &request.source.end)?;
    let target_parent = backend.validate_node(&state, &request.target_parent)?;
    let reference = backend.validate_node(&state, &request.reference)?;
    backend.validate_parent(&state, target_parent)?;
    if backend.record(&state, reference)?.parent != Some(target_parent) {
        return Err(DomError::ReferenceNotChild);
    }
    let children = backend.record(&state, source_parent)?.children.clone();
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
        detach(backend, &mut state, *child)?;
    }
    let target_index = backend
        .record(&state, target_parent)?
        .children
        .iter()
        .position(|id| *id == reference)
        .ok_or(DomError::ReferenceNotChild)?;
    for (offset, child) in moving.into_iter().enumerate() {
        insert_one(&mut state, target_parent, child, target_index + offset)?;
    }
    Ok(())
}
