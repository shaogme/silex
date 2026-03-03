use crate::DependencyList;
use crate::core::arena::Index as NodeId;
use crate::runtime::Runtime;
use crate::runtime::storage::{CleanupList, Node};
use std::cell::Cell;

pub(crate) struct Scopes {
    pub(crate) current_owner: Cell<Option<NodeId>>,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self {
            current_owner: Cell::new(None),
        }
    }
}

impl Runtime {
    pub(crate) fn current_owner(&self) -> Option<NodeId> {
        self.scopes.current_owner.get()
    }

    pub(crate) fn set_owner(&self, owner: Option<NodeId>) {
        self.scopes.current_owner.set(owner);
    }

    pub fn untrack<T>(&self, f: impl FnOnce() -> T) -> T {
        let prev_owner = self.current_owner();
        self.set_owner(None);
        let t = f();
        self.set_owner(prev_owner);
        t
    }

    pub fn create_scope<F>(&self, f: F) -> NodeId
    where
        F: FnOnce(),
    {
        let id = self.register_node();
        let prev_owner = self.current_owner();
        self.set_owner(Some(id));
        f();
        self.set_owner(prev_owner);
        id
    }

    pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        if let Some(owner) = self.current_owner() {
            if let Some(aux) = self.storage.try_aux_mut(owner) {
                aux.cleanups.push(Box::new(f));
            }
        }
    }

    pub fn dispose(&self, id: NodeId) {
        self.dispose_node_internal(id, true);
    }

    #[track_caller]
    pub(crate) fn register_node(&self) -> NodeId {
        let parent = self.current_owner();
        let mut node = Node::new();
        node.parent = parent;

        #[cfg(debug_assertions)]
        {
            node.defined_at = Some(std::panic::Location::caller());
        }

        let id = self.storage.graph.insert(node);

        if let Some(parent_id) = parent {
            if let Some(aux) = self.storage.try_aux_mut(parent_id) {
                aux.children.push(id);
            }
        }
        id
    }

    pub(crate) fn clean_node(&self, id: NodeId) {
        if self.storage.graph.get(id).is_none() {
            return;
        }
        let (children, cleanups) = {
            if let Some(aux) = self.storage.node_aux.get_mut(id) {
                (
                    std::mem::take(&mut aux.children),
                    std::mem::take(&mut aux.cleanups),
                )
            } else {
                (Vec::new(), CleanupList::default())
            }
        };

        let dependencies = {
            if let Some(n) = self.storage.reactive.get_mut(id)
                && let Some(effect_data) = &mut n.effect
            {
                let mut deps = DependencyList::default();
                std::mem::swap(&mut effect_data.dependencies, &mut deps);
                deps
            } else {
                DependencyList::default()
            }
        };

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    pub(crate) fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: CleanupList,
        dependencies: DependencyList,
    ) {
        for cleanup in cleanups {
            cleanup();
        }
        for child in children {
            self.dispose_node_internal(child, false);
        }
        for (dep_id, _) in dependencies {
            if let Some(n) = self.storage.reactive.get_mut(dep_id)
                && let Some(signal_data) = &mut n.signal
            {
                signal_data.subscribers.remove(&self_id);
            }
        }
    }

    pub(crate) fn dispose_node_internal(&self, id: NodeId, remove_from_parent: bool) {
        if self.storage.graph.get(id).is_none() {
            return;
        }
        self.clean_node(id);

        #[cfg(debug_assertions)]
        {
            if let Some(aux) = self.storage.node_aux.get_mut(id)
                && let Some(label) = aux.debug_label.take()
            {
                self.storage.dead_node_labels.insert(id, label);
            }
        }

        if remove_from_parent
            && let Some(parent_id) = self.storage.graph.get(id).and_then(|n| n.parent)
            && let Some(parent_aux) = self.storage.node_aux.get_mut(parent_id)
            && let Some(idx) = parent_aux.children.iter().position(|&x| x == id)
        {
            parent_aux.children.swap_remove(idx);
        }

        self.storage.graph.remove(id);
        self.storage.node_aux.remove(id);
        self.storage.reactive.remove(id);
        self.storage.extras.remove(id);
        self.scheduler.queued_observers.remove(id);
    }
}
