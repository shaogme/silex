use std::any::Any;

pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::scheduler::*;
use self::storage::*;
use crate::DependencyList;
use crate::core::algorithm::{self, GraphExecutor, NodeState, RuntimeAdapter as AbstractAdapter};
use crate::core::arena::Index as NodeId;
use crate::core::value::AnyValue;

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

    #[track_caller]
    pub fn create_signal<T: 'static>(&self, value: T) -> NodeId {
        self.create_signal_untyped(AnyValue::new(value))
    }

    pub(crate) fn create_signal_untyped(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage.signals.insert(
            id,
            SignalData {
                value,
                subscribers: crate::NodeList::Empty,
                last_tracked_by: None,
                version: 0,
            },
        );
        id
    }

    #[track_caller]
    pub fn create_effect<F: Fn() + 'static>(&self, f: F) -> NodeId {
        let id = self.register_node();
        self.storage.effects.insert(
            id,
            EffectData {
                computation: Some(Box::new(move |_| f())),
                dependencies: DependencyList::default(),
                effect_version: 0,
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
            let (owner_version, is_owner_valid) =
                if let Some(eff) = self.storage.effects.get_mut(owner) {
                    (eff.effect_version, true)
                } else {
                    (0, false)
                };
            if !is_owner_valid {
                return;
            }
            let mut registered = false;
            let mut target_version = 0;
            if let Some(signal_data) = self.storage.signals.get_mut(target_id) {
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
                if let Some(eff) = self.storage.effects.get_mut(owner) {
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
            let (owner_version, is_owner_valid) =
                if let Some(eff) = self.storage.effects.get_mut(owner) {
                    (eff.effect_version, true)
                } else {
                    (0, false)
                };
            if !is_owner_valid {
                return;
            }
            if let Some(eff) = self.storage.effects.get_mut(owner) {
                let dependencies = &mut eff.dependencies;
                for &target_id in target_ids {
                    if owner == target_id {
                        continue;
                    }
                    if let Some(signal_data) = self.storage.signals.get_mut(target_id) {
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

    pub(crate) fn update_signal_untyped(&self, id: NodeId, updater: &mut dyn FnMut(&mut AnyValue)) {
        if let Some(signal) = self.storage.signals.get_mut(id) {
            signal.version = signal.version.wrapping_add(1);
            updater(&mut signal.value);
            self.notify_update(id);
        }
    }

    fn prepare_memo_node(&self, id: NodeId) {
        // Signal Component
        self.storage.signals.insert(
            id,
            SignalData {
                value: crate::core::value::AnyValue::new(()), // Temporary dummy
                subscribers: crate::NodeList::Empty,
                last_tracked_by: None,
                version: 0,
            },
        );

        // Effect Component
        self.storage.effects.insert(
            id,
            EffectData {
                computation: None,
                dependencies: DependencyList::default(),
                effect_version: 0,
            },
        );

        // State Component
        self.storage.states.insert(id, NodeState::Clean);
    }

    pub(crate) fn commit_update(&self, id: NodeId, value: AnyValue, changed: bool) {
        if changed {
            if let Some(signal) = self.storage.signals.get_mut(id) {
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
                    if self.storage.effects.contains_key(id) {
                        if self.storage.signals.contains_key(id) {
                            self.update_if_necessary(id);
                        } else {
                            self.run_effect(id);
                        }
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
        self.storage.closures.insert(id, ClosureData { f });
        id
    }

    pub fn create_op(&self, data: crate::RawOpBuffer) -> NodeId {
        let id = self.register_node();
        self.storage.ops.insert(id, OpData(data));
        id
    }

    pub fn create_memo<T, F>(&self, f: F) -> NodeId
    where
        T: Clone + PartialEq + 'static,
        F: Fn(Option<&T>) -> T + 'static,
    {
        self.create_memo_untyped::<T>(Box::new(f))
    }

    fn create_memo_untyped<T>(&self, f: Box<dyn Fn(Option<&T>) -> T + 'static>) -> NodeId
    where
        T: Clone + PartialEq + 'static,
    {
        let id = self.register_node();
        self.prepare_memo_node(id);

        let initial_value = self.untrack(|| f(None));
        if let Some(signal) = self.storage.signals.get_mut(id) {
            signal.value = AnyValue::new_reactive(initial_value);
        }

        let runner = move |old_any: Option<AnyValue>| -> AnyValue {
            let old_t = old_any.and_then(|any| {
                let r: &T = any.downcast_ref::<T>()?;
                Some(r.clone())
            });
            let new_t = f(old_t.as_ref());
            AnyValue::new_reactive(new_t)
        };

        self.register_memo_computation(
            id,
            Box::new(UniversalMemoRunner {
                f: Box::new(runner),
            }),
        );
        id
    }

    pub fn register_derived<T: 'static>(&self, f: Box<dyn Fn() -> T>) -> NodeId {
        self.create_derived_untyped(f)
    }

    fn create_derived_untyped<T: 'static>(&self, f: Box<dyn Fn() -> T>) -> NodeId {
        let id = self.register_node();
        self.prepare_memo_node(id);

        let initial_value = self.untrack(|| f());
        if let Some(signal) = self.storage.signals.get_mut(id) {
            signal.value = AnyValue::new(initial_value);
        }

        let runner = move || -> AnyValue { AnyValue::new(f()) };

        self.register_memo_computation(
            id,
            Box::new(UniversalDerivedRunner {
                f: Box::new(runner),
            }),
        );
        id
    }

    fn register_memo_computation(&self, id: NodeId, runner: Box<dyn MemoRunnerTrait>) {
        let computation = move |rt: &Runtime| {
            runner.run(rt, id);
        };
        if let Some(effect) = self.storage.effects.get_mut(id) {
            effect.computation = Some(Box::new(computation));
        }
    }

    pub fn store_value<T: 'static>(&self, value: T) -> NodeId {
        self.store_value_untyped(AnyValue::new(value))
    }

    pub(crate) fn store_value_untyped(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage
            .stored_values
            .insert(id, StoredValueData { value });
        id
    }

    pub fn register_callback<F>(&self, f: F) -> NodeId
    where
        F: Fn(Box<dyn std::any::Any>) + 'static,
    {
        self.register_callback_untyped(std::rc::Rc::new(f))
    }

    pub(crate) fn register_callback_untyped(
        &self,
        f: std::rc::Rc<dyn Fn(Box<dyn std::any::Any>)>,
    ) -> NodeId {
        let id = self.register_node();
        self.storage.callbacks.insert(id, CallbackData { f });
        id
    }

    pub fn register_node_ref(&self) -> NodeId {
        let id = self.register_node();
        self.storage
            .node_refs
            .insert(id, NodeRefData { element: None });
        id
    }

    pub fn provide_context<T: 'static>(&self, value: T) {
        let key = std::any::TypeId::of::<T>();
        if let Some(owner) = self.current_owner() {
            if let Some(aux) = self.storage.try_aux_mut(owner) {
                if aux.context.is_none() {
                    aux.context = Some(std::collections::HashMap::new());
                }
                if let Some(ctx) = &mut aux.context {
                    ctx.insert(key, Box::new(value));
                }
            }
        }
    }

    pub fn use_context<T: Clone + 'static>(&self) -> Option<T> {
        let mut current_opt = self.current_owner();
        while let Some(current) = current_opt {
            if let Some(aux) = self.storage.node_aux.get(current)
                && let Some(ctx) = &aux.context
                && let Some(val) = ctx.get(&std::any::TypeId::of::<T>())
            {
                return val.downcast_ref::<T>().cloned();
            }
            current_opt = self.storage.graph.get(current).and_then(|n| n.parent);
        }
        None
    }

    pub(crate) unsafe fn get_any_raw_ptr_untracked(&self, id: NodeId) -> Option<*const ()> {
        if let Some(s) = self.storage.signals.get(id) {
            return Some(unsafe { s.value.as_ptr() });
        }
        if let Some(sv) = self.storage.stored_values.get(id) {
            return Some(unsafe { sv.value.as_ptr() });
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
            if let Some(effect_data) = self.storage.effects.get_mut(effect_id) {
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
            f(self);
            self.set_owner(prev_owner);

            if let Some(effect_data) = self.storage.effects.get_mut(effect_id) {
                effect_data.computation = Some(f);
            }
        }
    }
}

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
            if let Some(data) = self.storage.effects.get_mut(id) {
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
            f(self);
            self.set_owner(prev_owner);

            if let Some(data) = self.storage.effects.get_mut(id) {
                data.computation = Some(f);
            }
            if let Some(state) = self.storage.states.get_mut(id) {
                *state = NodeState::Clean;
            }
            return true;
        }
        false
    }
}

pub(crate) trait MemoRunnerTrait {
    fn run(&self, rt: &Runtime, id: NodeId);
}

struct UniversalMemoRunner {
    f: Box<dyn Fn(Option<AnyValue>) -> AnyValue>,
}

impl MemoRunnerTrait for UniversalMemoRunner {
    fn run(&self, rt: &Runtime, id: NodeId) {
        let old_any = rt.storage.signals.get(id).and_then(|s| s.value.try_clone());

        let new_any = {
            let prev_owner = rt.current_owner();
            rt.set_owner(Some(id));
            let v = (self.f)(old_any.as_ref().and_then(|any| any.try_clone()));
            rt.set_owner(prev_owner);
            v
        };

        let changed = match &old_any {
            Some(old) => !new_any.try_eq(old),
            None => true,
        };

        rt.commit_update(id, new_any, changed);
    }
}

struct UniversalDerivedRunner {
    f: Box<dyn Fn() -> AnyValue>,
}

impl MemoRunnerTrait for UniversalDerivedRunner {
    fn run(&self, rt: &Runtime, id: NodeId) {
        let new_any = {
            let prev_owner = rt.current_owner();
            rt.set_owner(Some(id));
            let v = (self.f)();
            rt.set_owner(prev_owner);
            v
        };
        rt.commit_update(id, new_any, true);
    }
}
