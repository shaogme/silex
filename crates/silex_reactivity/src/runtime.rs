use std::{cell::Ref, mem, panic::Location};

pub(crate) mod drive;
pub(crate) mod graph;
pub(crate) mod guard;
pub(crate) mod scheduler;
pub(crate) mod scope;
pub(crate) mod storage;

use self::{graph::NodeState, scheduler::*, scope::Scopes, storage::*};
use crate::{
    DependencyList, ReactiveError, ReactiveResult,
    internal::{
        arena::Index as NodeId,
        value::{AnyValue, Computation, MemoThunk},
    },
};

pub(crate) struct Runtime {
    pub(crate) storage: Storage,
    pub(crate) scheduler: Scheduler,
    pub(crate) scopes: Scopes,
}

/// 线程本地的运行时。
///
/// # 为什么外面包了一层 `RefCell`
///
/// 从前这里直接是 `ThreadLocal<Runtime>`，`RUNTIME.get()` 交出一个 `&Runtime`，
/// 于是运行时内部的一切可变性都得靠 `Cell` / `RefCell` / `UnsafeCell` 自己扛，
/// 而“执行用户代码时不得持有指向节点的引用”这条不变量只能靠逐点审读维系
/// （审计报告 §2.1、§4 阶段三）。
///
/// 现在访问入口收成 [`with_rt`] 一个函数，它交出的是 `&mut Runtime` ——
/// **独占**的。后果有三：
///
/// 1. 存储层不再需要内部可变性，可以退回成普通的安全容器（这是整条路线的
///    全部收益所在）；
/// 2. 用户代码只可能在两次借用**之间**执行，因为借用出不了 `with_rt` 的闭包 ——
///    见 [`drive`] 模块；
/// 3. 万一哪条路径不小心在借用之内跑了用户代码，那次重入拿到的是一句
///    [`ReactiveError::Reentrant`]，而不是一次静默的别名违规。
pub(crate) static RUNTIME: silex_thread_local::ThreadLocal<std::cell::RefCell<Runtime>> =
    silex_thread_local::ThreadLocal::new();

/// 触碰运行时的**唯一**方式。
///
/// `f` 只能是运行时内部代码，**绝不能是用户代码** —— 用户代码一律由 [`drive`]
/// 里的驱动循环在两次 `with_rt` 之间执行。
///
/// 这条纪律不由类型系统直接强制（闭包里当然写得出对公开 API 的调用），
/// 但它的**后果**由类型系统兜住了：`&mut Runtime` 是独占的，因此违反纪律的
/// 代价是一句清晰的 `Reentrant`，而不是从前那种要靠 Miri 才看得见的 UB。
#[inline]
pub(crate) fn with_rt<R>(f: impl FnOnce(&mut Runtime) -> R) -> ReactiveResult<R> {
    // 只读、或只写既有节点的路径一律用 `get()`：没有运行时就没有节点，
    // 不该仅仅为了报告“查无此节点”而把整个运行时建起来（AUDIT P19.9）。
    let cell = RUNTIME.get().ok_or(ReactiveError::NoRuntime)?;
    let mut rt = cell
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    Ok(f(&mut rt))
}

/// 同 [`with_rt`]，但运行时不存在时先把它建出来。
///
/// 只有真正会**创建**节点的入口才该用它。
#[inline]
pub(crate) fn with_rt_or_init<R>(f: impl FnOnce(&mut Runtime) -> R) -> ReactiveResult<R> {
    let cell = RUNTIME.get_or(|| std::cell::RefCell::new(Runtime::new()));
    let mut rt = cell
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    Ok(f(&mut rt))
}

/// [`Runtime::track_dependencies`] 一次攒多少条依赖边再写回 observer。
///
/// 只影响“observer 查表被摊薄多少倍”，不影响语义。取 16：`signal::track_batch`
/// 的真实调用点一次登记的 signal 数是个位数到十几个。
const TRACK_BATCH: usize = 16;

/// 一次计算的“待办”：所有载荷都已按值移出节点。
///
/// 它的存在就是为了让执行用户代码的那一层不必再向运行时要任何东西 ——
/// 取票（[`Runtime::begin_run`]）与执行（[`drive::run_node`]）分处两次不同的借用。
pub(crate) struct RunTicket {
    pub(crate) computation: Option<Computation>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) cleanups: CleanupList,
    pub(crate) dependencies: DependencyList,
}

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

    pub(crate) fn create_signal_at(
        &mut self,
        at: &'static Location<'static>,
        value: AnyValue,
    ) -> NodeId {
        let id = self.register_node_at(at);
        self.storage
            .reactive
            .insert(id, ReactiveNode::new_signal(value));
        id
    }

    /// 建一个带非响应式载荷的节点（stored value / callback / node-ref 共用）。
    pub(crate) fn store_payload_at(
        &mut self,
        at: &'static Location<'static>,
        value: AnyValue,
    ) -> NodeId {
        let id = self.register_node_at(at);
        self.storage
            .extras
            .insert(id, std::cell::RefCell::new(Some(value)));
        id
    }

    pub(crate) fn prepare_memo_node(&mut self, id: NodeId, computation: MemoThunk) {
        self.storage
            .reactive
            .insert(id, ReactiveNode::new_memo(computation));
    }

    // --- 依赖追踪 ---

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

    // --- 传播 ---

    /// 把下游标记为脏并把其中的 effect 推进队列。
    ///
    /// 全程不执行一行用户代码，所以整个跑在一次借用之内；工作队列从池子里借、
    /// 用完还回去，无需守卫（传播路径上没有 panic）。
    pub(crate) fn queue_dependents(&mut self, source_id: NodeId) {
        let mut queue = self.scheduler.workspace.borrow_deque();
        self.propagate(source_id, &mut queue);
        self.scheduler.workspace.return_deque(queue);
    }

    // --- 读 ---

    /// 这次读取能不能在**一次借用**里走完：目标已经干净，且队列里没有待办。
    ///
    /// `observer_queue` 那半个条件是为了保住“读取也是一个 flush 出口”这条现有
    /// 语义：队列里还有待办时照旧走完整路径，让这次读取把它们冲掉。
    #[inline]
    pub(crate) fn is_settled(&self, id: NodeId) -> bool {
        self.storage.get_state(id) == NodeState::Clean && self.scheduler.observer_queue.is_empty()
    }

    /// effect 队列现在该不该跑。
    ///
    /// 这是 effect 的唯一调度判据（AUDIT P15）：不在 batch 里、不在求值 DFS 里、
    /// 没有别的 `run_queue` 正在跑 —— 外加“队列里确实有东西”。
    ///
    /// 最后那一条是纯粹的短路：队列空时 `run_queue` 进去也只是抢一次标志、
    /// 弹一次空队列、再放掉标志，而在方案 B 之下那是**三次**线程本地查表。
    /// 0 订阅者的写入是最常见的写入形态，这一条把它从 7 次借用降到 2 次。
    #[inline]
    pub(crate) fn should_flush(&self) -> bool {
        self.scheduler.batch_depth == 0
            && self.scheduler.evaluating == 0
            && !self.scheduler.running_queue
            && !self.scheduler.observer_queue.is_empty()
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
    /// `None` 就是“正被某个用户闭包借出”—— 从前这是一个 `updating: bool`
    /// 加一个现造的 `AnyValue::placeholder()` 填进节点，读的时候还要靠一个
    /// `#[cold]` 的分类函数把“你重入了”和“你类型写错了”分开。
    pub(crate) fn take_signal_value(&self, id: NodeId) -> ReactiveResult<AnyValue> {
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

    /// 借用一个节点的当前值，不做求值也不建立依赖。
    ///
    /// 返回的 [`Ref`] 只在**这一次借用**之内有效 —— 它出不了 `with_rt` 的闭包，
    /// 因此“借用跨越用户代码”这件事在类型层面就发生不了。crate 里唯一会在它
    /// 存活期间执行用户代码的地方是读路径上的 `T::clone`，而那次重入拿到的是
    /// [`ReactiveError::Reentrant`]（`with_rt` 的 `try_borrow_mut` 失败），
    /// 不再是从前那条“`clone` 不得销毁这个节点”的手工契约。
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

    /// 不经借用计数地借用一个载荷。
    ///
    /// # Safety
    ///
    /// 契约见公开的逃生出口 [`crate::store::try_value_ref`]。需要把值交给用户
    /// 闭包时请改用 [`drive::with_payload`]，那条路径会先把值移出节点。
    pub(crate) unsafe fn payload_value_unchecked(&self, id: NodeId) -> Option<&AnyValue> {
        let slot = self.storage.extras.get(id)?;
        // SAFETY: `RefCell::as_ptr` 绕过借用计数，契约转嫁给调用方（见上）。
        unsafe { (*slot.as_ptr()).as_ref() }
    }

    // --- 运行 ---

    /// 运行的前置阶段：加重入锁，把计算闭包与上一次运行的残留整个移出节点。
    ///
    /// 重入检查必须发生在任何破坏性操作之前 —— 一旦第二次执行前置阶段，
    /// 节点的依赖列表会被清空、订阅关系被摘除，而重建订阅的那一步却因为
    /// 计算闭包已被借出而被跳过，该节点从此永久失联（AUDIT P1）。
    ///
    /// 返回 `None` 表示不该运行：节点不存在、不是计算节点、或正在运行中。
    ///
    /// 上一次运行的残留里，只有**子节点**与 **cleanup** 需要执行用户代码；
    /// 退订不需要。所以在没有子节点也没有 cleanup 的常见情形（memo 重算、
    /// 不建子节点的 effect 重跑）里，退订就在这一次借用里顺手做掉，票据带回去的
    /// 是一张空依赖表 —— 省掉驱动层的一次往返。
    pub(crate) fn begin_run(&mut self, id: NodeId) -> Option<RunTicket> {
        let node = self.storage.node(id)?;
        if !node.is_computation() || node.is_running() {
            return None;
        }
        node.set_running(true);
        node.effect_version
            .set(node.effect_version.get().wrapping_add(1));
        let (computation, mut dependencies) = {
            let mut slot = node.effect.borrow_mut();
            (slot.computation.take(), mem::take(&mut slot.dependencies))
        };
        let (children, cleanups) = self.take_scope_state(id);

        if children.is_empty() && matches!(cleanups, CleanupList::Empty) {
            self.unsubscribe(id, mem::take(&mut dependencies));
        }

        Some(RunTicket {
            computation,
            children,
            cleanups,
            dependencies,
        })
    }
}
