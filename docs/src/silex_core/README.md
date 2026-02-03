# Silex Core 核心库

`silex_core` 是面向开发者的上层 API 库。它封装了底层的 `silex_reactivity` 引擎，提供了强类型的接口和常用的工具集。

## 模块概览

### 1. Reactivity (响应式系统)

该模块在 `silex_reactivity` 的基础上提供了类型安全的包装器。

*   **SignalWrapper (通用信号)**:
    *   `Signal<T>`: 统一的信号包装器，**实现了 `Copy`**。
    *   它可以包装 `ReadSignal`, `RwSignal`, `Memo`，`Derived` (派生闭包) 或 `Constant` (常量)。
    *   作为组件 Props 的首选类型，因为它能接受任何类型的响应式数据源（包括普通值，会自动转换为常量信号）。

*   **Trait System (特征系统) - Zero-Copy First**:
    *   Silex 的特征系统设计以 **`With` (闭包访问)** 为绝对核心，旨在避免不必要的内存克隆。
    *   **核心 (Core)**: 
        *   `With`: `fn with(|v| ...)`。通过引用访问值并追踪。这是最基础的原语。
        *   `WithUntracked`: `fn with_untracked(|v| ...)`。通过引用访问值但不追踪。
    *   **便利 (Convenience)**:
        *   `Get`: `fn get()`。`With` + `Clone` 的语法糖。仅对 `Clone` 类型可用。应避免对大对象使用。
        *   `GetUntracked`: `fn get_untracked()`。`WithUntracked` + `Clone` 的语法糖。
    *   **派生**: `Map` (基于 `With` 的引用映射，零开销)。
    *   **多信号访问**: 
        *   `batch_read!(s1, s2 => |v1, v2| ...)`: 同时访问多个信号并追踪，零 Clone。
        *   `batch_read_untracked!(s1, s2 => |v1, v2| ...)`: 同时访问多个信号不追踪，零 Clone。
    *   **写**: `Set` (设置并通知), `SetUntracked` (设置不通知), `Update` (修改并通知), `SignalSetter` (生成 setter), `SignalUpdater` (生成 updater)。
    *   **转换**: `IntoSignal` (值转信号)。允许组件 Props 接受 `impl IntoSignal`，从而同时支持静态值（自动转为 `Constant`，零分配）和动态信号。也支持将元组 `(Signal<A>, Signal<B>)` 转换为组合信号 `Signal<(A, B)>`。
    *   **性能建议**: `Signal::from(val)` 会在运行时分配内存。若传递静态值，请直接使用 `val` 或 `Constant(val)` 以避免不必要的开销。仅在需要特定类型 `Signal<T>` 时才使用 `Signal::from`。
    *   这种设计使得你可以灵活组合不同的行为，例如 `StoredValue` 实现了 `GetUntracked`/`SetUntracked` 但不实现 `Track`/`Notify`。

*   **Primitive Signals (基础信号)**: 
    *   `ReadSignal<T>`: 只读信号句柄，实现了 `Get`, `GetUntracked` 等读取特征。
    *   `WriteSignal<T>`: 可写信号句柄，实现了 `Set`, `Update`, `SignalSetter`, `SignalUpdater` 等写入特征。
    *   `RwSignal<T>`: 读写一体的信号句柄，常用于组件 `Props`。
    *   `Memo<T>`: 派生计算缓存，实现了 `Map` 等读取特征。
    *   `Constant<T>`: 常量包装器，直接持有值。实现了 `Get` 等特征但无开销。是 `IntoSignal` 对字面量的默认转换结果。

    *   **Slice (切片)**:
        *   所有信号都支持 `.slice(|v| &v.field)` 方法。
        *   返回一个 `SignalSlice`，它持有源信号和投影函数。
        *   允许以**引用方式**访问大结构体的字段，实现**零拷贝**读取，极大优化了 `Vec` 或复杂 Struct 的访问性能。
    *   **Lazy Evaluation (惰性求值)**:
        *   `Map`, 比较操作 (`eq`, `gt`...), 算术运算 (`add`, `sub`...) 均返回 `Derived` 或 `ReactiveBinary`。
        *   这些结构体是零开销的 **无状态 (Stateless)** 计算单元。每次被访问时都会重新执行闭包，**不** 缓存结果。
        *   对于昂贵的计算，请务必使用 `.memo()` 或 `Signal::derive` 来创建有状态的缓存节点，以免影响性能。

*   **Effect (副作用)**:
    *   `Effect`: 创建自动追踪依赖的副作用，使用 `Effect::new`。

*   **Batching (批量更新)**:
    *   `batch`: 一个性能优化工具。在 `batch` 闭包内的所有信号更新，直到闭包执行完毕后才会触发 Effect。
    *   适用于一次性修改多个状态，避免中间态导致的无效渲染。

    ```rust
    // 假设 count 和 double 是相关联的信号
    batch(|| {
        set_count.update(|n| *n += 1);
        set_double.update(|n| *n = (*n) * 2);
    }); // 此时才会触发 Effect
    ```

*   **Resource (异步资源)**:
    *   `resource`: 用于处理异步数据加载（如 API 请求）。
    *   **State-based**: 采用单一来源的 `state` 枚举 (`Idle`, `Loading`, `Ready(T)`, `Reloading(T)`, `Error(E)`)。
    *   **Stale-While-Revalidate**: 当 `refetch` 时，状态会变为 `Reloading(old_data)`，UI 可据此决定是显示 Skeleton 还是仅显示顶部进度条。
    *   集成 `Suspense` 支持。
    *   支持 `refetch` 手动刷新。
    *   支持 `update` / `set` 手动修改本地数据（Optimistic UI）。

*   **Mutation (异步写入)**:
    *   `Mutation<Arg, T, E>`: 用于处理数据变更请求（如 POST/PUT）。
    *   **Manual Trigger**: 不同于 Resource，它不追踪依赖，必须通过 `.mutate(arg)` 手动触发。
    *   **Race Handling**: 自动处理并发，只保留最后一次请求的结果 (Latest Wins)。
    *   实现了 `Copy`，轻量级句柄。

*   **Context (上下文)**:
    *   `provide_context` / `use_context`: 基于类型 ID 的依赖注入机制，支持跨组件数据传递。
    *   `expect_context`: 严格版 `use_context`，未找到时会 Panic。

*   **StoredValue (存储值)**:
    *   `StoredValue<T>`: 非响应式数据容器。
    *   数据存储在运行时中，句柄实现 `Copy`。
    *   **特点**: 读写**不触发**任何 UI 更新。
    *   **优势**: 支持 `with_untracked` 以**引用**方式访问数据，适合存储复杂结构或不需渲染的内部状态。

### 2. Callback (回调)

*   **Callback<T>**: 一个轻量级的回调句柄，**实现了 `Copy`**。
*   闭包存储在响应式运行时中，`Callback` 只持有一个 `NodeId`。
*   用于在组件间传递事件处理函数，可以像 `Signal` 一样直接复制。

### 3. NodeRef (DOM 引用)

*   **NodeRef<T>**: 用于获取底层 DOM 节点的引用句柄，**实现了 `Copy`**。
*   当需要调用命令式 DOM API（如 `.focus()`, `.showModal()`, Canvas 绘图）时使用。
*   节点引用存储在响应式运行时中，`NodeRef` 只持有一个 `NodeId`。

```rust
use web_sys::HtmlInputElement;

let input_ref = NodeRef::<HtmlInputElement>::new();

input()
    .node_ref(input_ref)  // 无需 .clone()，NodeRef 是 Copy 的
    .on_click(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    })
```

### 4. Error Handling (错误处理)

*   **SilexError**: 统一的错误枚举，包含 `Dom`, `Reactivity`, `Javascript` 等变体。
*   **ErrorBoundary**: 提供了错误捕获机制，通过 `ErrorContext` 向上传递错误。

### 5. Logging (日志)

提供了同构的日志宏，自动适配浏览器控制台 (`console.log`) 和终端标准输出 (`println!`)。

*   `log!(...)`
*   `warn!(...)`
*   `error!(...)`
*   以及对应的 `debug_*` 变体。

### 6. Debugging (调试增强)

Silex 提供了强大的工具来帮助排查和避免响应式问题。

*   **Named Signals (命名信号)**:
    所有的 `Signal`, `Memo`, `StoredValue` 句柄都支持 `.with_name("MyLabel")`。
    
    ```rust
    let (count, set_count) = signal(0);
    count.with_name("Counter"); 
    ```

    当 Debug 模式下 Panic 时，报错会指出信号名称：
    > "Tried to access a reactive value **'Counter'** but it has already been disposed."

*   **Safe Cleanup (安全清理)**:
    `on_cleanup` 回调保证在作用域销毁**开始时**就执行。即使作用域即将结束，您依然可以在清理函数中读取 Signal 的最后状态。

## 最佳实践

### 信号读写分离
推荐使用 `(ReadSignal, WriteSignal)` 的元组解构形式创建信号，以明确读写权限。

```rust
let (count, set_count) = signal(0);
```

### 句柄类型的 `Copy` 特性
Silex 的信号句柄 (`ReadSignal`, `RwSignal`)、回调 (`Callback`) 和 DOM 引用 (`NodeRef`) 都实现了 `Copy`。这意味着它们只是指向底层数据的“指针”，复制它们非常廉价。

```rust
let input_ref = NodeRef::<HtmlInputElement>::new();
let cb = Callback::new(|x: i32| log!("{}", x));

// 直接复制，无需 .clone()
let ref2 = input_ref;
let cb2 = cb;
```

### 异步数据获取
使用 `Resource` 而不是在 `Effect` 中手动 spawn 异步任务，以便更好地与 `Suspense` 集成和处理竞态条件。
请利用 `ResourceState` 枚举来处理不同的 UI 状态（如 `Reloading` vs `Loading`）。
