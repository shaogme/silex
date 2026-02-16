# Silex Core 模块分析

## 1. 概要 (Overview)

*   **定义**：`silex_core` 是 Silex 框架的用户侧核心库，它在底层的 `silex_reactivity` 之上，提供了一套类型安全、符合人体工程学（Ergonomic）且高性能的响应式原语和工具集。
*   **作用**：它是连接底层响应式运行时（`silex_reactivity`）与上层应用逻辑（以及 `silex_dom`）的桥梁。它封装了裸指针和 `any` 类型，通过 Rust 强大的类型系统（Traits, Generics, PhantomData）保证了使用的安全性，并提供了 `Signal`, `Memo`, `Resource`, `Mutation` 等核心构建块。
*   **目标受众**：框架开发者、希望自定义响应式原语的高级用户。

## 2. 理念和思路 (Philosophy and Design)

### 核心思想：零拷贝优先 (Zero-Copy First)

`silex_core` 的设计核心在于**极度厌恶不必要的内存分配和拷贝**。

*   **以闭包访问为基础**：不同于传统的 "Getter 返回值" 模式（如 `fn get() -> T`），Silex 极其推崇 "闭包访问" 模式（`fn with(|val| ...)`）。
    *   在底层，信号的值存储在 Arena 或 `RefCell` 中。要获取 `T`，通常需要持有锁。
    *   如果直接返回 `T`，对于 `String` 或 `Vec` 等大对象，必须进行 `Check`。
    *   通过 `With` 特征 (`try_with(|val: &T| ...)`)，我们可以直接把内部数据的引用传递给用户闭包，实现 **Zero-Copy**。
*   **元组非信号 (Tuples are not Signals)**：为了贯彻零拷贝，`silex_core` 明确拒绝将 `(Signal<A>, Signal<B>)` 视为一个可以直接 `with` 的组合信号。因为 A 和 B 存储在内存的不同位置，无法同时给出一个 `&(A, B)` 的引用。为此，我们设计了 `batch_read!` 宏来显式处理多信号的零拷贝访问。

### 统一的特征系统 (Unified Trait System)

为了让 API 既灵活又统一，我们设计了一套庞大的 Trait 系统（见 `traits.rs`）。

*   **组合优于继承**：功能被拆分为原子化的 Traits：`Track`（追踪）、`Notify`（通知）、`WithUntracked`（访问）、`UpdateUntracked`（修改）。
*   **自动推导**：高级功能由低级功能自动组合而成。例如，只要实现了 `WithUntracked` + `Track`，类型就自动获得了 `With`（自动追踪访问）和 `Map`（派生）的能力。
*   **灵活性**：这允许 `Constant<T>`（常量）、`Derived`（计算属性）、`ReadSignal`（读信号）虽然底层实现完全不同，但对外仅仅表现为“可读取的响应式数据”。

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
    *   **警示**：文档中多次强调避免在热路径对大对象使用 `Get`。

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
    StoredConstant(NodeId, PhantomData<T>), // 存储的常量
}
```

*   **Derived 变体**：这是一个无缓存的计算属性。当你写 `signal_a + signal_b` 时，返回的就是一个 `Signal::Derived`。它没有专门的存储空间，每次访问都重新运行底层的闭包。
*   **StoredConstant**：这是一个优化。对于 `signal(42)` 或者常量配置，我们不需要追踪机制，但为了接口统一，我们把它包装起来。

### 4.3. 宏魔法 (`macros`)

*   **`rx!`**：
    *   极其简单：`macro_rules! rx { ($($expr:tt)*) => { move || { $($expr)* } }; }`
    *   作用：仅仅是为了少写 `move ||`，让代码看起来更像声明式公式。
*   **`batch_read!`**：
    *   解决了“多信号零拷贝”的难题。
    *   原理：将回调嵌套。
    *   `batch_read!(a, b => |ref_a, ref_b| ...)` 展开为 `a.with(|ref_a| b.with(|ref_b| ...))`。

### 4.4. 异步原语 (Resource & Mutation)

*   **Resource**: 将 `async` 读取转换为同步的 `Signal`。利用了 `SuspenseContext` 进行全局加载状态管理（与 SSR 集成）。
*   **Mutation**: 处理 `async` 写入。不仅仅是 `async` 函数的包装，还解决了**竞态条件 (Race Conditions)** —— 如果连续触发三次 Mutation，它保证只有最后一次的结果会生效 (Latest Wins)，这在表单提交和搜索建议中至关重要。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **`IntoSignal` 内存优化**:
    *   目前 `IntoSignal` 对基本类型（如 `i32`）的转换会占用 Arena 槽位。计划优化 `Constant` 的存储方式，尝试直接内联存储小数据，减少内存分配。
*   **Type Erasure 开销优化**:
    *   虽然后端 `Signal<T>` 枚举分发开销很小，但在极端性能敏感场景下，仍需探索减少 match 分发的途径。
*   **错误处理机制改进**:
    *   `expect_context` 目前直接 panic。未来计划提供更友好的错误恢复机制或错误边界集成，提供更详细的调试信息。
