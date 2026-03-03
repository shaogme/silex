use std::any::{Any, TypeId};

pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::scheduler::*;
use self::storage::*;
use crate::DependencyList;
use crate::core::algorithm::{self, GraphExecutor, NodeState, RuntimeAdapter as AbstractAdapter};
use crate::core::arena::Index as NodeId;
use crate::core::value::{AnyValue, ThunkValue};

pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) scheduler: Scheduler,
    pub(crate) scopes: self::scope::Scopes,
}

thread_local! {
    pub(crate) static RUNTIME: Runtime = Runtime::new();
}

impl Runtime {
    fn new() -> Self {
        Self {
            storage: Storage::new(),
            scheduler: Scheduler::new(),
            scopes: self::scope::Scopes::new(),
        }
    }

    pub fn create_signal(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage.reactive.insert(
            id,
            ReactiveNode {
                state: NodeState::Clean,
                signal: Some(SignalData {
                    value,
                    subscribers: crate::NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                }),
                effect: None,
            },
        );
        id
    }

    pub fn create_effect(&self, f: ThunkValue) -> NodeId {
        let id = self.register_node();
        self.storage.reactive.insert(
            id,
            ReactiveNode {
                state: NodeState::Clean,
                signal: None,
                effect: Some(EffectData {
                    computation: Some(f),
                    dependencies: DependencyList::default(),
                    effect_version: 0,
                }),
            },
        );
        self.run_effect(id);
        id
    }

    pub(crate) fn track_dependency(&self, target_id: NodeId) {
        if let Some(owner) = self.current_owner() {
            if owner == target_id {
                return;
            }
            if self.storage.graph.get(owner).is_none() {
                return;
            }
            let (owner_version, is_owner_valid) = if let Some(owner_node) =
                self.storage.reactive.get_mut(owner)
                && let Some(eff) = &owner_node.effect
            {
                (eff.effect_version, true)
            } else {
                (0, false)
            };
            if !is_owner_valid {
                return;
            }
            let mut registered = false;
            let mut target_version = 0;
            if let Some(target_node) = self.storage.reactive.get_mut(target_id)
                && let Some(signal_data) = &mut target_node.signal
            {
                if let Some((last_owner, last_version)) = signal_data.last_tracked_by
                    && last_owner == owner
                    && last_version == owner_version
                {
                    return;
                }
                signal_data.subscribers.push(owner);
                signal_data.last_tracked_by = Some((owner, owner_version));
                registered = true;
                target_version = signal_data.version;
            }
            if registered {
                if let Some(owner_node) = self.storage.reactive.get_mut(owner)
                    && let Some(eff) = &mut owner_node.effect
                {
                    eff.dependencies.push((target_id, target_version));
                }
            }
        }
    }

    pub(crate) fn track_dependencies(&self, target_ids: &[NodeId]) {
        if target_ids.is_empty() {
            return;
        }
        if let Some(owner) = self.current_owner() {
            if self.storage.graph.get(owner).is_none() {
                return;
            }
            let (owner_version, is_owner_valid) = if let Some(owner_node) =
                self.storage.reactive.get_mut(owner)
                && let Some(eff) = &owner_node.effect
            {
                (eff.effect_version, true)
            } else {
                (0, false)
            };
            if !is_owner_valid {
                return;
            }
            if let Some(owner_node) = self.storage.reactive.get_mut(owner)
                && let Some(eff) = &mut owner_node.effect
            {
                let dependencies = &mut eff.dependencies;
                for &target_id in target_ids {
                    if owner == target_id {
                        continue;
                    }
                    if let Some(target_node) = self.storage.reactive.get_mut(target_id)
                        && let Some(signal_data) = &mut target_node.signal
                    {
                        if let Some((last_owner, last_version)) = signal_data.last_tracked_by
                            && last_owner == owner
                            && last_version == owner_version
                        {
                            continue;
                        }
                        signal_data.subscribers.push(owner);
                        signal_data.last_tracked_by = Some((owner, owner_version));
                        dependencies.push((target_id, signal_data.version));
                    }
                }
            }
        }
    }

    pub(crate) fn queue_dependents(&self, source_id: NodeId) {
        let (mut queue, mut subs) = {
            let mut ws = self.scheduler.workspace.borrow_mut();
            (ws.borrow_deque(), ws.borrow_vec())
        };
        let mut adapter = AbstractAdapter {
            storage: &self.storage,
            scheduler: &self.scheduler,
            executor: self,
        };
        algorithm::propagate(&mut adapter, source_id, &mut queue, &mut subs);
        let mut ws = self.scheduler.workspace.borrow_mut();
        ws.return_deque(queue);
        ws.return_vec(subs);
    }

    pub(crate) fn update_if_necessary(&self, node_id: NodeId) {
        let (mut stack, mut deps) = {
            let mut ws = self.scheduler.workspace.borrow_mut();
            (ws.borrow_vec(), ws.borrow_vec())
        };
        let mut adapter = AbstractAdapter {
            storage: &self.storage,
            scheduler: &self.scheduler,
            executor: self,
        };
        algorithm::evaluate(&mut adapter, node_id, &mut stack, &mut deps);
        let mut ws = self.scheduler.workspace.borrow_mut();
        ws.return_vec(stack);
        ws.return_vec(deps);
    }

    pub(crate) fn notify_update(&self, id: NodeId) {
        self.queue_dependents(id);
        if self.scheduler.batch_depth.get() == 0 {
            self.run_queue();
        }
    }

    pub(crate) fn prepare_read(&self, id: NodeId) {
        self.track_dependency(id);
        self.update_if_necessary(id);
    }

    pub(crate) fn prepare_read_untracked(&self, id: NodeId) {
        self.update_if_necessary(id);
    }

    #[inline(never)]
    pub(crate) fn update_signal_untyped(&self, id: NodeId, updater: &mut dyn FnMut(&mut AnyValue)) {
        if let Some(n) = self.storage.reactive.get_mut(id)
            && let Some(signal) = &mut n.signal
        {
            signal.version = signal.version.wrapping_add(1);
            updater(&mut signal.value);
            self.notify_update(id);
        }
    }

    pub(crate) fn prepare_memo_node(&self, id: NodeId) {
        self.storage.reactive.insert(
            id,
            ReactiveNode {
                state: NodeState::Clean,
                signal: Some(SignalData {
                    value: crate::core::value::AnyValue::new(()), // Temporary dummy
                    subscribers: crate::NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                }),
                effect: Some(EffectData {
                    computation: None,
                    dependencies: DependencyList::default(),
                    effect_version: 0,
                }),
            },
        );
    }

    pub(crate) fn commit_update(&self, id: NodeId, value: AnyValue, changed: bool) {
        if changed {
            if let Some(n) = self.storage.reactive.get_mut(id)
                && let Some(signal) = &mut n.signal
            {
                signal.version = signal.version.wrapping_add(1);
                signal.value = value;
            }
            self.notify_update(id);
        }
    }

    pub(crate) fn run_queue(&self) {
        if self.scheduler.running_queue.get() {
            return;
        }
        self.scheduler.running_queue.set(true);

        loop {
            let next_to_run = self.scheduler.observer_queue.borrow_mut().pop_front();
            match next_to_run {
                Some(id) => {
                    self.scheduler.queued_observers.remove(id);
                    if let Some(n) = self.storage.reactive.get(id)
                        && n.effect.is_some()
                    {
                        self.update_if_necessary(id);
                    }
                }
                None => break,
            }
        }
        self.scheduler.running_queue.set(false);
    }

    #[track_caller]
    pub fn create_closure(&self, f: Box<dyn Any>) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::Closure(ClosureData { f }));
        id
    }

    pub fn create_op(&self, data: crate::RawOpBuffer) -> NodeId {
        let id = self.register_node();
        self.storage.extras.insert(id, ExtraData::Op(OpData(data)));
        id
    }

    #[inline(never)]
    pub(crate) fn run_with_owner<R>(&self, id: NodeId, f: impl FnOnce() -> R) -> R {
        let prev = self.current_owner();
        self.set_owner(Some(id));
        let result = f();
        self.set_owner(prev);
        result
    }

    #[inline(never)]
    pub(crate) unsafe fn initialize_memo_raw(&self, id: NodeId, data: [usize; 3]) {
        self.prepare_memo_node(id);

        let vtable_ptr = data[0] as *const MemoVTable;
        let vtable = unsafe { &*vtable_ptr };
        let data_ptr = unsafe { data.as_ptr().add(1) };

        let initial_value = self.run_with_owner(id, || unsafe {
            (vtable.compute.as_fn())(data_ptr as *const usize, None)
        });

        if let Some(n) = self.storage.reactive.get_mut(id) {
            if let Some(signal) = &mut n.signal {
                signal.value = initial_value;
            }
            if let Some(effect) = &mut n.effect {
                effect.computation = Some(ThunkValue::new_raw(data, &UNIVERSAL_MEMO_THUNK_VTABLE));
            }
        }
    }

    pub fn store_value(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::StoredValue(StoredValueData { value }));
        id
    }

    pub fn register_callback_untyped(
        &self,
        f: std::rc::Rc<dyn Fn(Box<dyn std::any::Any>)>,
    ) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::Callback(CallbackData { f }));
        id
    }

    pub fn register_node_ref(&self) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::NodeRef(NodeRefData { element: None }));
        id
    }

    pub fn provide_context(&self, key: TypeId, value: Box<dyn Any>) {
        if let Some(owner) = self.current_owner() {
            if let Some(aux) = self.storage.try_aux_mut(owner) {
                if aux.context.is_none() {
                    aux.context = Some(std::collections::HashMap::new());
                }
                if let Some(ctx) = &mut aux.context {
                    ctx.insert(key, value);
                }
            }
        }
    }

    pub fn use_context_raw(&self, key: TypeId) -> Option<&dyn Any> {
        let mut current_opt = self.current_owner();
        while let Some(current) = current_opt {
            if let Some(aux) = self.storage.node_aux.get(current)
                && let Some(ctx) = &aux.context
                && let Some(val) = ctx.get(&key)
            {
                return Some(val.as_ref());
            }
            current_opt = self.storage.graph.get(current).and_then(|n| n.parent);
        }
        None
    }

    pub(crate) unsafe fn get_any_raw_ptr_untracked(&self, id: NodeId) -> Option<*const ()> {
        if let Some(n) = self.storage.reactive.get(id)
            && let Some(s) = &n.signal
        {
            return Some(unsafe { s.value.as_ptr() });
        }
        if let Some(extra) = self.storage.extras.get(id) {
            if let ExtraData::StoredValue(sv) = extra {
                return Some(unsafe { sv.value.as_ptr() });
            }
        }
        None
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        let depth = self.scheduler.batch_depth.get();
        self.scheduler.batch_depth.set(depth + 1);

        let result = f();

        self.scheduler.batch_depth.set(depth);

        if depth == 0 && !self.scheduler.running_queue.get() {
            self.run_queue();
        }

        result
    }

    pub(crate) fn run_effect(&self, effect_id: NodeId) {
        let (children, cleanups) = {
            if let Some(aux) = self.storage.node_aux.get_mut(effect_id) {
                (
                    std::mem::take(&mut aux.children),
                    std::mem::take(&mut aux.cleanups),
                )
            } else {
                (Vec::new(), CleanupList::default())
            }
        };

        let (computation_fn, dependencies) = {
            if let Some(n) = self.storage.reactive.get_mut(effect_id)
                && let Some(effect_data) = &mut n.effect
            {
                effect_data.effect_version = effect_data.effect_version.wrapping_add(1);
                let mut deps = DependencyList::default();
                std::mem::swap(&mut effect_data.dependencies, &mut deps);
                (effect_data.computation.take(), deps)
            } else {
                return;
            }
        };

        self.run_cleanups(effect_id, children, cleanups, dependencies);

        if let Some(f) = computation_fn {
            let prev_owner = self.current_owner();
            self.set_owner(Some(effect_id));
            unsafe { f.call(self as *const Runtime as *const ()) };
            self.set_owner(prev_owner);

            if let Some(n) = self.storage.reactive.get_mut(effect_id) {
                if let Some(effect_data) = &mut n.effect {
                    effect_data.computation = Some(f);
                }
            }
        }
    }
}

impl Runtime {
    #[inline(never)]
    pub(crate) fn update_memo_core(
        &self,
        id: NodeId,
        compute_any: &mut dyn FnMut(Option<AnyValue>) -> AnyValue,
    ) {
        let old_any = self
            .storage
            .reactive
            .get(id)
            .and_then(|n| n.signal.as_ref())
            .and_then(|s| s.value.try_clone());
        let new_any = {
            let prev_owner = self.current_owner();
            self.set_owner(Some(id));
            let v = compute_any(old_any.as_ref().and_then(|any| any.try_clone()));
            self.set_owner(prev_owner);
            v
        };

        let changed = match &old_any {
            Some(old) => !new_any.try_eq(old),
            None => true,
        };
        self.commit_update(id, new_any, changed);
    }

    pub(crate) unsafe fn universal_memo_runner(ptr: *mut usize, rt_ptr: *const ()) {
        let rt = unsafe { &*(rt_ptr as *const Runtime) };
        let id = rt.current_owner().unwrap();
        let vtable_ptr = unsafe { *(ptr as *const *const MemoVTable) };
        let vtable = unsafe { &*vtable_ptr };
        let data_ptr = unsafe { ptr.add(1) };

        rt.update_memo_core(id, &mut |old| unsafe {
            (vtable.compute.as_fn())(data_ptr, old)
        });
    }

    pub(crate) unsafe fn universal_memo_drop(ptr: *mut usize) {
        let vtable_ptr = unsafe { *(ptr as *const *const MemoVTable) };
        let vtable = unsafe { &*vtable_ptr };
        let data_ptr = unsafe { ptr.add(1) };
        unsafe { (vtable.drop.as_fn())(data_ptr) };
    }
}

pub(crate) struct MemoVTable {
    pub(crate) compute: crate::core::FuncPtr<unsafe fn(*const usize, Option<AnyValue>) -> AnyValue>,
    pub(crate) drop: crate::core::FuncPtr<unsafe fn(*mut usize)>,
}

pub(crate) static UNIVERSAL_MEMO_THUNK_VTABLE: crate::core::value::ThunkVTable =
    crate::core::value::ThunkVTable {
        drop: crate::core::FuncPtr::new(Runtime::universal_memo_drop),
        call: crate::core::FuncPtr::new(Runtime::universal_memo_runner),
    };

impl GraphExecutor for Runtime {
    fn run_computation(&self, id: NodeId) -> bool {
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

        let (computation_fn, dependencies) = {
            if let Some(n) = self.storage.reactive.get_mut(id)
                && let Some(data) = &mut n.effect
            {
                data.effect_version = data.effect_version.wrapping_add(1);
                let mut deps = DependencyList::default();
                std::mem::swap(&mut data.dependencies, &mut deps);
                (data.computation.take(), deps)
            } else {
                return false;
            }
        };

        self.run_cleanups(id, children, cleanups, dependencies);

        if let Some(f) = computation_fn {
            let prev_owner = self.current_owner();
            self.set_owner(Some(id));
            unsafe { f.call(self as *const Runtime as *const ()) };
            self.set_owner(prev_owner);

            if let Some(n) = self.storage.reactive.get_mut(id) {
                if let Some(data) = &mut n.effect {
                    data.computation = Some(f);
                }
                n.state = NodeState::Clean;
            }
            return true;
        }
        false
    }
}
