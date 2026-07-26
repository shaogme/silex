use std::{any::Any, mem, rc::Rc};

pub(crate) mod guard;
pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::{
    guard::{
        ComputationGuard, DepthGuard, EvalBuffers, NodeRunGuard, PropagateBuffers, QueueGuard,
        SignalValueGuard,
    },
    scheduler::*,
    scope::Scopes,
    storage::*,
};
use crate::{
    DependencyList, NodeList, RawOpBuffer,
    core::{
        FuncPtr,
        algorithm::{
            self, GraphExecutor, GraphStorage, NodeState, RuntimeAdapter as AbstractAdapter,
        },
        arena::Index as NodeId,
        value::{AnyValue, ThunkVTable, ThunkValue},
    },
};
use silex_vtable::InlineStorage;

pub(crate) struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) scheduler: Scheduler,
    pub(crate) scopes: Scopes,
}

pub(crate) static RUNTIME: silex_thread_local::ThreadLocal<Runtime> =
    silex_thread_local::ThreadLocal::new();

/// [`Runtime::track_dependencies`] 一次攒多少条依赖边再写回 observer。
///
/// 只影响“observer 查表被摊薄多少倍”，不影响语义。取 16：`track_signals_batch`
/// 的真实调用点一次登记的 signal 数是个位数到十几个。
const TRACK_BATCH: usize = 16;

/// [`Runtime::with_signal_value_mut`] 借不到值时的原因。
///
/// 两种失败必须区分开：节点不存在是调用方常见且合法的情况（销毁之后继续写），
/// 而重入则是违反契约的编程错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalBorrowError {
    /// 节点不存在、已销毁，或根本不是一个 signal。
    Missing,
    /// 值正被外层的 update 闭包借出（不允许重入）。
    Reentrant,
}

impl Runtime {
    pub(crate) fn new() -> Self {
        Self {
            storage: Storage::new(),
            scheduler: Scheduler::new(),
            scopes: Scopes::new(),
        }
    }

    #[track_caller]
    pub(crate) fn create_signal(&self, value: AnyValue) -> NodeId {
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

    #[track_caller]
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
            self.run_node(id);
            is_outermost
        };

        if is_outermost {
            self.flush_if_idle();
        }
        id
    }

    /// 当前 observer 的运行代次，同时兼作“它确实是个还活着的计算节点”的判定。
    ///
    /// 返回 `None` 表示不该建立任何依赖：没有 observer（顶层读取 / `untrack`）、
    /// observer 已被销毁，或它不是计算节点。
    fn observer_version(&self, observer: NodeId) -> Option<u32> {
        self.storage.graph.get(observer)?;
        self.storage
            .reactive
            .get(observer)
            .and_then(|n| n.effect.as_ref())
            .map(|eff| eff.effect_version)
    }

    /// 把 `observer` 登记进 `target_id` 的订阅者表，返回该 target 当前的版本号。
    ///
    /// 返回 `None` 表示“不必写依赖边”：target 不是 signal，或本次运行已经登记过
    /// 它（`last_tracked_by` 这条按 signal 存的单条去重缓存）。
    ///
    /// 借用严格限制在本函数内：调用方**不得**在调用它时还持有指向 `reactive`
    /// 表里任何条目的引用。
    fn subscribe(&self, target_id: NodeId, observer: NodeId, observer_version: u32) -> Option<u32> {
        let target_node = self.storage.reactive.get_mut(target_id)?;
        let signal_data = target_node.signal.as_mut()?;
        if let Some((last_observer, last_version)) = signal_data.last_tracked_by
            && last_observer == observer
            && last_version == observer_version
        {
            return None;
        }
        signal_data.subscribers.push(observer);
        signal_data.last_tracked_by = Some((observer, observer_version));
        Some(signal_data.version)
    }

    /// 把若干条 `(target, version)` 依赖边一次性写进 observer 的依赖表。
    fn record_dependencies(&self, observer: NodeId, edges: &[(NodeId, u32)]) {
        if edges.is_empty() {
            return;
        }
        if let Some(observer_node) = self.storage.reactive.get_mut(observer)
            && let Some(eff) = &mut observer_node.effect
        {
            for &edge in edges {
                eff.dependencies.push(edge);
            }
        }
    }

    pub(crate) fn track_dependency(&self, target_id: NodeId) {
        let Some(observer) = self.current_observer() else {
            return;
        };
        if observer == target_id {
            return;
        }
        let Some(observer_version) = self.observer_version(observer) else {
            return;
        };
        if let Some(target_version) = self.subscribe(target_id, observer, observer_version) {
            self.record_dependencies(observer, &[(target_id, target_version)]);
        }
    }

    /// [`Runtime::track_dependency`] 的批量版本：observer 的查表被摊薄到
    /// 每 [`TRACK_BATCH`] 个 target 一次。
    ///
    /// 这里**不能**图省事，在整个循环期间一直握着 observer 节点的 `&mut`：
    /// 循环体内反复调用 `reactive.get_mut(target_id)`，只要某个 target 的
    /// arena 下标与 observer 相同（句柄被回收复用之后就会这样），那次 `get_mut`
    /// 就会在 Stacked Borrows 下把外层的 `&mut` 作废 —— 之后再往依赖表里 push
    /// 即为未定义行为。`tests/aliasing.rs` 里有对应的 Miri 探针。
    pub(crate) fn track_dependencies(&self, target_ids: &[NodeId]) {
        if target_ids.is_empty() {
            return;
        }
        let Some(observer) = self.current_observer() else {
            return;
        };
        let Some(observer_version) = self.observer_version(observer) else {
            return;
        };

        // 栈上的小缓冲，攒满一批再统一写回，全程不分配。
        let mut pending = [(observer, 0u32); TRACK_BATCH];
        let mut len = 0usize;
        for &target_id in target_ids {
            if observer == target_id {
                continue;
            }
            let Some(target_version) = self.subscribe(target_id, observer, observer_version) else {
                continue;
            };
            pending[len] = (target_id, target_version);
            len += 1;
            if len == TRACK_BATCH {
                self.record_dependencies(observer, &pending);
                len = 0;
            }
        }
        self.record_dependencies(observer, &pending[..len]);
    }

    pub(crate) fn queue_dependents(&self, source_id: NodeId) {
        let mut buffers = PropagateBuffers::acquire(&self.scheduler.workspace);
        let (queue, subs) = buffers.split();
        let mut adapter = AbstractAdapter {
            storage: &self.storage,
            scheduler: &self.scheduler,
            executor: self,
        };
        algorithm::propagate(&mut adapter, source_id, queue, subs);
    }

    /// 必要时把一个节点算干净。
    ///
    /// 绝大多数读取的目标是一个**永远干净的普通 signal**，而整套求值机制对它们
    /// 是纯粹的浪费：两次 `workspace.borrow_mut()` 借出工作栈、构造 adapter、
    /// 进 `evaluate` 看一眼状态就返回、再两次 `borrow_mut()` 还回去，外加一个
    /// 深度守卫和一次空队列的 `run_queue`。实测这条提前返回让无 owner 上下文的
    /// signal 读取从 20.5 ns 降到 11.6 ns（−43%），effect 内的追踪读取
    /// 从 24.6 ns 降到 17.4 ns（AUDIT 二轮 §1.3）。
    ///
    /// `observer_queue` 那半个条件是为了保住“读取也是一个 flush 出口”这条现有
    /// 语义：队列里还有待办时照旧走完整路径，让这次读取把它们冲掉。
    pub(crate) fn update_if_necessary(&self, node_id: NodeId) {
        if self.storage.get_state(node_id) == NodeState::Clean
            && self.scheduler.observer_queue.borrow().is_empty()
        {
            return;
        }

        let was_outermost = {
            // DFS 期间禁止 flush effect 队列（AUDIT P15）。
            let eval_guard = DepthGuard::enter(&self.scheduler.evaluating);

            {
                let mut buffers = EvalBuffers::acquire(&self.scheduler.workspace);
                let (stack, deps) = buffers.split();
                let mut adapter = AbstractAdapter {
                    storage: &self.storage,
                    scheduler: &self.scheduler,
                    executor: self,
                };
                // `evaluate` 在依赖成环时 panic；工作栈由守卫归还。
                algorithm::evaluate(&mut adapter, node_id, stack, deps);
            }

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

    /// 读取一个节点之前：先把它算干净，**然后**才建立依赖边。
    ///
    /// 顺序不能反过来。之前是先 `track_dependency` 再 `update_if_necessary`，
    /// 于是当一个 memo 在自己的计算过程中第一次读到一个正处于 `Dirty` 的上游时：
    ///
    /// 1. 它先把自己登记进上游的订阅者表；
    /// 2. 上游随即重算、值变了、`commit_update` → `propagate` 把订阅者标脏 ——
    ///    而本节点刚刚才登记进去，且此刻**正在运行**；
    /// 3. 本节点在运行前置的 `Clean` 被覆盖成 `Dirty`，跑完出栈时状态仍是 `Dirty`；
    /// 4. 下游读它时看到 `Dirty`，再算一遍。
    ///
    /// 实测每层恒定 2 倍（不随链长放大），用户的计算闭包被白跑一次。
    /// 先求值再追踪之后，上游提交时本节点还不是它的订阅者，标不到自己头上；
    /// 顺带把登记的版本号从“重算前”修正成“重算后”的正确值（AUDIT 二轮 §1.2）。
    ///
    /// 代价：依赖边晚一步建立，因此依赖环要多绕一轮才会被 `evaluate` 检测到 ——
    /// 仍然会被检测到，只是路径长一点。
    pub(crate) fn prepare_read(&self, id: NodeId) {
        self.update_if_necessary(id);
        self.track_dependency(id);
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
    /// debug 构建下会断言失败，release 下该次访问返回 [`SignalBorrowError::Reentrant`]。
    ///
    /// 版本号由 `f` 的第二个返回值决定：`true` 表示“值真的被改写了”，
    /// 此时版本号在**归还值的那一次查表里**顺带递增，写路径不必为它再查一次表
    /// （AUDIT P12 定下语义，AUDIT 二轮 §1.3 末段合并查表）。
    pub(crate) fn with_signal_value_mut<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&mut AnyValue) -> (R, bool),
    ) -> Result<R, SignalBorrowError> {
        let taken = {
            let Some(node) = self.storage.reactive.get_mut(id) else {
                return Err(SignalBorrowError::Missing);
            };
            let Some(signal) = node.signal.as_mut() else {
                return Err(SignalBorrowError::Missing);
            };
            if signal.updating {
                debug_assert!(
                    !signal.updating,
                    "在 update 闭包内重入访问同一个 signal 是不被支持的"
                );
                return Err(SignalBorrowError::Reentrant);
            }
            signal.updating = true;
            mem::replace(&mut signal.value, AnyValue::placeholder())
            // 借用在此结束 —— 用户闭包在借用作用域之外执行。
        };

        // 守卫保证值一定会被放回（panic 展开时也一样）。
        let mut borrowed = SignalValueGuard::new(self, id, taken);
        let (result, changed) = f(borrowed.value_mut());
        if changed {
            borrowed.bump_version_on_release();
        }
        Ok(result)
    }

    /// 写入 signal 并在写入真的发生时失效下游。
    ///
    /// `updater` 返回 `false` 表示它没有改动这个值（典型情况是类型不匹配）：
    /// 此时既不递增版本号也不通知下游 —— 之前的写法无条件递增并通知，
    /// 于是一次什么都没做的更新会静默地把全部下游重跑一遍（AUDIT P12）。
    #[inline(never)]
    pub(crate) fn update_signal_untyped(
        &self,
        id: NodeId,
        updater: &mut dyn FnMut(&mut AnyValue) -> bool,
    ) -> Result<bool, SignalBorrowError> {
        let applied = self.with_signal_value_mut(id, |value| {
            let applied = updater(value);
            // 第二个分量即“请递增版本号”，与“写入真的发生了”是同一件事。
            (applied, applied)
        })?;
        if applied {
            // 通知必须发生在借用作用域之外：`notify_update` 会同步执行下游 effect，
            // 那些 effect 会重新访问本节点（AUDIT P5）。
            self.notify_update(id);
        }
        Ok(applied)
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

    /// 执行 effect 队列直到清空。
    ///
    /// # Panics
    ///
    /// 单次执行超过 [`MAX_QUEUE_ITERATIONS`] 次迭代时 panic。互相写入对方依赖的
    /// 两个 effect 会让队列永远不空，之前既没有上限也没有诊断，表现就是浏览器
    /// 标签页直接冻死（AUDIT P13）。
    pub(crate) fn run_queue(&self) {
        // 守卫保证标志一定会被恢复：裸写法在 effect panic 时会让 `running_queue`
        // 永久卡在 true，此后 `run_queue` 每次入口直接返回，整个响应式系统静默停摆
        // （AUDIT P2）。`acquire` 返回 None 表示外层已经在跑队列。
        let Some(_queue_guard) = QueueGuard::acquire(&self.scheduler.running_queue) else {
            return;
        };

        let mut iterations = 0usize;
        loop {
            let next_to_run = self.scheduler.observer_queue.borrow_mut().pop_front();
            let Some(id) = next_to_run else { break };

            iterations += 1;
            if iterations > MAX_QUEUE_ITERATIONS {
                panic!(
                    "silex_reactivity: effect 队列执行超过 {MAX_QUEUE_ITERATIONS} 次仍未清空，\
                     大概率是若干 effect 在互相触发对方的依赖。最后一个被调度的是 {}。",
                    self.storage.describe(id)
                );
            }

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
    pub(crate) fn create_closure(&self, f: Box<dyn Any>) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::Closure(ClosureData { f }));
        id
    }

    #[track_caller]
    pub(crate) fn create_op(&self, data: RawOpBuffer) -> NodeId {
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
        self.run_node(id);
    }

    #[track_caller]
    pub(crate) fn store_value(&self, value: AnyValue) -> NodeId {
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

    #[track_caller]
    pub(crate) fn register_callback_untyped(&self, f: Rc<dyn Fn(Box<dyn Any>)>) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::Callback(CallbackData { f }));
        id
    }

    #[track_caller]
    pub(crate) fn register_node_ref(&self) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, ExtraData::NodeRef(NodeRefData { element: None }));
        id
    }

    /// 取出节点内部值的裸指针（signal 与 stored value 都支持）。
    ///
    /// # Safety
    ///
    /// 契约见 [`crate::try_get_any_raw_untracked`]：调用方负责类型正确，
    /// 并保证在使用期间不发生任何会让该地址失效的操作。
    pub(crate) unsafe fn get_any_raw_ptr_untracked(&self, id: NodeId) -> Option<*const ()> {
        if let Some(n) = self.storage.reactive.get(id)
            && let Some(s) = &n.signal
        {
            // SAFETY: 契约转嫁给调用方（见本函数的 `# Safety`）。
            return Some(unsafe { s.value.as_ptr() });
        }
        if let Some(extra) = self.storage.extras.get(id)
            && let ExtraData::StoredValue(sv) = extra
        {
            // SAFETY: 同上。
            return Some(unsafe { sv.value.as_ptr() });
        }
        None
    }

    pub(crate) fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
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
    pub(crate) fn run_node(&self, id: NodeId) -> bool {
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
        let (children, cleanups) = self.take_scope_state(id);
        self.run_cleanups(id, children, cleanups, dependencies);

        let Some(f) = run_guard.computation.as_ref() else {
            return false;
        };

        // 阶段三：状态在调用用户闭包**之前**置 Clean。
        // 运行期间产生的失效标记（例如 effect 写了自己的依赖）因此得以保留，
        // 不会被“运行完再无条件置 Clean”抹掉（AUDIT P8）。
        //
        // 这里曾经有一个 `set_clean: bool` 参数，三个调用点全都传 `true`
        // —— 一个只会让读者以为“还存在另一种状态转换”的死参数，已删除。
        if let Some(node) = self.storage.reactive.get_mut(id) {
            node.state = NodeState::Clean;
        }

        let _ctx = ComputationGuard::enter(self, id);
        // SAFETY: 传给 thunk 的指针就是当前运行时本身，其生命周期覆盖整个调用。
        unsafe { f.call(self as *const Runtime as *const ()) };
        true
    }
}

impl Runtime {
    /// 重算一个 memo：借出旧值 → 调用计算闭包 → 与旧值比较 → 提交。
    ///
    /// 旧值是**借**给计算闭包的，不是克隆给它的。之前这里为一次重算克隆旧值三次
    /// （节点里克隆一份、再克隆一份传给 vtable、vtable 里再 `cloned()` 一次），
    /// 而且闭包用不用 `old` 都要付这个代价 —— 对持有 `Vec` / `String` 的 memo
    /// 就是每次重算三次深拷贝（AUDIT P9）。
    ///
    /// 旧值在计算期间被**移出**节点（节点里放占位值），理由与 `update_signal`
    /// 相同：计算闭包是用户代码，运行时不能在它执行期间持有指向节点的引用
    /// （AUDIT P5）。因此“在 memo 的计算闭包里读它自己”读到的是占位值（`None`），
    /// 旧值只能从闭包参数拿 —— 这本来也是 `Fn(Option<&T>) -> T` 这个签名的用途。
    #[inline(never)]
    pub(crate) fn update_memo_core(
        &self,
        id: NodeId,
        compute_any: &mut dyn FnMut(Option<&AnyValue>) -> AnyValue,
    ) {
        let taken = match self
            .storage
            .reactive
            .get_mut(id)
            .and_then(|n| n.signal.as_mut())
        {
            Some(signal) if !signal.updating => {
                signal.updating = true;
                Some(mem::replace(&mut signal.value, AnyValue::placeholder()))
            }
            // 节点不存在，或它的值正被某个 update 闭包借出：没有可用的旧值，
            // 一律按“变了”处理，`commit_update` 自己会跳过不存在的节点。
            _ => None,
        };

        // 守卫保证旧值一定会被放回（计算闭包 panic 时也一样）。
        let borrowed = taken.map(|value| SignalValueGuard::new(self, id, value));

        let new_any = {
            let _ctx = ComputationGuard::enter(self, id);
            compute_any(borrowed.as_ref().and_then(SignalValueGuard::value))
        };

        // 比较也在旧值还被借出时进行：`try_eq` 会调用用户的 `PartialEq`，
        // 同样不该在运行时持有节点引用的情况下运行。
        let changed = match borrowed.as_ref().and_then(SignalValueGuard::value) {
            Some(old) => !new_any.try_eq(old),
            None => true,
        };

        // 旧值先回到节点（并清除借出标记），随后 `commit_update` 才可能覆盖它。
        drop(borrowed);
        self.commit_update(id, new_any, changed);
    }

    /// memo 载荷的统一入口。
    ///
    /// vtable 指针以**指针**形式从缓冲区读回（而不是先读成 `usize` 再转回指针），
    /// 否则 provenance 会被擦除（AUDIT P3）。
    /// # Safety
    ///
    /// `ptr` 必须指向一个合法的 memo 载荷（偏移 0 处是 `*const MemoVTable`），
    /// `rt_ptr` 必须是有效的 `*const Runtime`。两者都由 `run_node` 提供。
    pub(crate) unsafe fn universal_memo_runner(ptr: *const u8, rt_ptr: *const ()) {
        // SAFETY: `run_node` 传进来的就是当前运行时，其生命周期覆盖整个调用。
        let rt = unsafe { &*(rt_ptr as *const Runtime) };
        let id = rt
            .current_owner()
            .expect("memo runner must be invoked with the memo node as the current owner");
        // SAFETY: 载荷布局由 `build_memo_payload` / `internal_init_derived` 保证：
        // 偏移 0 是一个真正的 `*const MemoVTable`（不是 usize 往返，AUDIT P3），
        // 其后是该 vtable 约定的闭包表示。
        let vtable = unsafe { &*(*(ptr as *const *const MemoVTable)) };
        let data_ptr = unsafe { ptr.add(MEMO_PAYLOAD_OFFSET) };

        // SAFETY: `data_ptr` 指向的正是这张 vtable 约定的闭包表示。
        rt.update_memo_core(id, &mut |old| unsafe {
            (vtable.compute.as_fn())(data_ptr, old)
        });
    }

    /// # Safety
    ///
    /// `ptr` 必须指向一个尚未析构过的合法 memo 载荷；调用后载荷即失效。
    pub(crate) unsafe fn universal_memo_drop(ptr: *mut u8) {
        // SAFETY: 布局同 `universal_memo_runner`；析构只会发生一次
        // （`ThunkBox` 的 drop 路径）。
        let vtable = unsafe { &*(*(ptr as *const *const MemoVTable)) };
        let data_ptr = unsafe { ptr.add(MEMO_PAYLOAD_OFFSET) };
        unsafe { (vtable.drop.as_fn())(data_ptr) };
    }
}

/// memo 内联载荷中闭包相对于缓冲区起始处的偏移（前面是 `*const MemoVTable`）。
pub(crate) const MEMO_PAYLOAD_OFFSET: usize = mem::size_of::<usize>();

pub(crate) struct MemoVTable {
    /// `old` 是**借**给计算闭包的旧值（首算时是占位值，`downcast_ref` 会得到
    /// `None`）。绝不要在这里克隆它 —— 是否需要一份拷贝由用户闭包自己决定。
    pub(crate) compute: FuncPtr<unsafe fn(*const u8, Option<&AnyValue>) -> AnyValue>,
    pub(crate) drop: FuncPtr<unsafe fn(*mut u8)>,
}

pub(crate) static UNIVERSAL_MEMO_THUNK_VTABLE: ThunkVTable = ThunkVTable {
    drop: FuncPtr::new(Runtime::universal_memo_drop),
    call: FuncPtr::new(Runtime::universal_memo_runner),
};

impl GraphExecutor for Runtime {
    fn run_computation(&self, id: NodeId) -> bool {
        self.run_node(id)
    }
}
