# Silex Core 核心库

`silex_core` 是面向开发者的上层 API 库。它封装了底层的 `silex_reactivity` 引擎，提供了强类型的接口和常用的工具集。

## 模块概览

### 1. Reactivity (响应式系统)

该模块在 `silex_reactivity` 的基础上提供了类型安全的包装器。

*   **Signal (信号)**: 
    *   `ReadSignal<T>`: 只读信号句柄。
    *   `WriteSignal<T>`: 可写信号句柄。
    *   `RwSignal<T>`: 读写一体的信号句柄，常用于组件 `Props`。
    *   使用 `signal` 创建，利用 `PhantomData<T>` 保留类型信息，并在运行时通过 `downcast` 安全转换 `Any` 数据。

*   **Effect (副作用)**:
    *   `effect`: 创建自动追踪依赖的副作用。

*   **Resource (异步资源)**:
    *   `resource`: 用于处理异步数据加载（如 API 请求）。
    *   集成 `Suspense` 支持，自动管理 `loading`、`data` 和 `error` 状态。
    *   支持 `refetch` 手动刷新。

*   **Context (上下文)**:
    *   `provide_context` / `use_context`: 基于类型 ID 的依赖注入机制，支持跨组件数据传递。
    *   `expect_context`: 严格版 `use_context`，未找到时会 Panic。

### 2. Callback (回调)

*   **Callback<T>**: 一个简单的 `Rc<dyn Fn(T)>` 包装器。
*   用于在组件间传递事件处理函数，实现了 `Clone` 和 `PartialEq`（基于指针，待定），方便作为 Props 传递。

### 3. Error Handling (错误处理)

*   **SilexError**: 统一的错误枚举，包含 `Dom`, `Reactivity`, `Javascript` 等变体。
*   **ErrorBoundary**: 提供了错误捕获机制，通过 `ErrorContext` 向上传递错误。

### 4. Logging (日志)

提供了同构的日志宏，自动适配浏览器控制台 (`console.log`) 和终端标准输出 (`println!`)。

*   `log!(...)`
*   `warn!(...)`
*   `error!(...)`
*   以及对应的 `debug_*` 变体。

## 最佳实践

### 信号读写分离
推荐使用 `(ReadSignal, WriteSignal)` 的元组解构形式创建信号，以明确读写权限。

```rust
let (count, set_count) = signal(0);
```

### 避免 `Copy` 陷阱
Silex 的信号句柄 (`ReadSignal`, `RwSignal`) 都实现了 `Copy`。这意味着它们只是指向底层数据的“指针”，复制它们非常廉价。

### 异步数据获取
使用 `resource` 而不是在 `effect` 中手动 spawn 异步任务，以便更好地与 `Suspense` 集成和处理竞态条件。
