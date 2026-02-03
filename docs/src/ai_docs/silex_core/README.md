# Crate: `silex_core`

**High-level, type-safe API for Silex application development.**

此 Crate 对底层的 `silex_reactivity` 进行了封装，引入了泛型 (`PhantomData`) 以提供编译时类型检查，并集成了常用的工具宏和错误处理机制。

## 模块: `reactivity` (响应式核心)

源码路径: `silex_core/src/reactivity.rs`

### 1. Trait System (特征系统)

`silex_core` 基于 Traits 构建了灵活的响应式接口。所有信号类型均实现了这些 Trait。

#### Metadata Traits
*   `DefinedAt`: `fn defined_at(&self) -> Option<&'static Location<'static>>`。调试辅助，提供信号定义的位置信息。
*   `debug_name(&self) -> Option<String>`。调试辅助，提供信号的语义化名称。
*   `IsDisposed`: `fn is_disposed(&self) -> bool`。检查信号是否已被销毁。

#### Access Traits (读访问) - **以 Zero-Copy 为核心**
*   **Core Primitives (核心原语)**:
    *   `WithUntracked`: `fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>`。**基础特征**。不追踪，通过闭包以**引用 (`&T`)** 方式访问值。这是实现零拷贝访问的基石。
    *   `Track`: `fn track(&self)`。显式追踪。将当前信号添加为依赖。
    *   `With`: `fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>`。**核心特征** (`WithUntracked` + `Track`)。自动追踪，通过闭包以**引用 (`&T`)** 方式访问值。
*   **Convenience Extensions (便利扩展 - 基于 Clone)**:
    *   `GetUntracked`: `fn try_get_untracked(&self) -> Option<Self::Value>`。(`WithUntracked` + `Clone`)。不追踪，直接 Clone 并返回值。仅当 `T: Clone` 时可用。**注意：避免在热路径上对大对象使用。**
    *   `Get`: `fn try_get(&self) -> Option<Self::Value>`。(`With` + `Clone`)。自动追踪，直接 Clone 并返回值。仅当 `T: Clone` 时可用。
*   **Derived**:
    *   `Map`: `fn map<U, F>(self, f: F) -> Derived<Self, F>`。基于 `With` 实现。从当前信号创建派生计算信号 `Derived`。闭包接受引用 `&T`，减少 `Clone` 开销。这是一个轻量级的惰性求值信号，不涉及 Memo 缓存开销。
    *   `Memoize`: `fn memo(self) -> Memo<Self::Value>`。将任意信号转换为 `Memo` 缓存信号。要求 `T: Clone + PartialEq`。
*   **Multi-Signal Access (多信号访问)**:
    *   `batch_read!(s1, s2 => |v1, v2| ...)`: 宏。允许同时以引用方式访问多个信号，实现零拷贝。
    *   `batch_read_untracked!(s1, s2 => |v1, v2| ...)`: 宏。同上，但不追踪依赖。

*   **Conversion Traits**:
    *   `IntoSignal`: `fn into_signal(self) -> Self::Signal`。用于将普通值或信号统一转换为特定的信号类型。常用于组件参数，使其既能接受 `T` (自动转为 `Constant<T>`) 也能接受 `ReadSignal<T>` / `Memo<T>` 等。

#### Update Traits (写更新)
*   `Notify`: `fn notify(&self)`。显式通知。触发 subscribers 更新。
*   `UpdateUntracked`: `fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>`。不通知，通过可变引用修改值。
*   `Update`: `fn try_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>`。修改值并自动通知。
*   `Set`: `fn try_set(&self, value: Self::Value) -> Option<Self::Value>`。直接替换值并自动通知。
*   `SignalSetter`: `fn setter(self, value: Self::Value) -> impl Fn() + Clone`。创建设置值的闭包。
*   `SignalUpdater`: `fn updater<F>(self, f: F) -> impl Fn() + Clone`。创建更新值的闭包。
*   `SetUntracked`: `fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value>`。不通知，直接替换值。

---

### 2. Signal Wrappers (信号包装器)

#### `Signal<T>`
*   **Enum**:
    *   `Read(ReadSignal<T>)`
    *   `Derived(NodeId, PhantomData<T>)`
    *   `StoredConstant(NodeId, PhantomData<T>)`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `IsDisposed`, `Track`, `WithUntracked`, `GetUntracked`, `With`, `Get`, `Map`.
*   **Semantics**:
    *   通用的信号接口，统一了 `ReadSignal`、派生计算和常量。
    *   `Derived` 变体持有一个在 Runtime 中注册的闭包，每次 `get()` 时重新执行闭包（无缓存）。
    *   `StoredConstant` 变体持有一个存储在 Runtime 中的常量值。
*   **Methods**:
    *   `derive(f: impl Fn() -> T)`: 创建一个派生信号。
    *   `get() -> T`: (via `Get` trait).
    *   `slice(getter: impl Fn(&T) -> &O)`: 创建一个指向内部字段的切片信号 `SignalSlice`，实现零拷贝访问。
*   **Conversions**:
    *   `From<T>`: 将普通值转换为 `Signal::StoredConstant`。
    *   `From<&str>`: 将字符串切片转换为 `Signal<String>` (StoredConstant)。
    *   `From<ReadSignal<T>>`, `From<RwSignal<T>>`, `From<Memo<T>>`: 转换为 `Signal::Read`。
*   **Operator Overloads**:
    *   实现了 `Add`, `Sub`, `Mul`, `Div`, `Rem`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Neg`, `Not`。
    *   支持 `Signal op Signal` 以及 `Signal op T`。
    *   所有运算均返回 `ReactiveBinary` (二元) 或 `Derived` (一元)，自动创建惰性派生计算，无额外缓存开销。
    *   **Lazy Evaluation (惰性求值)**:
        *   `ReactiveBinary` 和 `Derived` (用于 `Map`) 都是**无状态的 (Stateless)**。它们不缓存结果，每次被访问 (`try_with`) 时都会**重新执行**计算闭包。
        *   这对于轻量级操作（如比较 `eq`、简单算术 `add`）非常高效（Zero-Copy）。
        *   **Performance Trap (性能陷阱)**: 如果派生计算非常昂贵，这种惰性重算可能导致性能问题。对于昂贵的计算，请使用 `memo()` 或 `Signal::derive` 显式创建有状态的缓存节点。

#### `ReadSignal<T>`
*   **Struct**: `pub struct ReadSignal<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `IsDisposed`, `Track`, `WithUntracked`, `GetUntracked`, `With`, `Get`, `Map`.
*   **Methods**:
    *   `slice(getter: impl Fn(&T) -> &O)`: 创建一个指向内部字段的切片信号 `SignalSlice`，实现零拷贝访问。
*   **Fluent API**: 实现了 `eq`, `ne`, `gt`, `lt`, `ge`, `le`，返回 `ReactiveBinary<Self, O::Signal, ...>`。
*   **Operator Overloads**: 同 `Signal<T>`，支持所有基本运算符，返回 `ReactiveBinary<...>` 或 `Derived<...>`。

#### `WriteSignal<T>`
*   **Struct**: `pub struct WriteSignal<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `IsDisposed`, `Notify`, `UpdateUntracked`, `Update`, `Set`, `SignalSetter`, `SignalUpdater`.
*   **Methods**:
    *   `set(new_value: T)`: (via `Set` trait).
    *   `update(f: impl FnOnce(&mut T))`: (via `Update` trait).
    *   `set(new_value: T)`: (via `Set` trait).
    *   `update(f: impl FnOnce(&mut T))`: (via `Update` trait).
    *   `setter(value: T) -> impl Fn()`: (via `SignalSetter` trait).
    *   `updater(f: F) -> impl Fn()`: (via `SignalUpdater` trait).

#### `RwSignal<T>`
*   **Struct**: `pub struct RwSignal<T> { read: ReadSignal<T>, write: WriteSignal<T> }`
*   **Traits**: Implements all traits of `ReadSignal` and `WriteSignal`.
*   **Semantics**: 读写合一的信号句柄，常用于组件 Props。
    *   **Implements**: `SignalSetter`, `SignalUpdater`.
    *   **Methods**:
        *   `slice`: (继承自 `ReadSignal` 部分)。
*   **Operator Overloads**: 同 `Signal<T>`，支持所有基本运算符 (针对 Read 部分)，返回 `ReactiveBinary` / `Derived`。

#### `Memo<T>`
*   **Struct**: `pub struct Memo<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `IsDisposed`, `Track`, `WithUntracked`, `GetUntracked`, `With`, `Get`, `Map`.
*   **Semantics**: 派生计算信号。值被缓存，仅在依赖变更时无效。
*   **Methods**:
    *   `new(f: impl Fn(Option<&T>) -> T)`: 创建 Memo。
*   **Operator Overloads**: 同 `Signal<T>`，支持所有基本运算符，返回 `ReactiveBinary` / `Derived`。

#### `signal<T>`
*   **Signature**: `pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>)`
*   **Usage**: `let (count, set_count) = signal(0);`

#### `Constant<T>`
*   **Struct**: `pub struct Constant<T>(pub T)`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `IsDisposed`, `Track`, `WithUntracked`, `GetUntracked`, `With`, `Get`.
*   **Semantics**:
    *   一个极轻量级的包装器，直接持有值 `T`。
    *   实现了所有读取相关的 Signal Traits，但 `track` 它是空操作，永远不会导致重新渲染。
    *   它是 `IntoSignal` 对基本类型 (如 `bool`, `i32`, `String`) 的默认转换目标。
    *   **Use Case**: 当你需要一个满足 `Get<Value=T>` 接口但实际上永远不会改变的值时使用。

#### `IntoSignal` Trait
*   **Trait**: `pub trait IntoSignal { type Value; type Signal: Get<Value = Self::Value>; fn into_signal(self) -> Self::Signal; }`
*   **Semantics**:
    *   这是一个辅助 Trait，用于编写灵活的 API。
    *   为所有基本类型 (`u8`..`u128`, `i8`..`i128`, `f32`, `f64`, `bool`, `char`, `String`, `&str`) 实现了该 Trait，转换为 `Constant<T>`。
    *   为所有信号类型 (`Signal`, `ReadSignal`, `RwSignal`, `Memo`, `Constant`) 实现了该 Trait，转换为它们自己。
    *   **Tuples (元组)**: 为 2-4 个元素的元组（如 `(Signal<A>, Signal<B>)`）实现了该 Trait。它们会被转换为一个通过 `Signal::derive` 创建的 `Signal<(A, B)>`，从而将多个信号合并为一个组合信号。
    *   这允许组件接受 `impl IntoSignal<Value=T>`，从而既支持直接传值，也支持传信号。
*   **Performance Advice (性能建议)**:
    *   `Signal::from(value)` 会调用 `store_value(value)`，在响应式运行时中分配内存。这意味着即使是 `Signal::from(42)` 也会产生 Arena 分配开销。
    *   当组件参数接受 `impl IntoSignal` 时，如果传递的是静态值，**请直接传递该值**（例如 `42` 或 `Constant(42)`）。它是零分配的（Zero-Allocation）。
    *   仅在需要类型擦除（Type Erasure）时才使用 `Signal::from(value)`。

#### `StoredValue<T>`

*   **Struct**: `pub struct StoredValue<T> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: `Copy`, `Clone`, `Debug`, `DefinedAt`, `WithUntracked`, `GetUntracked`, `UpdateUntracked`, `SetUntracked`.
*   **Semantics**: 非响应式的数据存储容器。数据均存储在响应式运行时中，随宿主 Scope/Effect 自动释放。
*   **Use Case**: 存储不需要驱动 UI 更新的数据（如定时器句柄、大数据缓存），或在事件处理中进行无感知的状态修改。
*   **Methods**:
    *   `new(value: T) -> Self`: 创建存储值。
    *   `set_untracked(value: T)`: (via `SetUntracked`).
    *   `update_untracked(f: impl FnOnce(&mut T))`: (via `UpdateUntracked`).
    *   `with_untracked<U>(f: impl FnOnce(&T) -> U) -> U`: (via `WithUntracked`).
    *   `get_untracked() -> T`: (via `GetUntracked`).

### 3. Utilities

#### `batch`
*   **Signature**: `pub fn batch<R>(f: impl FnOnce() -> R) -> R`
*   **Semantics**: 延迟 Effect 的执行，直到闭包 `f` 结束。用于优化多次连续更新。


### 4. Async Resources (异步资源)

#### `Resource<T, E>`
*   **Struct**:
    ```rust
    pub enum ResourceState<T, E> {
        Idle,
        Loading,
        Ready(T),
        Reloading(T), // Stale-While-Revalidate
        Error(E),
    }

    pub struct Resource<T: 'static, E: 'static = SilexError> {
        pub state: ReadSignal<ResourceState<T, E>>,
        // internal: set_state, trigger
    }
    ```
*   **Methods**:
    *   `state.get()`: 获取当前完整的资源状态。
    *   `get_data() -> Option<T>`: 便捷方法，获取数据（无论是 `Ready` 还是 `Reloading`）。
    *   `refetch()`: 手动重新触发 `source` 变更，强制刷新。
    *   `update(f: impl FnOnce(&mut T))`: 手动修改本地缓存数据 (Optimistic UI)。
    *   `set(value: T)`: 直接设置本地缓存数据。
    *   `Resource` 依然实现 `Get<Value=Option<T>>` 特征，以便于与现有代码兼容。

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

### 5. Mutation (异步写入)

#### `Mutation<Arg, T, E>`

*   **Struct**:
    ```rust
    pub struct Mutation<Arg, T, E = SilexError> {
        pub state: ReadSignal<MutationState<T, E>>,
        // ...
    }
    
    pub enum MutationState<T, E> {
        /// Initial state
        Idle,
        /// Triggered and pending
        Pending,
        /// Last mutation successful
        Success(T),
        /// Last mutation failed
        Error(E),
    }
    ```
*   **Traits**: **`Copy`**, `Clone`.
    *   Mutation 本身是一个轻量级的句柄，内部通过 `StoredValue` 引用执行逻辑，因此可以像 Signal 一样廉价复制。
*   **Semantics**:
    *   用于执行写操作（如 POST/PUT 请求）。
    *   **手动触发**: 与 Resource 自动追踪依赖不同，Mutation 需要调用 `.mutate(arg)` 显式触发。
    *   **竞态处理**: 自动处理并发请求，采用 "Latest Wins" 策略（最后一次触发的请求结果生效，旧请求的返回被忽略）。
*   **Methods**:
    *   `new<F, Fut>(f: F)`: 创建 Mutation。
    *   `mutate(arg: Arg)`: 触发 Mutation。
    *   `loading() -> bool`: 快捷检查是否为 `Pending`。
    *   `value() -> Option<T>`: 获取最后一次成功的返回值。
    *   `error() -> Option<E>`: 获取最后一次失败的错误。

#### Usage Example

```rust
let login = Mutation::new(|(username, password)| async move {
    my_api::login(username, password).await
});

let on_submit = move |_| {
    login.mutate(("user".into(), "pass".into()));
};

view! {
    <button on:click=on_submit disabled=login.loading()>
        {move || if login.loading() { "Logging in..." } else { "Login" }}
    </button>
    {move || login.error().map(|e| view! { <div class="error">{format!("{:?}", e)}</div> })}
}
```

### 6. Context & Suspense

#### `provide_context`, `use_context`
*   直接重导出自 `silex_reactivity`，增加了 `SilexError` 相关的默认 Context 支持。

#### `expect_context<T>`
*   **Signature**: `pub fn expect_context<T: Clone + 'static>() -> T`
*   **Semantics**: 类似 `use_context`，但如果未找到 Context 会打印错误日志并 **Panic**。

#### `SuspenseContext`
*   **Struct**: `{ count: ReadSignal<usize>, set_count: WriteSignal<usize> }`
*   **Usage**: 用于追踪全局或局部的异步任务数量。

### 7. Lifecycle & Safety (生命周期与安全)

#### Safer Cleanup (更安全的清理)
*   `on_cleanup` 回调现在保证在 **子节点销毁之前** 执行。
*   这意味着在清理函数中，依然可以安全地访问当前作用域内创建的 `Signal`、`StoredValue` 或其他响应式状态。
*   此前（旧版本）清理执行顺序在子节点销毁之后，导致访问已销毁状态时 Panic。

#### Debugging Support (调试支持)
*   **Debug Labels**: 所有的 `Signal`, `Memo`, `StoredValue` 现在都支持 `.with_name("label")` 方法。
*   **Panic Messages**: 当尝试访问已销毁的信号时，Panic 信息会包含该信号的名称（如果有），极大地辅助定位问题。
    > "At locations..., you tried to access a reactive value 'DashboardTimer' which was defined at ..., but it has already been disposed."

---

## 模块: `callback`

源码路径: `silex_core/src/callback.rs`

### `Callback<T>`
*   **Struct**: `pub struct Callback<T = ()> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: **`Copy`**, `Clone`, `Debug`, `Default`.
*   **Semantics**: 使用 `NodeId` 句柄的轻量级回调包装器，闭包存储在响应式运行时。与 `Signal` 风格一致。
*   **Methods**:
    *   `new<F>(f: F) -> Self`: 创建回调。
    *   `call(&self, arg: T)`: 执行回调。
    *   `id(&self) -> NodeId`: 获取底层 ID。
    *   `impl From<F>`: 允许直接传入闭包转换。

---

## 模块: `node_ref`

源码路径: `silex_core/src/node_ref.rs`

### `NodeRef<T>`
*   **Struct**: `pub struct NodeRef<T = ()> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: **`Copy`**, `Clone`, `Debug`, `Default`.
*   **Semantics**: 使用 `NodeId` 句柄的轻量级 DOM 节点引用，元素存储在响应式运行时。
*   **Methods**:
    *   `new() -> Self`: 创建空引用。
    *   `get(&self) -> Option<T>`: 获取节点。如果未挂载或类型不匹配，返回 None。
    *   `load(&self, node: T)`: 内部使用，加载节点（由框架自动调用）。
    *   `id(&self) -> NodeId`: 获取底层 ID。
*   **Usage**:
    ```rust
    let input_ref = NodeRef::<HtmlInputElement>::new();
    input().node_ref(input_ref)  // 无需 .clone()，NodeRef 是 Copy 的
    ```

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
