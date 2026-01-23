# Crate: `silex_core`

**High-level, type-safe API for Silex application development.**

此 Crate 对底层的 `silex_reactivity` 进行了封装，引入了泛型 (`PhantomData`) 以提供编译时类型检查，并集成了常用的工具宏和错误处理机制。

## 模块: `reactivity` (响应式核心)

源码路径: `silex_core/src/reactivity.rs`

### 1. Signal Wrappers (信号包装器)

#### `ReadSignal<T>`
*   **Struct**: `pub struct ReadSignal<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`, `Accessor<T>`.
*   **Methods**:
    *   `get() -> T`: 追踪并获取值 (Panic if dropped)。
    *   `try_get() -> Option<T>`: 追踪并尝试获取值。
    *   `get_untracked() -> T`: 不追踪获取 (Panic if dropped)。
    *   `map<U>(self, f: F) -> ReadSignal<U>`: 创建一个派生信号 (Memo)。
*   **Fluent API**: 实现了 `eq`, `ne`, `gt`, `lt`, `ge`, `le`，直接返回 `ReadSignal<bool>`。

#### `WriteSignal<T>`
*   **Struct**: `pub struct WriteSignal<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`.
*   **Methods**:
    *   `set(new_value: T)`: 更新信号值。
    *   `update(f: impl FnOnce(&mut T))`: 通过闭包修改值。
    *   `setter(value: T) -> impl Fn()`: 返回一个设置值的闭包 (用于事件绑定)。
    *   `updater(f: F) -> impl Fn()`: 返回一个更新值的闭包。

#### `RwSignal<T>`
*   **Struct**: `pub struct RwSignal<T> { read: ReadSignal<T>, write: WriteSignal<T> }`
*   **Semantics**: 读写合一的信号句柄，常用于组件 Props。
*   **Methods**: 代理了 `ReadSignal` 和 `WriteSignal` 的所有方法 (`get`, `set`, `update`, etc.)。

#### `signal<T>`
*   **Signature**: `pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>)`
*   **Usage**: `let (count, set_count) = signal(0);`


### 2. Async Resources (异步资源)

#### `Resource<T, E>`
*   **Struct**:
    ```rust
    pub struct Resource<T: 'static, E: 'static = SilexError> {
        pub data: ReadSignal<Option<T>>,
        pub error: ReadSignal<Option<E>>,
        pub loading: ReadSignal<bool>,
        trigger: WriteSignal<usize>, // Internal
    }
    ```
*   **Methods**:
    *   `get() -> Option<T>`: 获取数据。如果存在 Error，会自动上报到最近的 `ErrorContext`。
    *   `loading() -> bool`: 获取加载状态。
    *   `refetch()`: 手动重新触发 `source` 变更，强制刷新。

#### `Resource::new<S, Fetcher>`
*   **Signature**:
    ```rust
    pub fn new<S, Fetcher>(
        source: impl Fn() -> S + 'static,
        fetcher: Fetcher,
    ) -> Self
    ```
*   **Semantics**:
    1.  监听 `source` 闭包的变化。
    2.  当 `source` 变化时，自增 `request_id` 并调用 `fetcher`。
    3.  集成了 `SuspenseContext`：请求开始时 `increment`，结束时 `decrement`。
    4.  处理竞态条件：丢弃旧 ID 的返回结果。

### 3. Context & Suspense

#### `provide_context`, `use_context`
*   直接重导出自 `silex_reactivity`，增加了 `SilexError` 相关的默认 Context 支持。

#### `expect_context<T>`
*   **Signature**: `pub fn expect_context<T: Clone + 'static>() -> T`
*   **Semantics**: 类似 `use_context`，但如果未找到 Context 会打印错误日志并 **Panic**。

#### `SuspenseContext`
*   **Struct**: `{ count: ReadSignal<usize>, set_count: WriteSignal<usize> }`
*   **Usage**: 用于追踪全局或局部的异步任务数量。

---

## 模块: `callback`

源码路径: `silex_core/src/callback.rs`

### `Callback<T>`
*   **Struct**: `pub struct Callback<T = ()> { f: Rc<dyn Fn(T)> }`
*   **Semantics**: 一个可克隆的闭包包装器，用于组件间传递事件回调。
*   **Methods**:
    *   `call(&self, arg: T)`: 执行回调。
    *   `impl From<F>`: 允许直接传入闭包转换。

---

## 模块: `error`

源码路径: `silex_core/src/error.rs`

### `SilexError`
*   **Enum**:
    *   `Dom(String)`
    *   `Reactivity(String)`
    *   `Javascript(String)`
*   **Traits**: Implements `std::error::Error`.

### `ErrorContext`
*   **Struct**: `pub struct ErrorContext(pub Rc<dyn Fn(SilexError)>)`
*   **Semantics**: 错误处理的上报通道，通常由 `<ErrorBoundary>` 组件提供。

### `handle_error`
*   **Signature**: `pub fn handle_error(err: SilexError)`
*   **Logic**: 尝试获取 `ErrorContext` 并调用；若无 Context，则降级打印到控制台。

---

## 模块: `log`

源码路径: `silex_core/src/log.rs`

### Macros
*   `log!($($t:tt)*)`: 类似于 `println!`，输出普通日志。
*   `warn!($($t:tt)*)`: 输出警告。
*   `error!($($t:tt)*)`: 输出错误。
*   `debug_log!`, `debug_warn!`, `debug_error!`: 仅在 `debug_assertions` 开启时输出。

### Platform Support
*   **Browser (wasm32)**: 调用 `web_sys::console::log_1` 等 API。
*   **Native / Testing**: 调用标准 `println!` / `eprintln!`。

---

## 宏 (Macros)

### `rx!`
*   **Definition**: `macro_rules! rx { ($($expr:tt)*) => { move || { $($expr)* } }; }`
*   **Usage**: `let doubled = rx!(count.get() * 2);`
*   **Semantics**: 语法糖，用于快速创建 `move ||` 闭包，常用于 Signals 的派生计算或属性绑定。
