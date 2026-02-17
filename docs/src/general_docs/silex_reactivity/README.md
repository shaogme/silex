# Silex Reactivity 引擎

`silex_reactivity` 是 Silex 框架的底层响应式引擎。它实现了一个**类型擦除 (Type-Erased)**、**细粒度 (Fine-Grained)** 的响应式图谱。

## 设计理念

该 crate 采用了**引擎与接口分离**的设计模式：

*   **Runtime (运行时)**：负责管理节点图谱、依赖收集、副作用调度和内存管理。
*   **Type Erasure (类型擦除)**：所有的信号值都以 `AnyValue` 的形式存储。这是一种支持**小对象优化 (SOO)** 的异构容器，使得运行时可以统一管理不同类型的信号，且避免了小数据的堆分配。
*   **Arena 存储**：使用定制的 `Arena` 和 `SparseSecondaryMap` 存储节点数据，提供稳定的 `NodeId` 引用和高效的内存访问（通过分块和代际索引）。

## 核心架构

### 1. Runtime (运行时)

`Runtime` 是一个线程局部 (Thread-Local) 的单例，包含了整个响应式系统的状态：

```rust
pub struct Runtime {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux>, // 冷数据存储
    pub(crate) signals: SparseSecondaryMap<SignalData>,
    pub(crate) effects: SparseSecondaryMap<EffectData>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: SparseSecondaryMap<()>,
    // ...
}
```

### 2. Arena & SparseSecondaryMap (内存管理)

`silex_reactivity` 不再依赖外部的 ECS 或 Arena 库，而是实现了定制的内存分配策略：

*   **Arena**: 采用分块 (`Chunk`) 存储和代际索引 (`Generational Index`)。它使用 `UnsafeCell` 提供了内部可变性，允许在不违反 Rust 借用规则的前提下高效地构建自引用的响应式图谱。
*   **SparseSecondaryMap**: 配合 `Arena` 使用的辅助存储，用于映射 `NodeId` 到特定的组件数据（如 `SignalData`, `EffectData`, `NodeAux`）。

### 3. NodeId 与 Node (拓扑结构)

*   **NodeId**: `arena::Index` 的别名。包含 `index` (u32) 和 `generation` (u32)，用于安全地引用 Arena 中的槽位。
*   **Node**: 仅存储最核心的图谱信息（如 `parent`），以保持轻量级，利于 CPU 缓存。
*   **NodeAux**: 存储辅助性或“冷”数据，如子节点列表 (`children`)、清理回调 (`cleanups`) 和上下文 (`context`)。这些数据不常被访问，因此从 `Node` 中分离出来。

### 4. SignalData (信号数据)

信号是响应式图谱中的数据源。

*   **Value**: 使用 `AnyValue` 存储。如果数据较小（如 `i32`, `bool`, `f64`），直接内联存储；否则才使用堆分配 (`Box`)。
*   **Subscribers**: 订阅了该信号变更的副作用节点列表。内部使用了优化过的枚举结构 (`Empty`/`Single`/`Many`) 来减少常见单订阅场景的内存占用。

### 5. EffectData (副作用数据)

副作用是响应式图谱中的观察者。

*   **Computation**: 实际执行的闭包逻辑。为了性能，以 `Option<Box<dyn Fn()>>` 形式存储（执行时取出所有权，避免引用计数开销）。
*   **Dependencies**: 该副作用依赖的信号列表（用于自动清理依赖关系）。

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

### 观察者队列与批量更新
    *   **Observer Queue**: 当信号更新时，它不会立即执行依赖它的副作用，而是将它们加入 `observer_queue`。
    *   **Implicit Batching**: 单个 `update_signal` 调用结束时会自动刷新队列。
    *   **Explicit Batching**: 使用 `batch(|| ...)` API 可以推迟队列刷新，直到闭包内的所有操作完成。这对于一次性更新多个相关联的信号非常有用，可以防止 Effect 被多次无效触发。

### 内存管理与清理

*   **Dispose**: 调用 `dispose(id)` 会递归清理该节点及其所有子节点。
*   **Cleanup**: 副作用重新执行前，会自动清理旧的依赖关系和注册的清理回调 (`on_cleanup`)。
