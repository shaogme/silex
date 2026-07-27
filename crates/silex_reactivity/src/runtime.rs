use std::{cell::Ref, mem};

pub(crate) mod graph;
pub(crate) mod guard;
pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::{
    graph::NodeState,
    guard::{
        ComputationGuard, DepthGuard, NodeRunGuard, PayloadGuard, PropagateQueue, QueueGuard,
        SignalValueGuard,
    },
    scheduler::*,
    scope::Scopes,
    storage::*,
};
use crate::{
    DependencyList, ReactiveError, ReactiveResult,
    internal::{
        arena::Index as NodeId,
        value::{AnyValue, Computation, EffectThunk, MemoThunk},
    },
};

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

impl Runtime {
    // --- 种类判定：为 `Handle::<K>::is_alive` 提供依据 ---

    /// 节点存在（不问种类）。
    pub(crate) fn node_exists(&self, id: NodeId) -> bool {
        self.storage.graph.get(id).is_some()
    }

    /// 节点带一个可读的值（signal / memo / derived 三者共用这条）。
    pub(crate) fn node_has_value(&self, id: NodeId) -> bool {
        self.storage.node(id).is_some_and(ReactiveNode::has_value)
    }

    /// 节点是一个 effect（有计算、没有值）。
    pub(crate) fn node_is_effect(&self, id: NodeId) -> bool {
        self.storage.node(id).is_some_and(ReactiveNode::is_effect)
    }

    /// 节点带一个非响应式载荷（stored value / callback / node-ref）。
    pub(crate) fn node_has_payload(&self, id: NodeId) -> bool {
        self.storage.extras.contains_key(id)
    }
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
        self.storage
            .reactive
            .insert(id, ReactiveNode::new_signal(value));
        id
    }

    #[track_caller]
    pub(crate) fn create_effect(&self, f: EffectThunk) -> NodeId {
        let id = self.register_node();
        self.storage
            .reactive
            .insert(id, ReactiveNode::new_effect(f));

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
        let node = self.storage.node(observer)?;
        node.is_computation().then(|| node.effect_version.get())
    }

    /// 把 `observer` 登记进 `target_id` 的订阅者表，返回该 target 当前的版本号。
    ///
    /// 返回 `None` 表示“不必写依赖边”：target 不是可读节点，或本次运行已经登记过
    /// 它（`last_tracked_by` 这条按 signal 存的单条去重缓存）。
    fn subscribe(&self, target_id: NodeId, observer: NodeId, observer_version: u32) -> Option<u32> {
        let target = self.storage.node(target_id)?;
        if !target.has_value() {
            return None;
        }
        if let Some((last_observer, last_version)) = target.last_tracked_by.get()
            && last_observer == observer
            && last_version == observer_version
        {
            return None;
        }
        target.signal.borrow_mut().subscribers.push(observer);
        target
            .last_tracked_by
            .set(Some((observer, observer_version)));
        Some(target.version.get())
    }

    /// 把若干条 `(target, version)` 依赖边一次性写进 observer 的依赖表。
    fn record_dependencies(&self, observer: NodeId, edges: &[(NodeId, u32)]) {
        if edges.is_empty() {
            return;
        }
        let Some(node) = self.storage.node(observer) else {
            return;
        };
        if !node.is_computation() {
            return;
        }
        let mut slot = node.effect.borrow_mut();
        for &edge in edges {
            slot.dependencies.push(edge);
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
    /// 这里**不能**图省事，在整个循环期间一直握着 observer 的载荷借用：
    /// 循环体内会去改每个 target 的订阅者表，而 target 与 observer 可能是同一个
    /// 节点（句柄被回收复用之后就会这样）—— 那就是一次 `RefCell` 双重借用。
    /// 阶段三之前这个错误更严重：它是一次静默的别名违规，
    /// `tests/aliasing.rs` 里有对应的 Miri 探针。
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
        let mut queue = PropagateQueue::acquire(&self.scheduler.workspace);
        self.propagate(source_id, queue.get());
    }

    /// 必要时把一个节点算干净。
    ///
    /// 绝大多数读取的目标是一个**永远干净的普通 signal**，而整套求值机制对它们
    /// 是纯粹的浪费：借出工作栈、进 `evaluate` 看一眼状态就返回、再还回去，
    /// 外加一个深度守卫和一次空队列的 `run_queue`。实测这条提前返回让无 owner
    /// 上下文的 signal 读取从 20.5 ns 降到 11.6 ns（−43%），effect 内的追踪读取
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
            self.drive_eval(node_id);
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

    /// 一个节点没有 `reactive` 条目时，到底是“查无此节点”还是“种类不对”。
    ///
    /// stored value / callback / node-ref 的载荷住在 `extras` 表里，它们在
    /// `reactive` 表里根本没有条目 —— 拿这样一个（经 `RawNodeId` 擦除的）句柄
    /// 去读 signal，报的应当是 `WrongKind`。只在失败路径上问，标 `#[cold]`。
    #[cold]
    fn missing_value_reason(&self, id: NodeId) -> ReactiveError {
        if self.node_exists(id) {
            ReactiveError::WrongKind
        } else {
            ReactiveError::NoSuchNode
        }
    }

    /// 把 signal 的值移出节点。失败原因见 [`ReactiveError`]。
    ///
    /// `None` 就是“正被某个用户闭包借出”—— 阶段三之前这是一个 `updating: bool`
    /// 加一个现造的 `AnyValue::placeholder()` 填进节点，读的时候还要靠一个
    /// `#[cold]` 的分类函数把“你重入了”和“你类型写错了”分开。
    fn take_signal_value(&self, id: NodeId) -> ReactiveResult<AnyValue> {
        let node = self
            .storage
            .node(id)
            .ok_or_else(|| self.missing_value_reason(id))?;
        if !node.has_value() {
            return Err(ReactiveError::WrongKind);
        }
        node.signal
            .try_borrow_mut()
            .map_err(|_| ReactiveError::Reentrant)?
            .value
            .take()
            .ok_or(ReactiveError::Reentrant)
    }

    /// 以“取出 → 交给用户闭包 → 放回”的方式修改 signal 的值。
    ///
    /// 用户闭包执行期间，节点里的值是 `None`，运行时不再持有任何指向该节点载荷
    /// 的借用 —— 否则闭包内一旦重入访问同一个节点（哪怕只是读一下），就会构造出
    /// 与之重叠的引用，这是实打实的 UB（AUDIT P5）。
    ///
    /// 代价是一条明确的契约：**不允许在 update 闭包内访问同一个 signal**。
    /// debug 构建下会断言失败，release 下该次访问返回 [`ReactiveError::Reentrant`]。
    ///
    /// 版本号由 `f` 的第二个返回值决定：`true` 表示“值真的被改写了”，
    /// 此时版本号在**归还值的那一次查表里**顺带递增（AUDIT P12 定下语义）。
    pub(crate) fn with_signal_value_mut<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&mut AnyValue) -> (R, bool),
    ) -> ReactiveResult<R> {
        let taken = match self.take_signal_value(id) {
            Ok(value) => value,
            Err(e) => {
                debug_assert!(
                    e != ReactiveError::Reentrant,
                    "在 update 闭包内重入访问同一个 signal 是不被支持的"
                );
                return Err(e);
            }
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
    ) -> ReactiveResult<bool> {
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

    /// 借用一个节点的当前值，不做求值也不建立依赖。
    ///
    /// 返回的是一个 [`Ref`]：值本身住在节点的 `RefCell` 里，借用计数由它自己
    /// 维护。因此“在读的过程中写同一个 signal”现在拿到的是一句
    /// [`ReactiveError::Reentrant`]，而不是从前那种静默的别名违规。
    ///
    /// 仍然剩下的契约：这个借用不得跨越对**同一个节点的销毁**（销毁会把整个
    /// `RefCell` 从表里移走）。crate 内部唯一会在借用期间执行用户代码的地方是
    /// `T::clone`，见 crate 文档里的残留契约。
    pub(crate) fn signal_value(&self, id: NodeId) -> ReactiveResult<Ref<'_, AnyValue>> {
        let node = self
            .storage
            .node(id)
            .ok_or_else(|| self.missing_value_reason(id))?;
        // 节点还在图里，只是它没有值（是个 effect）。
        if !node.has_value() {
            return Err(ReactiveError::WrongKind);
        }
        let slot = node
            .signal
            .try_borrow()
            .map_err(|_| ReactiveError::Reentrant)?;
        Ref::filter_map(slot, |s| s.value.as_ref()).map_err(|_| ReactiveError::Reentrant)
    }

    /// 不经借用计数地取一个 signal 值的引用。
    ///
    /// # Safety
    ///
    /// 契约见公开的逃生出口 [`crate::signal::try_value_ref`] 与
    /// [`crate::try_get_any_raw_untracked`]：调用方负责保证在使用期间不发生
    /// 任何会写这个槽位、移动这个值或销毁这个节点的操作。
    pub(crate) unsafe fn signal_value_unchecked(&self, id: NodeId) -> Option<&AnyValue> {
        let node = self.storage.node(id)?;
        if !node.has_value() {
            return None;
        }
        // SAFETY: `RefCell::as_ptr` 绕过借用计数，契约转嫁给调用方（见上）。
        unsafe { (*node.signal.as_ptr()).value.as_ref() }
    }

    pub(crate) fn get_signal_value(&self, id: NodeId) -> ReactiveResult<Ref<'_, AnyValue>> {
        self.prepare_read(id);
        self.signal_value(id)
    }

    pub(crate) fn get_signal_value_untracked(
        &self,
        id: NodeId,
    ) -> ReactiveResult<Ref<'_, AnyValue>> {
        self.prepare_read_untracked(id);
        self.signal_value(id)
    }

    /// 把 signal 的值移出节点、交给**用户闭包**、再放回去（只读版本）。
    ///
    /// 只读也要移出：闭包是用户代码，它可以销毁任何节点 —— 包括这一个。
    /// 代价是与写入侧一致的一条契约：**不允许在闭包内访问同一个 signal**，
    /// 否则拿到 [`ReactiveError::Reentrant`]。
    pub(crate) fn with_signal_value<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&AnyValue) -> R,
    ) -> ReactiveResult<R> {
        let taken = self.take_signal_value(id)?;
        // 守卫保证值一定会被放回（闭包 panic 时也一样），且不递增版本号。
        let borrowed = SignalValueGuard::new(self, id, taken);
        Ok(f(borrowed.value().expect("just moved in")))
    }

    pub(crate) fn prepare_memo_node(&self, id: NodeId, computation: MemoThunk) {
        self.storage
            .reactive
            .insert(id, ReactiveNode::new_memo(computation));
    }

    pub(crate) fn commit_update(&self, id: NodeId, value: AnyValue, changed: bool) {
        if !changed {
            return;
        }
        if let Some(node) = self.storage.node(id) {
            node.bump_version();
            node.signal.borrow_mut().value = Some(value);
        }
        self.notify_update(id);
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
                .node(id)
                .is_some_and(ReactiveNode::is_computation)
            {
                self.update_if_necessary(id);
            }
        }
    }

    /// 装上计算闭包并立即完成首次计算。
    ///
    /// 闭包先被装进节点，再由统一的 [`Runtime::run_node`] 驱动首跑：
    /// 这样首跑与后续重算走同一条路径，也不存在“闭包尚未被节点接管就提前返回”
    /// 导致析构函数永不运行的窗口（AUDIT P19.10）。
    #[inline(never)]
    pub(crate) fn initialize_memo(&self, id: NodeId, thunk: MemoThunk) {
        self.prepare_memo_node(id, thunk);
        self.run_node(id);
    }

    /// 建一个带非响应式载荷的节点（stored value / callback / node-ref 共用）。
    #[track_caller]
    pub(crate) fn store_payload(&self, value: AnyValue) -> NodeId {
        let id = self.register_node();
        self.storage
            .extras
            .insert(id, std::cell::RefCell::new(Some(value)));
        id
    }

    /// 不经借用计数地借用一个载荷。
    ///
    /// # Safety
    ///
    /// 契约见公开的逃生出口 [`crate::store::try_value_ref`]。需要把值交给用户
    /// 闭包时请改用 [`Runtime::with_payload`]，那条路径会先把值移出节点。
    pub(crate) unsafe fn payload_value_unchecked(&self, id: NodeId) -> Option<&AnyValue> {
        let slot = self.storage.extras.get(id)?;
        // SAFETY: `RefCell::as_ptr` 绕过借用计数，契约转嫁给调用方（见上）。
        unsafe { (*slot.as_ptr()).as_ref() }
    }

    /// 把载荷移出节点、交给**用户闭包**、再放回去。
    ///
    /// 这是所有会执行用户代码的载荷访问的唯一入口（审计报告 §2.1）。
    pub(crate) fn with_payload<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&AnyValue) -> R,
    ) -> ReactiveResult<R> {
        let borrowed = PayloadGuard::acquire(self, id)?;
        Ok(f(borrowed.value()))
    }

    /// 同上，可变版本。
    pub(crate) fn with_payload_mut<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&mut AnyValue) -> R,
    ) -> ReactiveResult<R> {
        let mut borrowed = PayloadGuard::acquire(self, id)?;
        Ok(f(borrowed.value_mut()))
    }

    /// 取出节点内部值的裸指针（signal 与 stored value 都支持）。
    ///
    /// # Safety
    ///
    /// 契约见 [`crate::try_get_any_raw_untracked`]：调用方负责类型正确，
    /// 并保证在使用期间不发生任何会让该地址失效的操作。
    pub(crate) unsafe fn get_any_raw_ptr_untracked(&self, id: NodeId) -> Option<*const ()> {
        // 先把节点算干净，**再**取指针。少了这一步，读一个脏 memo / derived 会
        // 安静地拿到上一轮的值 —— 上层框架的类型擦除读取路径正是走这里，
        // 而它读到的可能是任何一种可读节点。干净节点（普通 signal、stored value）
        // 会在 `update_if_necessary` 的入口直接返回，不额外付钱（AUDIT 二轮 §1.3）。
        //
        // 顺序不能反：求值会执行用户代码（memo 闭包、被冲掉的 effect 队列），
        // 那正是本函数的 `# Safety` 段里说的“会让指针失效”的操作。
        self.update_if_necessary(id);

        // SAFETY: 契约转嫁给调用方（见本函数的 `# Safety`）。
        unsafe {
            if let Some(value) = self.signal_value_unchecked(id) {
                return Some(value.as_ptr());
            }
            self.payload_value_unchecked(id).map(|v| v.as_ptr())
        }
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
    /// 它同时也是一个**驱动**：所有会执行用户代码的步骤（cleanup、计算闭包）
    /// 都发生在这一层，而它们需要的载荷（闭包、cleanup 列表、依赖表）已经由
    /// [`Runtime::begin_run`] 整个移出了节点。方案 B 之下这一层会拆成若干次
    /// `with_rt`，用户代码落在两次借用之间。
    ///
    /// 返回值表示是否真的执行了计算闭包。以下情况返回 `false`：
    /// 节点不存在、不是计算节点、或**正在运行中**。
    pub(crate) fn run_node(&self, id: NodeId) -> bool {
        let Some(ticket) = self.begin_run(id) else {
            return false;
        };

        // 从这里开始，闭包的归还与重入锁的释放由守卫接管（panic 展开时同样生效）。
        let run_guard = NodeRunGuard::new(self, id, ticket.computation);

        // 清理上一次运行留下的子节点、cleanup 与订阅关系。
        self.run_cleanups(id, ticket.children, ticket.cleanups, ticket.dependencies);

        let Some(computation) = run_guard.computation.as_ref() else {
            return false;
        };

        // 状态在调用用户闭包**之前**置 Clean。运行期间产生的失效标记
        // （例如 effect 写了自己的依赖）因此得以保留，不会被“运行完再无条件置
        // Clean”抹掉（AUDIT P8）。
        self.storage.set_state(id, NodeState::Clean);

        let _ctx = ComputationGuard::enter(self, id);
        match computation {
            Computation::Effect(f) => f.call(),
            Computation::Memo(f) => self.recompute_memo(id, f),
        }
        true
    }

    /// 运行的前置阶段：加重入锁，把计算闭包与上一次运行的残留整个移出节点。
    ///
    /// 重入检查必须发生在任何破坏性操作之前 —— 一旦第二次执行前置阶段，
    /// 节点的依赖列表会被清空、订阅关系被摘除，而重建订阅的那一步却因为
    /// 计算闭包已被借出而被跳过，该节点从此永久失联（AUDIT P1）。
    ///
    /// 返回 `None` 表示不该运行：节点不存在、不是计算节点、或正在运行中。
    fn begin_run(&self, id: NodeId) -> Option<RunTicket> {
        let node = self.storage.node(id)?;
        if !node.is_computation() || node.is_running() {
            return None;
        }
        node.set_running(true);
        node.effect_version
            .set(node.effect_version.get().wrapping_add(1));
        let (computation, dependencies) = {
            let mut slot = node.effect.borrow_mut();
            (slot.computation.take(), mem::take(&mut slot.dependencies))
        };
        let (children, cleanups) = self.take_scope_state(id);
        Some(RunTicket {
            computation,
            children,
            cleanups,
            dependencies,
        })
    }

    /// 重算一个 memo：借出旧值 → 调用计算闭包 → 与旧值比较 → 提交。
    ///
    /// 旧值是**借**给计算闭包的，不是克隆给它的。之前这里为一次重算克隆旧值三次
    /// （节点里克隆一份、再克隆一份传给 vtable、vtable 里再 `cloned()` 一次），
    /// 而且闭包用不用 `old` 都要付这个代价 —— 对持有 `Vec` / `String` 的 memo
    /// 就是每次重算三次深拷贝（AUDIT P9）。
    ///
    /// 旧值在计算期间被**移出**节点，理由与 `update_signal` 相同：计算闭包是用户
    /// 代码，运行时不能在它执行期间持有指向节点载荷的借用（AUDIT P5）。因此
    /// “在 memo 的计算闭包里读它自己”读到的是 [`ReactiveError::Reentrant`]，
    /// 旧值只能从闭包参数拿 —— 这本来也是 `Fn(Option<&T>) -> T` 这个签名的用途。
    ///
    /// 这里从前隔着一层“通用 runner”：memo 的闭包被打包成一个 `ThunkValue`，
    /// 运行时把 `*const Runtime` 传进去，runner 再从 `current_owner()` 反查自己是
    /// 哪个节点、把 vtable 从载荷偏移 0 处读回来、回调进 `update_memo_core`。
    /// 驱动本来就知道 id、也拿得到旧值，那一层因此整个删掉了（方案 B §5.2）。
    #[inline(never)]
    fn recompute_memo(&self, id: NodeId, thunk: &MemoThunk) {
        // 首算时节点里本来就没有值；节点不存在、或值正被某个 update 闭包借出时
        // 同样没有可用的旧值 —— 一律按“变了”处理，`commit_update` 自己会跳过
        // 不存在的节点。
        let taken = self.take_signal_value(id).ok();

        // 守卫保证旧值一定会被放回（计算闭包 panic 时也一样）。
        let borrowed = taken.map(|value| SignalValueGuard::new(self, id, value));

        let new_any = thunk.compute(borrowed.as_ref().and_then(SignalValueGuard::value));

        // 比较也在旧值还被借出时进行：`try_eq` 会调用用户的 `PartialEq`，
        // 同样不该在运行时持有节点借用的情况下运行。
        let changed = match borrowed.as_ref().and_then(SignalValueGuard::value) {
            Some(old) => !new_any.try_eq(old),
            None => true,
        };

        // 旧值先回到节点（并解除借出状态），随后 `commit_update` 才可能覆盖它。
        drop(borrowed);
        self.commit_update(id, new_any, changed);
    }
}

/// 一次计算的“待办”：所有载荷都已按值移出节点。
///
/// 它的存在就是为了让执行用户代码的那一层不必再向运行时要任何东西 ——
/// 方案 B 之下取票与执行分处两次不同的借用。
struct RunTicket {
    computation: Option<Computation>,
    children: Vec<NodeId>,
    cleanups: crate::runtime::storage::CleanupList,
    dependencies: DependencyList,
}
