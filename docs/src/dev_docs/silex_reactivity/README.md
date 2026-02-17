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
├── algorithm.rs    // 核心图算法 (ReactiveGraph Trait, Propagate, Evaluate)
├── arena.rs        // 定制的 Generational Arena 和稀疏二级映射表
├── lib.rs          // 核心 Runtime 实现，包含 Signal, Effect, Memo 等逻辑
├── list.rs         // ThinVec 和 List 枚举实现 (无堆分配/紧凑布局优化)
├── runtime.rs      // Runtime 结构体及核心数据结构 (Node, NodeAux, SignalData 等) 定义
└── value.rs        // AnyValue 实现，提供小对象优化 (SOO)
```

### 核心组件关系

```mermaid
classDiagram
    class Runtime {
        +graph: Arena<Node>
        +node_aux: SparseSecondaryMap<NodeAux>
        +signals: SparseSecondaryMap<SignalData>
        +effects: SparseSecondaryMap<EffectData>
        +states: SparseSecondaryMap<NodeState>
        +node_refs: SparseSecondaryMap<NodeRefData>
        +callbacks: SparseSecondaryMap<CallbackData>
        +stored_values: SparseSecondaryMap<StoredValueData>
        +observer_queue: VecDeque<NodeId>
        +current_owner: Cell<Option<NodeId>>
        +workspace: RefCell<WorkSpace>
    }

    class NodeId {
        +index: u32
        +generation: u32
    }

    class Node {
        +parent: Option<NodeId>
        +defined_at: Option<Location>
    }

    class NodeAux {
        +children: Vec<NodeId>
        +cleanups: CleanupList
        +context: HashMap<TypeId, Box<dyn Any>>
        +debug_label: Option<String>
    }

    class NodeState {
        <<enumeration>>
        Clean
        Check
        Dirty
    }

    class SignalData {
        +value: AnyValue
        +subscribers: NodeList
        +last_tracked_by: Option<(NodeId, u32)>
        +version: u32
    }

    class EffectData {
        +computation: Option<Box<dyn Fn()>>
        +dependencies: DependencyList
        +effect_version: u32
    }

    class DependencyList {
       <<enumeration>>
       Empty
       Single((NodeId, u32))
       Many(ThinVec<(NodeId, u32)>)
    }

    class NodeList {
        <<enumeration>>
        Empty
        Single(NodeId)
        Many(ThinVec<NodeId>)
    }

    Runtime "1" *-- "1" Node : Hot Data (Topology)
    Runtime "1" *-- "1" NodeAux : Cold Data
    Runtime "1" *-- "1" SignalData : Stores Data
    Runtime "1" *-- "1" EffectData : Stores Logic
    Runtime "1" *-- "1" NodeState : Status
    Node "1" --> "*" Node : Parent (via ID)
    NodeAux "1" --> "*" NodeId : Children
    SignalData "1" --> "1" NodeList : Subscribers
    EffectData "1" --> "1" DependencyList : Dependencies
```

*   **Runtime**：线程局部的单例 (Thread-Local Singleton)，拥有所有状态。
*   **Node**：仅包含最核心的热数据（如 `parent`），用于高频访问的图谱遍历。
*   **NodeAux**：存储相对“冷”的数据（如 `children`, `cleanups`），通过 `SparseSecondaryMap` 存储，以提高 `Node` 的缓存局部性。
*   **SignalData/EffectData**：通过 `SparseSecondaryMap` 与 `Node` 关联的附加数据。这种设计类似于 ECS 中的组件（Component）。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 Arena 与内存布局 (arena.rs)

`Arena<T>` 是整个系统的基石。为了支持稳定的索引和高效的增删，我们实现了一个基于分块 (`Chunk`) 的代际索引 Arena。

*   **Index (NodeId)**：包含 `index` (u32) 和 `generation` (u32)。`generation` 用于解决 ABA 问题——当一个槽位被释放并重新分配时，旧的 ID 会因为代数不匹配而失效。
*   **Slot<T>**：使用 `union` 复用内存（Value vs NextFree），并包含 `generation` 字段。
*   **SparseSecondaryMap**：配合 `Arena` 使用，支持自定义 `Block Size` (const N: usize) 以适应不同密度组件的数据存储需求。

### 4.2 Runtime 与算法层 (runtime.rs & algorithm.rs)

我们将核心的图算法从 Runtime 中剥离出来，放入 `algorithm.rs`，二者通过 `ReactiveGraph` trait 进行交互。

#### 核心算法 (Algorithm)

*   **NodeState**：引入了所有的节点状态：
    *   `Clean`: 节点数据有效且最新。
    *   `Check`: 节点的依赖可能发生了变化，需要进行检查（Pull-based 验证）。
    *   `Dirty`: 节点数据已过期，必须重新计算。
*   **Propagate (BFS)**: `algorithm::propagate`。当 Signal 更新时，从起点开始进行广度优先搜索。
    *   直接订阅者 -> `Dirty`。
    *   间接订阅者 -> `Check`。
    *   将纯 Effect 加入 `queue`。
*   **Evaluate (Iterative DFS)**: `algorithm::evaluate`。当需要读取一个节点（如 Memo）的值时调用。
    *   使用 **Trampoline (蹦床)** 技术将递归 DFS 转换为迭代循环，防止深层依赖链导致栈溢出。
    *   **Early Cutoff**: 如果节点状态是 `Check`，会先检查其所有依赖的 `version` 是否发生变化。如果没有变化，直接切回 `Clean` 状态，跳过计算。

#### 工作区 (WorkSpace)

为了实现**零分配 (Zero-Allocation)** 的算法执行，`Runtime` 维护了一个 `WorkSpace`：
*   **Object Pooling**: 内部包含 `vec_pool` 和 `deque_pool`。
*   **Borrow & Return**: `algorithm::propagate` 和 `evaluate` 需要 `Vec` 或 `VecDeque` 作为临时栈/队列。它们从 `WorkSpace` 中借用，使用完毕后清空并归还。这使得高频的图遍历操作不会产生任何堆内存分配和释放的开销。

### 4.3 列表优化 (list.rs)

在响应式图中，绝大多数 Signal 只有 0 或 1 个订阅者。标准 `Vec` 即使为空也会占用空间（或者 allocation overhead）。我们实现了 `List<T>` 和 `ThinVec<T>`：

*   **ThinVec<T>**：
    *   一种手动管理内存布局的 Vector。
    *   **Layout**: `[Header { len, cap }][Data...]`。
    *   **Stack Size**: 仅占用 1 个机器字长 (Pointer)，相比标准 `Vec` 的 3 个字长 (Ptr, Len, Cap) 极大地节省了 `SignalData` 和 `EffectData` 的结构体体积。
*   **List<T>**：
    *   `Empty`: 零开销。
    *   `Single`: 内联存储一个元素，避免堆分配。
    *   `Many`: 使用 `ThinVec<T>` 存储多个元素。

### 4.4 核心数据结构细节

*   **SignalData**:
    *   `last_tracked_by`: 缓存最近一次追踪此 Signal 的 `(NodeId, Version)`。如果同一个 Effect 在同一轮计算中多次读取此 Signal，可以直接跳过后续的依赖注册过程。
*   **EffectData**:
    *   `effect_version`: 每次计算时递增。用于配合 `DependencyList` 中的版本号进行依赖变更检测。
    *   `dependencies`: 类型为 `List<(NodeId, u32)>`，存储依赖节点的 ID 及其当时的版本号。

### 4.5 高级原语实现

*   **Memo (计算属性)**：
    *   **架构**: Memo 节点是同时挂载了 `SignalData` 和 `EffectData` 组件的 Node。
    *   **Lazy Evaluation**: `Runtime` 能够识别这种双重身份。当作为 Signal 被读取时，触发 `update_if_necessary` (Evaluate)；当作为 Effect 被通知时，仅标记状态而不立即执行。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **线程安全性 (Thread Safety)**：目前的 Runtime 基于 `thread_local!`，仅支持单线程运行。
*   **API 易用性 (Ergonomics)**：计划结合更多的宏（macros）来提供自动解构、自动 Copy 等语法糖，减少样板代码。
*   **调试工具 (DevTools)**：开发可视化的依赖图调试工具，利用 NodeAux 中的 `debug_label`。
