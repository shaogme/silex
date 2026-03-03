use crate::core::algorithm::{GraphStorage, NodeState};
use crate::core::arena::{Arena, Index as NodeId, SparseSecondaryMap};
use crate::core::value::{AnyValue, OnceThunk, ThunkValue};
use crate::{DependencyList, NodeList};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) struct ReactiveNode {
    pub(crate) state: NodeState,
    pub(crate) signal: Option<SignalData>,
    pub(crate) effect: Option<EffectData>,
}

pub(crate) enum ExtraData {
    Callback(CallbackData),
    NodeRef(NodeRefData),
    StoredValue(StoredValueData),
    Closure(ClosureData),
    Op(OpData),
}

pub(crate) struct Storage {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux, 32>,
    pub(crate) reactive: SparseSecondaryMap<ReactiveNode, 64>,
    pub(crate) extras: SparseSecondaryMap<ExtraData, 32>,

    #[cfg(debug_assertions)]
    pub(crate) dead_node_labels: SparseSecondaryMap<String>,
}

impl Storage {
    pub(crate) fn new() -> Self {
        Self {
            graph: Arena::new(),
            node_aux: SparseSecondaryMap::new(),
            reactive: SparseSecondaryMap::new(),
            extras: SparseSecondaryMap::new(),
            #[cfg(debug_assertions)]
            dead_node_labels: SparseSecondaryMap::new(),
        }
    }

    pub(crate) fn try_aux_mut(&self, id: NodeId) -> Option<&mut NodeAux> {
        if self.node_aux.get(id).is_none() {
            if self.graph.get(id).is_none() {
                return None;
            }
            self.node_aux.insert(id, NodeAux::default());
        }
        self.node_aux.get_mut(id)
    }
}

impl GraphStorage for Storage {
    fn get_state(&self, id: NodeId) -> NodeState {
        self.reactive
            .get(id)
            .map(|n| n.state)
            .unwrap_or(NodeState::Clean)
    }

    fn set_state(&self, id: NodeId, state: NodeState) {
        if let Some(n) = self.reactive.get_mut(id) {
            n.state = state;
        } else {
            self.reactive.insert(
                id,
                ReactiveNode {
                    state,
                    signal: None,
                    effect: None,
                },
            );
        }
    }

    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        if let Some(n) = self.reactive.get(id)
            && let Some(signal) = &n.signal
        {
            signal.subscribers.for_each(|&n| dest.push(n));
        }
    }

    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        if let Some(n) = self.reactive.get(id)
            && let Some(eff) = &n.effect
        {
            eff.dependencies.for_each(|(n, _)| dest.push(*n));
        }
    }

    fn is_effect(&self, id: NodeId) -> bool {
        self.reactive
            .get(id)
            .map_or(false, |n| n.effect.is_some() && n.signal.is_none())
    }

    fn check_dependencies_changed(&self, id: NodeId) -> bool {
        if let Some(n) = self.reactive.get(id)
            && let Some(eff) = &n.effect
        {
            let mut found_change = false;
            eff.dependencies.for_each(|(dep_id, expected_ver)| {
                if found_change {
                    return;
                }
                if let Some(dep_node) = self.reactive.get(*dep_id)
                    && let Some(s) = &dep_node.signal
                {
                    if s.version != *expected_ver {
                        found_change = true;
                    }
                } else {
                    found_change = true;
                }
            });
            found_change
        } else {
            false
        }
    }
}

/// 辅助数据结构，存储“冷数据” (Cold Data)
#[derive(Default)]
pub(crate) struct NodeAux {
    pub(crate) children: Vec<NodeId>,
    pub(crate) cleanups: CleanupList,
    pub(crate) context: Option<HashMap<TypeId, Box<dyn Any>>>,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
}

/// 响应式节点通用结构体 (Metadata)。
/// 仅保留最核心的“热数据”以减小体积。
pub(crate) struct Node {
    pub(crate) parent: Option<NodeId>,
    #[cfg(debug_assertions)]
    pub(crate) defined_at: Option<&'static std::panic::Location<'static>>,
}

impl Node {
    pub(crate) fn new() -> Self {
        Self {
            parent: None,
            #[cfg(debug_assertions)]
            defined_at: None,
        }
    }
}

#[derive(Default)]
pub(crate) enum CleanupList {
    #[default]
    Empty,
    Single(OnceThunk),
    Many(Vec<OnceThunk>),
}

impl CleanupList {
    pub(crate) fn push(&mut self, f: OnceThunk) {
        if let Self::Many(vec) = self {
            vec.push(f);
            return;
        }

        let old = std::mem::take(self);
        match old {
            Self::Empty => *self = Self::Single(f),
            Self::Single(prev) => *self = Self::Many(vec![prev, f]),
            Self::Many(_) => unreachable!(),
        }
    }
}

impl IntoIterator for CleanupList {
    type Item = OnceThunk;
    type IntoIter = CleanupListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            CleanupList::Empty => CleanupListIntoIter::Empty,
            CleanupList::Single(f) => CleanupListIntoIter::Single(Some(f)),
            CleanupList::Many(vec) => CleanupListIntoIter::Many(vec.into_iter()),
        }
    }
}

pub(crate) enum CleanupListIntoIter {
    Empty,
    Single(Option<OnceThunk>),
    Many(std::vec::IntoIter<OnceThunk>),
}

impl Iterator for CleanupListIntoIter {
    type Item = OnceThunk;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Single(opt) => opt.take(),
            Self::Many(iter) => iter.next(),
        }
    }
}

pub(crate) struct SignalData {
    pub(crate) value: AnyValue,
    pub(crate) subscribers: NodeList,
    pub(crate) last_tracked_by: Option<(NodeId, u32)>,
    pub(crate) version: u32,
}

pub(crate) struct EffectData {
    pub(crate) computation: Option<ThunkValue>,
    pub(crate) dependencies: DependencyList,
    pub(crate) effect_version: u32,
}

pub(crate) struct CallbackData {
    pub(crate) f: Rc<dyn Fn(Box<dyn Any>)>,
}

pub(crate) struct NodeRefData {
    pub(crate) element: Option<Box<dyn Any>>,
}

pub(crate) struct StoredValueData {
    pub(crate) value: AnyValue,
}

pub(crate) struct ClosureData {
    pub(crate) f: Box<dyn Any>,
}

pub(crate) struct OpData(pub(crate) crate::RawOpBuffer);
