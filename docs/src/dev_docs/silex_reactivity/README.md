# silex_reactivity 内部开发文档

## 1. 概要 (Overview)

`silex_reactivity` 是 Silex 框架中最底层的核心 crate，它提供了一个**基于推送 (Push-Based)**、**细粒度 (Fine-Grained)** 且**类型擦除 (Type-Erased)** 的响应式引擎。

*   **定义**：它是一个实现了反应式编程原语（Signals, Effects, Memos）的独立库。
*   **作用**：作为 Silex 的“心脏”，负责管理整个应用的状态流、副作用调度和依赖追踪。它不依赖于任何 DOM 或 UI 相关的逻辑，因此可以独立于 Web 环境运行（例如在服务端渲染或非 GUI 应用中）。
*   **目标受众**：框架核心开发者，或希望在 Rust 中实现高性能响应式系统的开发者。建议具备 Rust 指针操作、UnsafeCell 内部可变性以及 `Any` trait 也就是类型擦除的相关知识。

## 2. 理念和思路 (Design Philosophy)

*   **设计背景**：早期的 Silex 原型可能依赖于复杂的泛型参数来传递信号类型，导致类型签名极度膨胀。为了简化 API 并支持动态的依赖图谱构建，我们需要一种能够统一管理异构数据的方案。
*   **核心思想**：
    *   **类型擦除 (Type Erasure)**：所有的信号值通过 `AnyValue` 存储（一种支持小对象优化的动态容器）。这使得 Runtime 可以统一管理所有节点，而无需关心具体的泛型类型。
    *   **Arena 内存管理**：使用强类型的 `Index` (即 `NodeId`) 代替引用。这解决了 Rust 中自引用结构体的生命周期难题，并提供了缓存友好的内存布局。
    *   **细粒度更新**：只更新订阅了变化信号的 Effect，而不是重新渲染整个组件树。
*   **方案取舍 (Trade-offs)**：
    *   **运行时开销 vs 编译时复杂性**：为了用户体验，我们选择了动态分发带来的少量运行时开销，以换取极其简洁的 API 和无泛型污染的类型签名。同时，通过 **Small Object Optimization (SOO)** 显著减少了堆分配。
    *   **Unsafe vs RefCell**：为了最大限度地提高性能并绕过 `RefCell` 的运行时借用检查开销（在已知安全的情况下），内部大量使用了 `UnsafeCell` 和裸指针操作。这要求我们必须非常小心地维护不变量。

## 3. 模块内结构 (Internal Structure)

### 目录结构

```text
src/
├── arena.rs        // 定制的 Generational Arena 和稀疏二级映射表
├── lib.rs          // 核心 Runtime 实现，包含 Signal, Effect, Memo 等逻辑
└── value.rs        // AnyValue 实现，提供小对象优化 (SOO)
```

### 核心组件关系

```mermaid
classDiagram
    class Runtime {
        +graph: Arena<Node>
        +signals: SparseSecondaryMap<SignalData>
        +effects: SparseSecondaryMap<EffectData>
        +observer_queue: VecDeque<NodeId>
        +current_owner: Cell<Option<NodeId>>
    }

    class NodeId {
        +index: u32
        +generation: u32
    }

    class Node {
        +children: Vec<NodeId>
        +parent: Option<NodeId>
        +cleanups: Vec<Box<dyn FnOnce()>>
    }

    class SignalData {
        +value: AnyValue
        +subscribers: Vec<NodeId>
    }

    class EffectData {
        +computation: Rc<dyn Fn()>
        +dependencies: Vec<NodeId>
    }

    Runtime "1" *-- "1" Node : Manages Topology
    Runtime "1" *-- "1" SignalData : Stores Data
    Runtime "1" *-- "1" EffectData : Stores Logic
    Node "1" --> "*" Node : Parent/Children
    SignalData "1" --> "*" NodeId : Subscribers
    EffectData "1" --> "*" NodeId : Dependencies
```

*   **Runtime**：线程局部的单例 (Thread-Local Singleton)，拥有所有状态。
*   **Node**：表示依赖图中的一个拓扑节点，负责层级关系（Parent-Child）和生命周期（Cleanup）。
*   **SignalData/EffectData**：通过 `SparseSecondaryMap` 与 `Node` 关联的附加数据。这种设计类似于 ECS 中的组件（Component）。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 Arena 与内存布局 (arena.rs)

`Arena<T>` 是整个系统的基石。为了支持稳定的索引和高效的增删，我们实现了一个基于分块 (`Chunk`) 的代际索引 Arena。

*   **Index (NodeId)**：包含 `index` (u32) 和 `generation` (u32)。`generation` 用于解决 ABA 问题——当一个槽位被释放并重新分配时，旧的 ID 会因为代数不匹配而失效。
*   **Slot<T>**：
    ```rust
    union SlotUnion<T> {
        value: ManuallyDrop<T>,
        next_free: u32,
    }
    struct Slot<T> {
        u: SlotUnion<T>,
        generation: u32, // 偶数表示空闲，奇数表示占用
    }
    ```
    这里使用了 `union` 来复用内存：当槽位空闲时，存储下一个空闲槽位的索引（Free List）；当槽位占用时，存储实际数据。
*   **Interior Mutability**：`Arena::insert` 和 `get_mut` 等方法接收 `&self`，内部使用 `UnsafeCell`。这是为了配合 Runtime 的设计，使得我们可以在持有 Runtime 引用（通常是 `thread_local` 的借用）的同时，修改特定的节点数据。这也意味着调用者（Runtime）必须确保不会同时对同一个 ID 获取两个 `&mut T`。

### 4.2 运行时核心循环 (lib.rs)

`Runtime` 结构体维护了全局状态，包括依赖图谱 (`graph`) 和待执行队列 (`observer_queue`)。

#### 依赖收集 (Dependency Tracking)

当访问一个 Signal 时（例如调用 `try_get_signal`），会触发 `track_dependency`：

1.  检查 `current_owner`（当前正在运行的 Effect ID）。
2.  如果存在 `current_owner`，则建立双向链接：
    *   Signal 将 Effect ID 加入 `subscribers`。
    *   Effect 将 Signal ID 加入 `dependencies`。
3.  **版本检查**：为了减少重复注册，会检查 `last_tracked_by`。如果同一个 Effect 在同一轮执行中多次读取同一个 Signal，只有第一次会触发注册。

#### 变更通知与批处理 (Notification & Batching)

当 Signal 更新时（`update_signal`）：

1.  找到所有订阅者 (`subscribers`)。
2.  将订阅者加入 `observer_queue`。
3.  **批处理 (Batching)**：
    *   如果 `batch_depth == 0`，立即调用 `run_queue` 处理队列。
    *   否则（例如在 `batch(|| ...)` 闭包中），只入队，推迟执行。

#### 清理机制 (Cleanup)

Silex 极其重视资源回收，特别是在复杂的响应式图中：

*   **Effect 重运行前**：必须清理上一轮的依赖关系。这是因为条件分支可能导致依赖改变。如果不清理，Effect update 后可能会继续监听不再需要的 Signal。
*   **节点销毁 (Dispose)**：递归清理 `children`，执行 `cleanups` 回调，并从父节点的子列表中移除自己，最后释放 Arena 槽位。

### 4.3 高级原语实现

*   **Memo (计算属性)**：Memo 本质上是一个“既是 Signal 又是 Effect”的混合体。
    *   它创建一个 Effect 来监听依赖变化。
    *   Effect 的回调函数内部会计算新值，并更新一个内部 Signal。
    *   下游 Effect 监听这个内部 Signal。
    *   **优化**：只有当新计算的值与旧值不等 (`PartialEq`) 时，才会通知下游，从而阻断不必要的更新传播。

*   **NodeRef**：一种特殊的节点，存储弱类型的 DOM 引用或其他外部资源，同样利用 Arena 的生命周期管理机制。

### 4.4 值存储与优化 (value.rs)

为了缓解完全类型擦除带来的堆分配压力（`Box<dyn Any>`），我们引入了 `AnyValue` 结构体实现了**小对象优化 (Small Object Optimization, SOO)**。

*   **原理**：`AnyValue` 内部包含一个固定大小的缓冲区（目前为 3 个 `usize`，即 24 字节 + 8 字节 vtable = 32 字节）。
*   **策略**：
    *   **Inline**：如果类型 `T` 的大小小于等于缓冲区大小且对齐满足要求，直接存储在缓冲区内。
    *   **Boxed**：否则，分配 `Box<T>` 并将指针存储在缓冲区内。
*   **VTable**：手动维护 `vtable` (`type_id`, `drop`, `as_ptr`, `as_mut_ptr`) 来实现动态分发，避免了 Rust 原生 trait object 的双重引用问题，并允许对 Inline 数据进行正确操作。

这意味着像 `bool`, `i32`, `f64`, `usize` 甚至小的结构体现在都**不需要堆内存分配**。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **线程安全性 (Thread Safety)**：目前的 Runtime 基于 `thread_local!`，仅支持单线程运行。虽然这对 CSR 足够，但未来可探索 Send/Sync 支持以适应 Web Workers 等多线程场景。
*   **性能微调**:
    *   [x] 优化 `Box<dyn Any>` 的分配，已实现对小数据类型（如 `bool`, `i32`）的内联存储 (Small Object Optimization)。
    *   优化 `SparseSecondaryMap` 在稀疏数据集下的内存占用。
*   **API 易用性 (Ergonomics)**：计划结合更多的宏（macros）来提供自动解构、自动 Copy 等语法糖，减少样板代码。
*   **调试工具 (DevTools)**：开发可视化的依赖图调试工具，帮助开发者定位循环依赖或无效更新。
