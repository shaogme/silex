# Crate: `silex_reactivity`

**Low-level, untyped, fine-grained reactivity engine for Silex.**

此 Crate 实现了响应式图谱的核心逻辑。它不通过泛型暴露类型，而是使用 `Box<dyn Any>` 进行**类型擦除**，以便在运行时统一管理依赖关系。上层 `silex_core` 负责提供类型安全。

## 核心架构 (Architecture)

### 1. `Runtime` (运行时)
*   **Thread Local**: `thread_local! { pub static RUNTIME: Runtime ... }`
*   **Components**:
    *   `nodes: SlotMap<NodeId, Node>`: 存储所有响应式节点（Metadata）。
    *   `signals: SecondaryMap<NodeId, SignalData>`: 存储信号值及订阅者。
    *   `effects: SecondaryMap<NodeId, EffectData>`: 存储副作用计算及依赖。
    *   `observer_queue: VecDeque<NodeId>`: 待执行的副作用队列（BFS 调度）。
    *   `current_owner: Option<NodeId>`: 当前正在执行的副作用/包括 Scope，用于依赖收集和 Cleanup 注册。
    *   `batch_depth: Cell<usize>`: 当前批量更新的嵌套深度。

### 2. `NodeId`
*   **Type**: `slotmap::new_key_type!`
*   **Semantics**: 响应式图谱中的唯一句柄，实现了 `Copy`, `Clone`, `Eq`, `Hash`.

### 3. `Node` (Graph Metadata)
*   **Fields**:
    *   `children: Vec<NodeId>`: 子节点（用于级联销毁）。
    *   `parent: Option<NodeId>`: 父节点（Owner）。
    *   `cleanups: Vec<Box<dyn FnOnce()>>`: `on_cleanup` 注册的回调。
    *   `context: Option<HashMap<TypeId, Box<dyn Any>>>`: 依赖注入容器。

### 4. `SignalData` (Source)
*   **Fields**:
    *   `value: Box<dyn Any>`: 存储信号的实际值（类型擦除）。
    *   `subscribers: Vec<NodeId>`: 依赖此信号的副作用列表。
    *   `last_tracked_by: Option<(NodeId, u64)>`: 简单的缓存，防止重复追踪。

### 5. `EffectData` (Observer)
*   **Fields**:
    *   `computation: Option<Rc<dyn Fn()>>`:副作用逻辑闭包。
    *   `dependencies: Vec<NodeId>`: 此副作用依赖的信号（用于重新执行前清理依赖）。
    *   `effect_version: u64`: 用于版本检查（防止旧的依赖关系污染）。

---

## 内部运行时方法 (Internal Runtime Methods)

以下方法直接操作 `Runtime`，通常通过公共 API 间接调用。

### `register_node`
*   **Signature**: `fn register_node(&self) -> NodeId`
*   **Logic**: 创建新 `Node`，自动连接 `current_owner` 作为父节点。

### `track_dependency`
*   **Signature**: `fn track_dependency(&self, signal_id: NodeId)`
*   **Logic**:
    1.  检查 `current_owner`。
    2.  若存在 Owner，将 Owner 加入 `signal.subscribers`。
    3.  将 `signal_id` 加入 `owner.dependencies`。
*   **Side Effects**: 修改图谱连接关系。

### `queue_dependents`
*   **Signature**: `fn queue_dependents(&self, signal_id: NodeId)`
*   **Logic**: 遍历 `signal.subscribers`，将其加入 `observer_queue` (去重)。

### `run_queue`
*   **Signature**: `fn run_queue(&self)`
*   **Logic**: 循环消耗 `observer_queue`，调用 `run_effect_internal`。确保队列处理期间 `running_queue` 锁住以防重入。

### `clean_node`
*   **Signature**: `fn clean_node(&self, id: NodeId)`
*   **Logic**:
    1.  移除并销毁所有 `children` (递归)。
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
    2.  **Read**: 尝试将 `value` downcast 为 `T` 并 Clone 返回。
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
    1.  创建一个计算节点。
    2.  内部包含一个 `Signal` (存储计算结果) 和一个 `Effect` (监听依赖更新信号)。
    3.  仅当计算结果发生变化 (`!=`) 时，才会触发下游更新。

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
