use crate::model::node::DomNode;

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
