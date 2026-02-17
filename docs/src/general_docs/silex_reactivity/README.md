# Silex Reactivity 引擎

`silex_reactivity` 是 Silex 框架的底层响应式引擎。它实现了一个**类型擦除 (Type-Erased)**、**细粒度 (Fine-Grained)** 的响应式图谱。

## 设计理念

该 crate 采用了**引擎与接口分离**的设计模式：

*   **Runtime (运行时)**：负责管理节点图谱、数据存储 (`Arena` / `SparseSecondaryMap`)、副作用调度和内存管理。
*   **Algorithm (算法)**：核心图算法（如 BFS 状态传播、迭代式 DFS 求值）被解耦到 `algorithm.rs` 模块，并通过 `ReactiveGraph` trait 与运行时交互。
*   **Type Erasure (类型擦除)**：所有的信号值都以 `AnyValue` 的形式存储。这是一种支持**小对象优化 (SOO)** 的异构容器，使得运行时可以统一管理不同类型的信号，且避免了小数据的堆分配。
*   **Zero-Allocation (零分配)**：利用 `WorkSpace` 对象池复用 `Vec` 和 `VecDeque`，在图遍历和更新传播过程中实现摊销零分配。
*   **Arena 存储**：使用定制的 `Arena` 和 `SparseSecondaryMap` 存储节点数据，提供稳定的 `NodeId` 引用和高效的内存访问（通过分块和代际索引）。

## 核心架构

### 1. Runtime (运行时)

`Runtime` 是一个线程局部 (Thread-Local) 的单例，包含了整个响应式系统的状态：

```rust
pub struct Runtime {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux, 32>, // 冷数据存储
    pub(crate) signals: SparseSecondaryMap<SignalData, 64>, // 信号数据
    pub(crate) effects: SparseSecondaryMap<EffectData, 64>, // 副作用数据
    pub(crate) states: SparseSecondaryMap<NodeState, 64>,   // 节点状态 (Clean/Check/Dirty)
    
    // 任务队列与工作区
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) workspace: RefCell<WorkSpace>,
    // ...
}
```

### 2. Arena & SparseSecondaryMap (内存管理)

`silex_reactivity` 不再依赖外部的 ECS 或 Arena 库，而是实现了定制的内存分配策略：

*   **Arena**: 采用分块 (`Chunk`) 存储和代际索引 (`Generational Index`)。它使用 `UnsafeCell` 提供了内部可变性，允许在不违反 Rust 借用规则的前提下高效地构建自引用的响应式图谱。
*   **SparseSecondaryMap**: 配合 `Arena` 使用的辅助存储，用于映射 `NodeId` 到特定的组件数据（如 `SignalData`, `EffectData`, `NodeAux`）。支持可配置的块大小 (`const N: usize`) 以平衡内存占用和缓存局部性。

### 3. NodeId 与 Node (拓扑结构)

*   **NodeId**: `arena::Index` 的别名。包含 `index` (u32) 和 `generation` (u32)。
*   **Node**: 仅存储最核心的图谱信息（如 `parent`），以保持轻量级，利于 CPU 缓存。
*   **NodeAux**: 存储辅助性或“冷”数据，如子节点列表 (`children`)、清理回调 (`cleanups`) 和上下文 (`context`)。这些数据不常被访问，因此从 `Node` 中分离出来。
*   **NodeState**: 节点的响应式状态，用于惰性求值优化。
    *   `Clean`: 节点是最新的。
    *   `Check`: 依赖可能已变动，需要检查。
    *   `Dirty`: 节点已过时，必须重新计算。

### 4. SignalData (信号数据)

信号是响应式图谱中的数据源。

*   **Value**: 使用 `AnyValue` 存储。如果数据较小（如 `i32`, `bool`, `f64`），直接内联存储；否则才使用堆分配 (`Box`)。
*   **Subscribers**: 订阅了该信号变更的副作用节点列表。内部使用了优化过的枚举结构 `NodeList` (`Empty`/`Single`/`Many`) 来减少常见单订阅场景的内存占用。
*   **Version & Tracking**: 维护 `version` 和 `last_tracked_by`，用于优化依赖收集，防止重复注册。

### 5. EffectData (副作用数据)

副作用是响应式图谱中的观察者。

*   **Computation**: 实际执行的闭包逻辑。为了性能，以 `Option<Box<dyn Fn()>>` 形式存储（执行时取出所有权，避免引用计数开销）。
*   **Dependencies**: 该副作用依赖的信号列表。使用 `DependencyList` (`List<(NodeId, u32)>`) 存储依赖 ID 及其版本号，用于变更检测。

### 6. Memo (派生数据)

派生数据（Memo）是信号和副作用的混合体，用于缓存计算结果。

*   **Composition (组合式)**: 这里没有独立的 `DerivedData` 结构体。一个 Memo 节点实际上是同时拥有 `SignalData`（作为数据源被下游消费）和 `EffectData`（作为观察者依赖上游）的 Node。
*   **Lazy Evaluation (惰性求值)**: 利用 `algorithm::evaluate` 实现迭代式 DFS 求值。当 Memo 被访问时，根据 `NodeState` 决定是否需要重新计算。如果状态为 `Check`，会先检查所有依赖的版本号 (`dependency_versions`) 是否变更，若无变更则直接转为 `Clean`，避免不必要的计算。

## 关键机制

### 自动依赖追踪

当一个副作用执行时，`Runtime` 会将其设为 `current_owner`。在此期间读取的任何信号都会自动将该副作用注册为订阅者。

```rust
// 伪代码流程
effect(|| {
    // try_get_signal 内部调用 track_dependency
    let value = try_get_signal(id).unwrap(); 
    println!("Value: {}", value);
});
```

### 状态传播与批量更新

*   **Propagation (BFS)**: 当信号更新时，`algorithm::propagate` 使用广度优先搜索 (BFS) 遍历所有下游节点。
    *   将直接订阅者标记为 `Dirty`。
    *   将更下游的节点标记为 `Check`。
    *   将纯副作用节点 (`EffectData` only) 加入 `observer_queue`。
*   **Queue Execution**: 批量更新阶段（`run_queue`），运行时从队列中取出节点并执行。对于 Memo 节点，此时仅标记状态；对于 Effect 节点，则执行其计算闭包。
*   **Zero-Allocation**: 这一过程使用的 `Vec` 和 `VecDeque` 均从 `WorkSpace` 对象池中借用，用完即还。

### 内存管理与清理

*   **Dispose**: 调用 `dispose(id)` 会递归清理该节点及其所有子节点。
*   **Cleanup**: 副作用重新执行前，会自动清理旧的依赖关系（反注册订阅）和注册的清理回调 (`on_cleanup`)。
