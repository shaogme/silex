use std::{any::Any, mem, rc::Rc};

pub(crate) mod guard;
pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::{
    guard::{DepthGuard, NodeRunGuard, OwnerGuard, QueueGuard, SignalValueGuard},
    scheduler::*,
    scope::Scopes,
    storage::*,
};
use crate::{
    DependencyList, NodeList, RawOpBuffer,
    core::{
        FuncPtr,
        algorithm::{self, GraphExecutor, NodeState, RuntimeAdapter as AbstractAdapter},
        arena::Index as NodeId,
        value::{AnyValue, ThunkVTable, ThunkValue},
    },
};
use silex_vtable::InlineStorage;

pub struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) scheduler: Scheduler,
    pub(crate) scopes: Scopes,
}

pub(crate) static RUNTIME: silex_thread_local::ThreadLocal<Runtime> =
    silex_thread_local::ThreadLocal::new();

impl Runtime {
    pub(crate) fn new() -> Self {
        Self {
            storage: Storage::new(),
            scheduler: Scheduler::new(),
            scopes: Scopes::new(),
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
                    subscribers: NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                    updating: false,
                }),
                effect: None,
            },
        );
        id
    }

    pub(crate) fn create_effect(&self, f: ThunkValue) -> NodeId {
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
                    running: false,
                }),
            },
        );

        // 首跑必须与重跑走同一条调度路径：先占住 `running_queue`，
        // 让 effect 体内的写入只入队、不在体内嵌套 flush，运行结束后统一 flush。
        // 否则同一段用户代码的执行顺序会取决于它是首跑还是重跑（AUDIT P15），
        // 而嵌套 flush 更会重入到这个正在运行的 effect 上（AUDIT P1）。
        let is_outermost = {
            let queue_guard = QueueGuard::acquire(&self.scheduler.running_queue);
            let is_outermost = queue_guard.is_some();
            self.run_node(id, true);
            is_outermost
        };

        if is_outermost {
            self.flush_if_idle();
        }
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
            if registered
                && let Some(owner_node) = self.storage.reactive.get_mut(owner)
                && let Some(eff) = &mut owner_node.effect
            {
                eff.dependencies.push((target_id, target_version));
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
        let was_outermost = {
            // DFS 期间禁止 flush effect 队列（AUDIT P15）。
            let eval_guard = DepthGuard::enter(&self.scheduler.evaluating);

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

            eval_guard.is_outermost()
        };

        // DFS 期间被推迟的更新在这里统一 flush。
        // 若本次求值本身就发生在队列执行中，`run_queue` 的守卫会让这次调用直接返回，
        // 由外层的队列循环继续消费。
        if was_outermost {
            self.flush_if_idle();
        }
    }

    pub(crate) fn notify_update(&self, id: NodeId) {
        self.queue_dependents(id);
        self.flush_if_idle();
    }

    /// 在没有 batch、也没有正在进行的求值 DFS 时执行 effect 队列。
    ///
    /// 这是 effect 的**唯一**调度出口：所有会产生失效的路径都汇聚到这里，
    /// 执行时机不再取决于调用方走的是哪条入口（AUDIT P15）。
    #[inline]
    pub(crate) fn flush_if_idle(&self) {
        if self.scheduler.batch_depth.get() == 0 && self.scheduler.evaluating.get() == 0 {
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

    /// 以“取出 → 交给用户闭包 → 放回”的方式修改 signal 的值。
    ///
    /// 用户闭包执行期间，节点里放的是一个占位值，运行时不再持有任何指向该节点的
    /// `&mut` —— 否则闭包内一旦重入访问同一个节点（哪怕只是读一下），就会构造出
    /// 与之重叠的引用，这是实打实的 UB（AUDIT P5）。
    ///
    /// 代价是一条明确的契约：**不允许在 update 闭包内访问同一个 signal**。
    /// debug 构建下会断言失败，release 下该次访问返回 `None`。
    pub(crate) fn with_signal_value_mut<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&mut AnyValue) -> R,
    ) -> Option<R> {
        let taken = {
            let node = self.storage.reactive.get_mut(id)?;
            let signal = node.signal.as_mut()?;
            if signal.updating {
                debug_assert!(
                    !signal.updating,
                    "在 update 闭包内重入访问同一个 signal 是不被支持的"
                );
                return None;
            }
            signal.updating = true;
            signal.version = signal.version.wrapping_add(1);
            mem::replace(&mut signal.value, AnyValue::placeholder())
            // 借用在此结束 —— 用户闭包在借用作用域之外执行。
        };

        // 守卫保证值一定会被放回（panic 展开时也一样）。
        let mut borrowed = SignalValueGuard::new(self, id, taken);
        Some(f(borrowed.value_mut()))
    }

    #[inline(never)]
    pub(crate) fn update_signal_untyped(&self, id: NodeId, updater: &mut dyn FnMut(&mut AnyValue)) {
        if self
            .with_signal_value_mut(id, |value| updater(value))
            .is_some()
        {
            // 通知必须发生在借用作用域之外：`notify_update` 会同步执行下游 effect，
            // 那些 effect 会重新访问本节点（AUDIT P5）。
            self.notify_update(id);
        }
    }

    pub(crate) fn get_signal_value(&self, id: NodeId) -> Option<&AnyValue> {
        self.prepare_read(id);
        self.storage
            .reactive
            .get(id)?
            .signal
            .as_ref()
            .map(|s| &s.value)
    }

    pub(crate) fn get_signal_value_untracked(&self, id: NodeId) -> Option<&AnyValue> {
        self.prepare_read_untracked(id);
        self.storage
            .reactive
            .get(id)?
            .signal
            .as_ref()
            .map(|s| &s.value)
    }

    pub(crate) fn prepare_memo_node(&self, id: NodeId, computation: ThunkValue) {
        self.storage.reactive.insert(
            id,
            ReactiveNode {
                state: NodeState::Dirty,
                signal: Some(SignalData {
                    value: AnyValue::placeholder(), // 首次计算前的临时占位值
                    subscribers: NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                    updating: false,
                }),
                effect: Some(EffectData {
                    computation: Some(computation),
                    dependencies: DependencyList::default(),
                    effect_version: 0,
                    running: false,
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
        // 守卫保证标志一定会被恢复：裸写法在 effect panic 时会让 `running_queue`
        // 永久卡在 true，此后 `run_queue` 每次入口直接返回，整个响应式系统静默停摆
        // （AUDIT P2）。`acquire` 返回 None 表示外层已经在跑队列。
        let Some(_queue_guard) = QueueGuard::acquire(&self.scheduler.running_queue) else {
            return;
        };

        loop {
            let next_to_run = self.scheduler.observer_queue.borrow_mut().pop_front();
            let Some(id) = next_to_run else { break };

            self.scheduler.queued_observers.remove(id);
            if self
                .storage
                .reactive
                .get(id)
                .is_some_and(|n| n.effect.is_some())
            {
                self.update_if_necessary(id);
            }
        }
    }

    #[track_caller]
    pub fn create_closure(&self, f: Box<dyn Any>) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::Closure(ClosureData { f }));
        id
    }

    pub fn create_op(&self, data: RawOpBuffer) -> NodeId {
        let id = self.register_node();
        self.storage.extras.insert(id, ExtraData::Op(OpData(data)));
        id
    }

    /// 用给定的载荷初始化一个 memo / derived 节点，并立即完成首次计算。
    ///
    /// 计算闭包先被装进节点，再由统一的 [`Runtime::run_node`] 驱动首跑：
    /// 这样首跑与后续重算走同一条路径，也不存在“闭包尚未被 `ThunkValue` 接管
    /// 就提前返回”导致析构函数永不运行的窗口（AUDIT P19.10）。
    ///
    /// # Safety
    ///
    /// `data` 的内容必须是一个合法的 memo 载荷：偏移 0 处是 `*const MemoVTable`，
    /// 其后是该 vtable 所约定的闭包表示。
    #[inline(never)]
    pub(crate) unsafe fn initialize_memo(&self, id: NodeId, data: InlineStorage) {
        // SAFETY: 由调用方保证载荷布局与 `UNIVERSAL_MEMO_THUNK_VTABLE` 一致。
        let thunk = unsafe { ThunkValue::new_raw(data, &UNIVERSAL_MEMO_THUNK_VTABLE) };
        self.prepare_memo_node(id, thunk);
        self.run_node(id, true);
    }

    pub fn store_value(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::StoredValue(StoredValueData { value }));
        id
    }

    pub(crate) fn get_stored_value(&self, id: NodeId) -> Option<&AnyValue> {
        let extra = self.storage.extras.get(id)?;
        if let ExtraData::StoredValue(sv) = extra {
            Some(&sv.value)
        } else {
            None
        }
    }

    pub(crate) fn get_stored_value_mut(&self, id: NodeId) -> Option<&mut AnyValue> {
        let extra = self.storage.extras.get_mut(id)?;
        if let ExtraData::StoredValue(sv) = extra {
            Some(&mut sv.value)
        } else {
            None
        }
    }

    pub fn register_callback_untyped(&self, f: Rc<dyn Fn(Box<dyn Any>)>) -> NodeId {
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

    pub(crate) unsafe fn get_any_raw_ptr_untracked(&self, id: NodeId) -> Option<*const ()> {
        if let Some(n) = self.storage.reactive.get(id)
            && let Some(s) = &n.signal
        {
            return Some(unsafe { s.value.as_ptr() });
        }
        if let Some(extra) = self.storage.extras.get(id)
            && let ExtraData::StoredValue(sv) = extra
        {
            return Some(unsafe { sv.value.as_ptr() });
        }
        None
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        // 守卫保证深度一定会被恢复：裸写法在 f panic 时会让 `batch_depth` 卡在非零，
        // 此后所有更新被永久挂起（AUDIT P2）。
        let result = {
            let _batch_guard = DepthGuard::enter(&self.scheduler.batch_depth);
            f()
        };

        self.flush_if_idle();
        result
    }

    /// 运行一个计算节点（effect 或 memo）的计算闭包。
    ///
    /// 这是**唯一**执行用户计算的入口 —— effect 首跑、effect 重跑、memo 首算、
    /// memo 重算全部走这里。之前 `run_effect` 与 `run_computation` 是同一段逻辑的
    /// 两份拷贝，各自演化出不同的状态转换，正是 P1 / P8 得以存在的土壤（AUDIT P16）。
    ///
    /// 返回值表示是否真的执行了计算闭包。以下情况返回 `false`：
    /// 节点不存在、不是计算节点、或**正在运行中**。
    pub(crate) fn run_node(&self, id: NodeId, set_clean: bool) -> bool {
        // 阶段一：加重入锁并借出计算闭包。
        // 重入检查必须发生在任何破坏性操作之前 —— 一旦第二次执行前置阶段，
        // 节点的依赖列表会被清空、订阅关系被摘除，而重建订阅的那一步却因为
        // `computation` 已被借出而被跳过，该节点从此永久失联（AUDIT P1）。
        let (computation, dependencies) = {
            let Some(node) = self.storage.reactive.get_mut(id) else {
                return false;
            };
            let Some(effect) = node.effect.as_mut() else {
                return false;
            };
            if effect.running {
                return false;
            }
            effect.running = true;
            effect.effect_version = effect.effect_version.wrapping_add(1);
            (
                effect.computation.take(),
                mem::take(&mut effect.dependencies),
            )
        };

        // 从这里开始，闭包的归还与重入锁的释放由守卫接管（panic 展开时同样生效）。
        let run_guard = NodeRunGuard::new(self, id, computation);

        // 阶段二：清理上一次运行留下的子节点、cleanup 与订阅关系。
        let (children, cleanups) = match self.storage.node_aux.get_mut(id) {
            Some(aux) => (mem::take(&mut aux.children), mem::take(&mut aux.cleanups)),
            None => (Vec::new(), CleanupList::default()),
        };
        self.run_cleanups(id, children, cleanups, dependencies);

        let Some(f) = run_guard.computation.as_ref() else {
            return false;
        };

        // 阶段三：状态在调用用户闭包**之前**置 Clean。
        // 运行期间产生的失效标记（例如 effect 写了自己的依赖）因此得以保留，
        // 不会被“运行完再无条件置 Clean”抹掉（AUDIT P8）。
        if set_clean && let Some(node) = self.storage.reactive.get_mut(id) {
            node.state = NodeState::Clean;
        }

        let _owner = OwnerGuard::set(self, Some(id));
        // SAFETY: 传给 thunk 的指针就是当前运行时本身，其生命周期覆盖整个调用。
        unsafe { f.call(self as *const Runtime as *const ()) };
        true
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
            let _owner = OwnerGuard::set(self, Some(id));
            compute_any(old_any.as_ref().and_then(|any| any.try_clone()))
        };

        let changed = match &old_any {
            Some(old) => !new_any.try_eq(old),
            None => true,
        };
        self.commit_update(id, new_any, changed);
    }

    /// memo 载荷的统一入口。
    ///
    /// vtable 指针以**指针**形式从缓冲区读回（而不是先读成 `usize` 再转回指针），
    /// 否则 provenance 会被擦除（AUDIT P3）。
    pub(crate) unsafe fn universal_memo_runner(ptr: *const u8, rt_ptr: *const ()) {
        let rt = unsafe { &*(rt_ptr as *const Runtime) };
        let id = rt
            .current_owner()
            .expect("memo runner must be invoked with the memo node as the current owner");
        let vtable = unsafe { &*(*(ptr as *const *const MemoVTable)) };
        let data_ptr = unsafe { ptr.add(MEMO_PAYLOAD_OFFSET) };

        rt.update_memo_core(id, &mut |old| unsafe {
            (vtable.compute.as_fn())(data_ptr, old)
        });
    }

    pub(crate) unsafe fn universal_memo_drop(ptr: *mut u8) {
        let vtable = unsafe { &*(*(ptr as *const *const MemoVTable)) };
        let data_ptr = unsafe { ptr.add(MEMO_PAYLOAD_OFFSET) };
        unsafe { (vtable.drop.as_fn())(data_ptr) };
    }
}

/// memo 内联载荷中闭包相对于缓冲区起始处的偏移（前面是 `*const MemoVTable`）。
pub(crate) const MEMO_PAYLOAD_OFFSET: usize = mem::size_of::<usize>();

pub(crate) struct MemoVTable {
    pub(crate) compute: FuncPtr<unsafe fn(*const u8, Option<AnyValue>) -> AnyValue>,
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
}

pub(crate) static UNIVERSAL_MEMO_THUNK_VTABLE: ThunkVTable = ThunkVTable {
    drop: FuncPtr::new(Runtime::universal_memo_drop),
    call: FuncPtr::new(Runtime::universal_memo_runner),
};

impl GraphExecutor for Runtime {
    fn run_computation(&self, id: NodeId) -> bool {
        self.run_node(id, true)
    }
}
