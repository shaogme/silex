use crate::core::arena::{Index as NodeId, SparseSecondaryMap};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

pub(crate) struct Scheduler {
    pub(crate) workspace: RefCell<WorkSpace>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: SparseSecondaryMap<()>,
    pub(crate) running_queue: Cell<bool>,
    pub(crate) batch_depth: Cell<usize>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            workspace: RefCell::new(WorkSpace::new()),
            observer_queue: RefCell::new(VecDeque::new()),
            queued_observers: SparseSecondaryMap::new(),
            running_queue: Cell::new(false),
            batch_depth: Cell::new(0),
        }
    }
}

impl crate::core::algorithm::GraphScheduler for Scheduler {
    fn queue_effect(&self, id: NodeId) {
        if self.queued_observers.get(id).is_none() {
            self.queued_observers.insert(id, ());
            self.observer_queue.borrow_mut().push_back(id);
        }
    }
}

pub(crate) struct WorkSpace {
    pub(crate) vec_pool: Vec<Vec<NodeId>>,
    pub(crate) deque_pool: Vec<VecDeque<NodeId>>,
}

impl WorkSpace {
    pub(crate) fn new() -> Self {
        Self {
            vec_pool: Vec::new(),
            deque_pool: Vec::new(),
        }
    }

    pub(crate) fn borrow_vec(&mut self) -> Vec<NodeId> {
        self.vec_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_vec(&mut self, mut v: Vec<NodeId>) {
        v.clear();
        if self.vec_pool.len() < 32 {
            self.vec_pool.push(v);
        }
    }

    pub(crate) fn borrow_deque(&mut self) -> VecDeque<NodeId> {
        self.deque_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_deque(&mut self, mut d: VecDeque<NodeId>) {
        d.clear();
        if self.deque_pool.len() < 32 {
            self.deque_pool.push(d);
        }
    }
}
