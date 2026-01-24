# Silex Reactivity 引擎

`silex_reactivity` 是 Silex 框架的底层响应式引擎。它实现了一个**类型擦除 (Type-Erased)**、**细粒度 (Fine-Grained)** 的响应式图谱。

## 设计理念

该 crate 采用了**引擎与接口分离**的设计模式：

*   **Runtime (运行时)**：负责管理节点图谱、依赖收集、副作用调度和内存管理。
*   **Type Erasure (类型擦除)**：所有的信号值都以 `Box<dyn Any>` 的形式存储。这使得运行时可以统一管理不同类型的信号，而不需要泛型参数污染运行时结构。
*   **SlotMap 存储**：使用 `SlotMap` 和 `SecondaryMap` 存储节点数据，提供稳定的 `NodeId` 引用和高效的内存访问。

## 核心架构

### 1. Runtime (运行时)

`Runtime` 是一个线程局部 (Thread-Local) 的单例，包含了整个响应式系统的状态：

```rust
pub struct Runtime {
    pub(crate) nodes: RefCell<SlotMap<NodeId, Node>>,
    pub(crate) signals: RefCell<SecondaryMap<NodeId, SignalData>>,
    pub(crate) effects: RefCell<SecondaryMap<NodeId, EffectData>>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    // ...
}
```

### 2. NodeId 与 Node

*   **NodeId**: 一个轻量级的句柄（newtype around valid key），用于引用图中的任何节点（信号、副作用、计算属性等）。
*   **Node**: 存储通用的图谱信息，如父子关系 (`parent`, `children`)、清理回调 (`cleanups`) 和上下文 (`context`)。

### 3. SignalData (信号数据)

信号是响应式图谱中的数据源。

*   **Value**: 使用 `Box<dyn Any>` 存储任意类型的值。
*   **Subscribers**: 订阅了该信号变更的副作用节点列表。

### 4. EffectData (副作用数据)

副作用是响应式图谱中的观察者。

*   **Computation**: 实际执行的闭包逻辑。
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
