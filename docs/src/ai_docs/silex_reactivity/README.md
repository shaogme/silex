# Crate: `silex_reactivity`

**Low-level, untyped, fine-grained reactivity engine for Silex.**

此 Crate 实现了响应式图谱的核心逻辑。它不通过泛型暴露类型，而是使用 `AnyValue` (带有 Small Object Optimization 的类型擦除容器) 进行统一管理。上层 `silex_core` 负责提供类型安全。

## 核心架构 (Architecture)

### 1. `Runtime` (运行时)
*   **Thread Local**: `thread_local! { pub static RUNTIME: Runtime ... }`
*   **Components**:
    *   `graph: Arena<Node>`: 负责管理节点拓扑结构（Nodes, Parent-Child Relationships）。内部使用 `UnsafeCell` 实现内部可变性。
    *   `node_aux: SparseSecondaryMap<NodeAux>`: 存储节点的“冷数据”（Children, Cleanups, Context）。
    *   `signals: SparseSecondaryMap<SignalData, 64>`: 存储信号值及订阅者。
    *   `effects: SparseSecondaryMap<EffectData, 64>`: 存储副作用计算及依赖。
    *   `states: SparseSecondaryMap<NodeState, 64>`: 存储节点状态 (Clean/Check/Dirty)。
    *   `callbacks: SparseSecondaryMap<CallbackData>`: 存储回调函数。
    *   `node_refs: SparseSecondaryMap<NodeRefData>`: 存储 DOM 节点引用。
    *   `stored_values: SparseSecondaryMap<StoredValueData>`: 存储通用值。
    *   `observer_queue: RefCell<VecDeque<NodeId>>`: 待执行的副作用队列。
    *   `queued_observers: SparseSecondaryMap<()>`: 已入队副作用的集合（用于去重）。
    *   `current_owner: Cell<Option<NodeId>>`: 当前正在执行的副作用/包括 Scope，用于依赖收集和 Cleanup 注册。
    *   `workspace: RefCell<WorkSpace>`: 对象池，用于算法层的零分配执行。

### 2. `Algorithm`
*   **Modules**: `algorithm.rs`.
*   **ReactiveGraph Trait**: 解耦算法与 Runtime 数据结构。
*   **Logic**:
    *   **Propagate (BFS)**: 
        *   从更新源开始。
        *   标记直接订阅者 `Dirty`。
        *   标记更深层订阅者 `Check`。
        *   将 Pure Effects 加入 `observer_queue`。
    *   **Evaluate (Iterative DFS)**:
        *   Lazy 求值策略。
        *   如果状态是 `Clean` -> 返回。
        *   如果状态是 `Check` -> 检查所有依赖的 `version` 是否变更。若无变更，转为 `Clean` 并返回。
        *   如果状态是 `Dirty` 或依赖已变更 -> 执行计算 -> 更新状态为 `Clean`。
        *   **Trampoline**: 避免深层递归导致的 Stack Overflow。

### 3. `Arena<T>` (Memory Management)
*   **Structure**: 分块内存池 (`UnsafeCell<Vec<Chunk<T>>>`)。
*   **Features**:
    *   **Generational Indices**: 使用 `Index` (u32 index + u32 generation) 解决 ABA 问题。
    *   **Interior Mutability**: 通过 `UnsafeCell` 提供类似 `RefCell` 的能力，但针对细粒度响应式系统进行了优化。
    *   **Cache Locality**: 数据按块 (`Chunk`) 连续存储。

### 4. `SparseSecondaryMap<T>` (Auxiliary Storage)
*   **Structure**: 稀疏的分块存储 (`UnsafeCell<Vec<Option<Box<[UnsafeCell<Option<T>>]>>>>`)。
*   **Optimization**: 支持泛型 `N` 指定 Chunk Size (例如 `signals` 使用 64, `node_refs` 使用 16)。

### 5. `NodeId`
*   **Type**: `arena::Index`
*   **Semantics**: 响应式图谱中的唯一句柄，包含 `index` 和 `generation`。

### 6. `AnyValue` (Optimized Storage)
*   **Purpose**: 替代 `Box<dyn Any>` 以减少堆分配。
*   **Mechanism**: **Small Object Optimization (SOO)**.
    *   如果 `size_of::<T>() <= 24` bytes (3 words) 且对齐合适，直接存储在结构体内 (Inline)。
    *   否则，存储 `Box<T>` (Boxed)。

### 7. `Node` & `NodeAux` (Graph Metadata)
*   **Optimization**: 采用 "Hot/Cold Splitting" 策略优化缓存命中率。
*   **Node (Hot)**: `parent: Option<NodeId>`, `defined_at`.
*   **NodeAux (Cold)**: `children`, `cleanups`, `context`, `debug_label`.

### 8. `SignalData` (Source)
*   **Fields**:
    *   `value: AnyValue`: 存储信号的实际值。
    *   `subscribers: NodeList`: 优化过的订阅者列表 (`Empty` / `Single` / `Many(ThinVec)`).
    *   `last_tracked_by: Option<(NodeId, u32)>`: 缓存，避免同一次计算中重复注册依赖。
    *   `version: u32`: 信号版本号，每次变更自增。

### 9. `EffectData` (Observer)
*   **Fields**:
    *   `computation: Option<Box<dyn Fn()>>`: 副作用逻辑闭包。`Box` 替代 `Rc`.
    *   `dependencies: DependencyList`: 依赖列表，类型为 `List<(NodeId, u32)>` (存储依赖 ID 和当时的版本号)。
    *   `effect_version: u32`: 副作用自身的版本号。

### 10. `Memo` (Derived Implementation)
*   **Structure**: Memo 节点是同时拥有 `SignalData` 和 `EffectData` 组件的 `Node`。
*   **ECS Style**: 通过组合数据组件而非独立结构体实现。
*   **State**: 利用 `states` Map 存储 `Clean`/`Check`/`Dirty` 状态参与算法调度。

---

## 内部运行时方法 (Internal Runtime Methods)

以下方法直接操作 `Runtime`，通常通过公共 API 间接调用。

### `register_node`
*   **Signature**: `fn register_node(&self) -> NodeId`
*   **Logic**: 委托给 `self.graph.register`。创建新 `Node`，自动连接 `current_owner` 作为父节点。

### `track_dependency`
*   **Signature**: `fn track_dependency(&self, target_id: NodeId)`
*   **Logic**:
    1.  获取 `current_owner`。
    2.  检查 `SignalData.last_tracked_by` 缓存。
    3.  若未追踪 -> 互相注册 (`subscribers` push owner, `dependencies` push target)。
    4.  更新缓存。

### `queue_dependents`
*   **Signature**: `fn queue_dependents(&self, target_id: NodeId)`
*   **Logic**:
    1.  从 `workspace` 借用 `queue` 和 `subs` buffer。
    2.  调用 `algorithm::propagate` 执行 BFS 状态标记和入队。
    3.  归还 buffer。

### `run_queue`
*   **Signature**: `fn run_queue(&self)`
*   **Logic**: 循环消耗 `observer_queue`。
    *   若节点既有 `EffectData` 又有 `SignalData` (Memo) -> 调用 `update_if_necessary`。
    *   若仅有 `EffectData` (Pure Effect) -> 调用 `run_effect_internal`。

### `update_if_necessary`
*   **Logic**:
    1.  从 `workspace` 借用 buffer。
    2.  调用 `algorithm::evaluate` 执行迭代式 DFS 求值。
    3.  归还 buffer。

### `clean_node`
*   **Signature**: `fn clean_node(&self, id: NodeId)`
*   **Logic**:
    1.  从 `graph` 中获取并移除所有 `children` (递归)。
    2.  执行所有 `cleanups`。
    3.  从所有 `dependencies` 的 `subscribers` 列表中移除自身 (断开反向引用)。

---

## 公共接口 (Public API)

所有公共接口均在 `RUNTIME.with(...)` 块中执行。

### Signal API

#### `signal<T>`
*   **Signature**: `pub fn signal<T: 'static>(value: T) -> NodeId`
*   **Semantics**: 注册一个新的 Signal 节点。
*   **Return**: 节点的 `id`。

#### `try_get_signal<T>`
*   **Signature**: `pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T>`
*   **Semantics**:
    1.  **Track**: 调用 `track_dependency(id)`。
    2.  **Read**: 尝试从 Signal 或 Derived 中获取 `value` 并这种 downcast 为 `T`。
*   **Return**: `Some(T)` if type matches and exists, else `None`.

#### `try_get_signal_untracked<T>`
*   **Signature**: `pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T>`
*   **Semantics**: 读取值但不建立依赖关系。

#### `update_signal<T>`
*   **Signature**: `pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T))`
*   **Semantics**:
    1.  **Write**: Downcast `value` 为 `&mut T` 并执行 `f`。
    2.  **Queue**: 调用 `queue_dependents(id)`。
    3.  **Run**: **仅当** `batch_depth == 0` 时，调用 `run_queue()` 立即执行副作用；否则推迟执行。

### Batch API

#### `batch`
*   **Signature**: `pub fn batch<R>(f: impl FnOnce() -> R) -> R`
*   **Semantics**:
    1.  递增 `batch_depth`。
    2.  执行闭包 `f`。
    3.  递减 `batch_depth`。
    4.  若 `batch_depth` 归零，调用 `run_queue()` 执行所有挂起的副作用。
*   **Use Case**: 在一次操作中修改多个信号，避免触发中间状态的副作用，提高性能。

### Effect / Computation API

#### `effect`
*   **Signature**: `pub fn effect<F: Fn() + 'static>(f: F)`
*   **Semantics**: 注册并**立即执行**一次副作用。
*   **Auto-Cleanup**: 每次执行前会自动清理旧的依赖和子节点。

#### `memo<T>`
*   **Signature**: `pub fn memo<T, F>(f: F) -> NodeId where T: PartialEq...`
*   **Semantics**:
    1.  创建一个同时注册了 `SignalData` 和 `EffectData` 的节点。
    2.  初始执行 `f` 计算并存储结果。
    3.  当依赖更新时，标记为 Dirty/Check。
    4.  **Lazy Evaluation**: 下游访问时触发 `evaluate`，重新计算并更新 `value`，仅当 `!=` 时通知下游。

### Lifecycle API

#### `create_scope`
*   **Signature**: `pub fn create_scope<F: FnOnce()>(f: F) -> NodeId`
*   **Semantics**: 创建一个不带计算逻辑的 Owner 节点，用于组织子节点（如 Component 边界）。

#### `on_cleanup`
*   **Signature**: `pub fn on_cleanup(f: impl FnOnce() + 'static)`
*   **Semantics**: 将回调注册到 `current_owner`。当 Owner 重新执行或被销毁时调用。

#### `dispose`
*   **Signature**: `pub fn dispose(id: NodeId)`
*   **Semantics**: 强制销毁一个子树。从父节点移除自身，并递归清理所有资源。

#### `untrack<T>`
*   **Signature**: `pub fn untrack<T>(f: impl FnOnce() -> T) -> T`
*   **Semantics**: 在 `current_owner = None` 的上下文中执行 `f`，防止 `f` 内部的读取操作被外部追踪。

### Context API

#### `provide_context<T>`
*   **Signature**: `pub fn provide_context<T: 'static>(value: T)`
*   **Semantics**: 将值存储在 `current_owner` 的 `context` map 中。

#### `use_context<T>`
*   **Signature**: `pub fn use_context<T: Clone + 'static>() -> Option<T>`
*   **Semantics**: 从 `current_owner` 开始向上遍历 `parent` 链，查找 `TypeId::of::<T>`。
