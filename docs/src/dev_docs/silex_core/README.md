# Silex Core 模块分析

## 1. 概要 (Overview)

*   **定义**：`silex_core` 是 Silex 框架的用户侧核心库，它在底层的 `silex_reactivity` 之上，提供了一套类型安全、符合人体工程学（Ergonomic）且高性能的响应式原语和工具集。
*   **作用**：它是连接底层响应式运行时（`silex_reactivity`）与上层应用逻辑（以及 `silex_dom`）的桥梁。它封装了裸指针和 `any` 类型，通过 Rust 强大的类型系统（Traits, Generics, PhantomData）保证了使用的安全性，并提供了 `Signal`, `Memo`, `Resource`, `Mutation` 等核心构建块。
*   **目标受众**：框架开发者、希望自定义响应式原语的高级用户。

## 2. 理念和思路 (Philosophy and Design)

### 核心思想：零拷贝优先 & Rx 委托 (Zero-Copy & Rx Delegate)

`silex_core` 的设计建立在两个核心支柱之上：

*   **以闭包访问为基础 (Zero-Copy)**：不同于传统的 "Getter 返回值" 模式（如 `fn get() -> T`），Silex 极其推崇 "闭包访问" 模式（`fn with(|val| ...)`）。
    *   在底层，信号的值存储在 Arena 中。获取 `T` 通常涉及获取锁。
    *   通过 `With` 特征 (`try_with(|val: &T| ...)`)，可以直接把内部数据的引用传递给用户闭包，实现 **Zero-Copy**。
*   **Rx 委托模式 (Rx Delegate)**：现代 Silex 采用委托模式。所有的响应式操作（算术、比较、映射等）不再直接实现在每个信号类型上，而是通过 [`IntoRx`] 统一转换为 [`Rx`] 包装器。
    *   [`Rx`] 作为所有响应式计算的对外接口，极大地减少了泛型膨胀和重复代码。
    *   通过 [`RxInternal`] 隐藏内部实现细节，保持 API 的整洁。
*   **元组支持 (Tuples support)**：元组（如 `(Signal<A>, Signal<B>)`）现在通过 [`IntoRx`] 自动支持转换为组合 `Rx`。虽然这涉及克隆来构建结果元组，但它提供了极佳的组合灵活性。对于追求性能的场景，仍推荐使用 [`batch_read!`] 宏。

### 统一的特征系统 (Unified Trait System)

为了让 API 既灵活又统一，我们设计了一套庞大的 Trait 系统（见 `traits.rs`）。

*   **组合优于继承**：功能被拆分为原子化的 Traits：`Track`（追踪）、`Notify`（通知）、`WithUntracked`（访问）、`UpdateUntracked`（修改）。
*   **自动推导**：高级功能由低级功能自动组合而成。例如，只要实现了 `WithUntracked` + `Track`，类型就自动获得了 `With`（自动追踪访问）和 `Map`（派生）的能力。
*   **灵活性**：这允许 `Constant<T>`（常量）、`DerivedPayload`（派生）、`ReadSignal`（读信号）虽然底层实现完全不同，但对外仅仅表现为“可读取的响应式数据”。

## 3. 模块内结构 (Internal Structure)

`silex_core` 的代码组织如下：

```text
silex_core/src/
├── lib.rs              // 导出与宏定义 (rx!, batch_read!)
├── traits.rs           // 核心特征定义 (With, Track, Update 等)
├── reactivity.rs       // 响应式模块重导出与胶水代码
├── reactivity/
│   ├── signal.rs       // Signal, ReadSignal, WriteSignal, Constant 实现
│   ├── memo.rs         // Memo (缓存计算) 实现
│   ├── effect.rs       // Effect (副作用) 实现
│   ├── resource.rs     // Resource (异步读) 实现
│   ├── mutation.rs     // Mutation (异步写) 实现
│   └── slice.rs        // SignalSlice (切片) 实现
├── callback.rs         // Callback 封装
├── node_ref.rs         // NodeRef DOM 引用封装
├── error.rs            // SilexError 错误处理类型
└── log.rs              // 同构日志宏 (log!, warn!, error!)
```

### 核心组件关系

*   **Signal Wrappers** (`ReadSignal`, `WriteSignal` 等) 仅仅是 **New Type Wrapper**。它们内部只包含 `NodeId` (即 `u32` 索引) 和 `PhantomData<T>`。
*   **数据流向**：
    1.  用户操作 `WriteSignal`。
    2.  `silex_core` 将操作转发给 `silex_reactivity` 运行时。
    3.  运行时更新 Arena 中的数据，触发依赖更新。
    4.  `ReadSignal` 及其派生的 `Memo` 收到通知。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1. 泛型 Trait 系统 (`traits.rs`)

这是本 crate 最复杂也是最精彩的部分。

#### 访问层级 (Access Hierarchy)

1.  **`WithUntracked` (基石)**: `fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U>`
    *   这是所有读取操作的源头。它要求实现者提供对内部数据的**不可变引用**。
    *   绝大多数信号（`ReadSignal`, `Constant`, `Memo`）都通过实现此 Trait 来暴露数据。
2.  **`With` (自动追踪)**: `WithUntracked` + `Track`。
    *   只要能无追踪访问且能追踪依赖，就能实现“响应式访问”。
    *   默认实现：先调用 `self.track()`，再调用 `self.try_with_untracked(f)`。
3.  **`Get` / `GetUntracked` (便利性扩展)**:
    *   仅当 `T: Clone` 时可用。
    *   本质上就是 `self.with(Clone::clone)`。
    *   **警示**：避免在热路径对大对象使用 `Get`。
4.  **`IntoRx` (大一统接口)**:
    *   允许将常量、信号、闭包及元组转化为统一的 `Rx`。
    *   提供了 `is_constant()` 检测，允许框架进行激进的静态优化。
5.  **`RxInternal` (委托原语)**:
    *   隐藏的底层 Trait，定义了 `Rx` 包装器如何与实际数据交互。

#### 修改层级 (Mutation Hierarchy)

1.  **`UpdateUntracked` (基石)**: 提供 `&mut T` 访问。
2.  **`Update` (自动通知)**: `UpdateUntracked` + `Notify`。先修改，后通知。
3.  **`Set`**: 基于 `Update` 实现，直接 `*val = new_val`。

### 4.2. 信号枚举与其统一 (`signal.rs`)

为了支持如 `IntoSignal` 这样的多态参数，`Signal<T>` 被设计为一个枚举：

```rust
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),                    // 普通读信号
    Derived(NodeId, PhantomData<T>),        // 派生信号（闭包）
    StoredConstant(NodeId, PhantomData<T>), // 存储的常量 (Arena)
    InlineConstant(u64, PhantomData<T>),    // 内联常量 (Small Copy Types)
}
```

*   **Rx Delegate**: 所有的运算符实现 (`+`, `-`, `*`, `/` 等) 以及逻辑比较 (`equals`, `greater_than` 等) 均不在 `Signal` 枚举上直接实现，而是先调用 `.into_rx()`。这大幅降低了代码生成的复杂度（见 `traits/impls.rs`）。
*   **InlineConstant**: 这是一个**极致优化**。对于 `i32`, `f64`, `bool` 等小型的 `Copy` 类型，我们将值直接**内联存储**在 `Signal` 枚举的变体中（通过 unsafe 位拷贝），从而**完全消除了 Arena 内存分配**。这意味着 `Signal::from(42)` 现在是零分配的。
*   **is_constant()**: `Signal` 提供此方法用于在运行时快速检测其是否为常量。这对于 `silex_dom` 等上层库非常有用，可以据此决定是否需要为该值挂载监听器。

### 4.3. 宏魔法 (`macros`)

*   **`rx!`**：
    *   极其简单：`macro_rules! rx { ($($expr:tt)*) => { move || { $($expr)* } }; }`
    *   作用：仅仅是为了少写 `move ||`，让代码看起来更像声明式公式。
*   **`batch_read!`**：
    *   解决了“多信号零拷贝”的难题。
    *   原理：将回调嵌套。
    *   `batch_read!(a, b => |ref_a, ref_b| ...)` 展开为 `a.with(|ref_a| b.with(|ref_b| ...))`。

*   **`Rx<F, M>` 包装器**：
    *   `Rx` 是闭包的薄包装，通过 `PhantomData<M>` 区分 `RxValue` (计算单元) 和 `RxEffect` (事件处理器)。
    *   它实现了 `WithUntracked`, `Track`, `DefinedAt` 等 Trait，使其可以直接参与响应式运算。
    *   **运算符重载** (`+`, `-`, `==` 等) 均返回 `Rx<DerivedPayload, RxValue>`。

### 4.4. 异步原语 (Resource & Mutation)

*   **Resource**: 将 `async` 读取转换为同步的 `Signal`。利用了 `SuspenseContext` 进行全局加载状态管理（与 SSR 集成）。
*   **Mutation**: 处理 `async` 写入。不仅仅是 `async` 函数的包装，还解决了**竞态条件 (Race Conditions)** —— 如果连续触发三次 Mutation，它保证只有最后一次的结果会生效 (Latest Wins)，这在表单提交和搜索建议中至关重要。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **`IntoSignal` 内存优化** (已完成):
    *   `IntoSignal` 对基本类型（如 `i32`）的转换现在使用 `InlineConstant`，直接内联存储小数据，消除了内存分配。
*   **Type Erasure 开销优化**:
    *   虽然后端 `Signal<T>` 枚举分发开销很小，但在极端性能敏感场景下，仍需探索减少 match 分发的途径。
*   **错误处理机制改进**:
    *   `expect_context` 目前直接 panic。未来计划提供更友好的错误恢复机制或错误边界集成，提供更详细的调试信息。
