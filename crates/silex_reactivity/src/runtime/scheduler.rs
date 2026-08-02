//! Global flat scheduler and scope bitmask lifetime tracking.

use super::model::ScopeState;
use crate::internal::RawId;

use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

type ErasedScopeState = RefCell<ScopeState<'static>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ScopeId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TargetNode {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observer {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScheduledTask {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
}

pub(crate) struct BitSet {
    bits: Vec<u64>,
}

impl BitSet {
    pub(crate) fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub(crate) fn is_set(&self, id: u32) -> bool {
        let index = (id / 64) as usize;
        if index >= self.bits.len() {
            return false;
        }
        (self.bits[index] & (1u64 << (id % 64))) != 0
    }

    pub(crate) fn set(&mut self, id: u32, value: bool) {
        let index = (id / 64) as usize;
        if index >= self.bits.len() {
            if !value {
                return;
            }
            self.bits.resize(index + 1, 0);
        }
        let bit = 1u64 << (id % 64);
        if value {
            self.bits[index] |= bit;
        } else {
            self.bits[index] &= !bit;
        }
    }
}

pub(crate) struct ScopeEntry {
    erased_state: Weak<ErasedScopeState>,
}

pub(crate) struct GlobalScheduler {
    active_mask: BitSet,
    scopes: Vec<Option<ScopeEntry>>,
    next_scope_id: u32,
    epoch: u64,
    observer: Option<Observer>,
    pub(crate) global_queue: VecDeque<ScheduledTask>,
    pub(crate) running_queue: bool,
    pub(crate) batch_depth: usize,
    pub(crate) evaluating: usize,
    pub(crate) executing: usize,
    pub(crate) borrowed_values: usize,
}

impl GlobalScheduler {
    pub(crate) fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            active_mask: BitSet::new(),
            scopes: Vec::new(),
            next_scope_id: 0,
            epoch: 1,
            observer: None,
            global_queue: VecDeque::new(),
            running_queue: false,
            batch_depth: 0,
            evaluating: 0,
            executing: 0,
            borrowed_values: 0,
        }))
    }

    pub(crate) fn current_epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.epoch
    }

    pub(crate) fn observer(&self) -> Option<Observer> {
        self.observer
    }

    pub(crate) fn set_observer(&mut self, observer: Option<Observer>) -> Option<Observer> {
        std::mem::replace(&mut self.observer, observer)
    }

    pub(crate) fn alloc_scope<'scope>(
        &mut self,
        state: &Rc<RefCell<ScopeState<'scope>>>,
    ) -> ScopeId {
        let id = self.next_scope_id;
        self.next_scope_id = self.next_scope_id.wrapping_add(1);
        let scope_id = ScopeId(id);

        let weak_state = Rc::downgrade(state);
        // SAFETY: `ErasedScopeState` 是 `RefCell<ScopeState<'static>>` 的类型擦除别名。
        // 在此处擦除 `'scope` 生命周期是 Sound 的，因为全局调度器仅保留 `Weak` 弱引用，
        // 且仅在 `get_scope` 中通过 `is_scope_active` 确认 Scope 在词法作用域内处于 Active 状态时，
        // 才会将弱引用 restore 为对应的目标生命周期 `ScopeState<'scope>`，绝不会导致生命周期悬空。
        let erased_state = unsafe {
            std::mem::transmute::<Weak<RefCell<ScopeState<'scope>>>, Weak<ErasedScopeState>>(
                weak_state,
            )
        };

        self.active_mask.set(id, true);
        let index = id as usize;
        if index >= self.scopes.len() {
            self.scopes.resize_with(index + 1, || None);
        }
        self.scopes[index] = Some(ScopeEntry { erased_state });
        scope_id
    }

    pub(crate) fn deactivate_scope(&mut self, id: ScopeId) {
        self.active_mask.set(id.0, false);
        let index = id.0 as usize;
        if index < self.scopes.len() {
            self.scopes[index] = None;
        }
    }

    pub(crate) fn is_scope_active(&self, id: ScopeId) -> bool {
        self.active_mask.is_set(id.0)
    }

    pub(crate) fn active_scope_ids(&self) -> Vec<ScopeId> {
        self.scopes
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|_| ScopeId(index as u32))
                    .filter(|id| self.is_scope_active(*id))
            })
            .collect()
    }

    pub(crate) fn get_scope<'scope>(&self, id: ScopeId) -> Option<Rc<RefCell<ScopeState<'scope>>>> {
        if !self.is_scope_active(id) {
            return None;
        }
        let entry = self.scopes.get(id.0 as usize)?.as_ref()?;
        // SAFETY: `erased_state` 在被 `transmute` 为当前要求的 `ScopeState<'scope>` 引用前，
        // 已通过 `is_scope_active` 验证该 ScopeId 在当下确实处于活跃生命周期内。
        // 此外，`Weak::upgrade()` 返回 `Option<Rc<RefCell<ScopeState<'scope>>>>`，
        // 借用者使用返回的 `Rc` 受调用方作用域约束，不会造成非法内存越界访问或生命周期悬空。
        unsafe {
            let weak = std::mem::transmute::<
                &Weak<ErasedScopeState>,
                &Weak<RefCell<ScopeState<'scope>>>,
            >(&entry.erased_state);
            weak.upgrade()
        }
    }

    pub(crate) fn enqueue_effect(&mut self, task: ScheduledTask) {
        if self.is_scope_active(task.scope_id) {
            self.global_queue.push_back(task);
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.batch_depth == 0 && self.evaluating == 0 && self.executing == 0 && !self.running_queue
    }

    pub(crate) fn should_flush(&self) -> bool {
        self.is_idle() && self.borrowed_values == 0 && !self.global_queue.is_empty()
    }
}
