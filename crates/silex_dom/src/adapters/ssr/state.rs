use std::collections::BTreeMap;

use crate::model::{
    attribute::PropertyValue,
    event::EventRecord,
    node::{ElementSpec, Namespace, NodeKind},
};

pub(super) type NodeId = u64;

#[derive(Clone)]
pub(super) struct SsrHandle(pub(super) NodeId);

pub(super) struct NodeRecord {
    pub(super) kind: NodeKind,
    pub(super) parent: Option<NodeId>,
    pub(super) children: Vec<NodeId>,
    pub(super) name: Option<String>,
    pub(super) namespace: Option<Namespace>,
    pub(super) void: bool,
    pub(super) text: Option<String>,
    pub(super) attributes: BTreeMap<String, String>,
    pub(super) properties: BTreeMap<String, PropertyValue>,
}

impl NodeRecord {
    pub(super) fn document() -> Self {
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

    pub(super) fn element(spec: &ElementSpec) -> Self {
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

    pub(super) fn leaf(kind: NodeKind, text: String) -> Self {
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

    pub(super) fn fragment() -> Self {
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

pub(super) struct SsrState {
    pub(super) next_id: NodeId,
    pub(super) next_event_id: u64,
    pub(super) nodes: BTreeMap<NodeId, NodeRecord>,
    pub(super) events: Vec<EventRecord>,
}

impl SsrState {
    pub(super) fn new() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(0, NodeRecord::document());
        Self {
            next_id: 1,
            next_event_id: 1,
            nodes,
            events: Vec::new(),
        }
    }
}
