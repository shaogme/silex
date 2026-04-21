# Crate: silex

`silex` 是框架的主入口 Crate (Facade)，重新导出了核心组件并提供了上层抽象（Router, Flow Control, UI Components）。

## 0. 功能特性 (Feature Flags)

*   `macros` (default): 启用过程宏支持。
*   `persistence`: 启用 `silex::persist` 模块。
*   `json`: 启用 `JsonCodec` 支持（依赖 `persistence`, `serde`, `serde_json`）。
*   `net`: 启用网络通信支持 (`silex::net`)。

## 1. 核心导出 (Core Exports)

*   `silex::prelude`: 包含所有常用 Traits, MACROS 和 Types。
*   `silex::reexports`: 重新导出 `js_sys`, `web_sys`, `wasm_bindgen` 等底层依赖，确保版本一致性。
*   `silex::core`: 重新导出 `silex_core`。
*   `silex::dom`: 重新导出 `silex_dom`。
*   `silex::html`: 重新导出 `silex_html`。

## 2. Router 系统 (silex::router)

基于 History API 和 Reactivity System 的客户端路由。

### 架构图解
*   **Source of Truth**: `window.location` (URL)。
*   **State Management**: `RouterContext` (包含 `path`, `search` 信号)。
*   **Sync Mechanism**: `popstate` 事件监听 + `history.pushState` 调用。
*   **Matching**: 字符串前缀匹配 (Router) 或 Enum 强类型匹配 (Routable)。
*   **Query Persistence**: URL query 不再通过单独 hook 暴露，而是通过 `silex::persist::QueryBackend` 接入统一 persist builder。

### RouterContext
`silex/src/router/context.rs`
存储路由全局状态，通过 `provide_context` 在 `Router` 组件根部注入。

| Field | Type | Description |
| :--- | :--- | :--- |
| `base_path` | `String` | 应用的基础路径 (e.g. `/app`)，所有路由匹配基于此剥离。 |
| `path` | `ReadSignal<String>` | 当前逻辑路径 (不含 base_path)。 |
| `search` | `ReadSignal<String>` | 当前查询字符串 (含 `?`)。 |
| `navigator` | `Navigator` | 封装了 `push`, `replace` 方法的控制器。 |

#### Navigator
`silex/src/router/context.rs` -> `struct Navigator`
*   **push(url)**: 调用 `history.pushState` 并更新 Context 信号。
*   **replace(url)**: 调用 `history.replaceState` 并更新 Context 信号。
*   **set_query(key, value)**: 原子化更新查询参数。读取 -> 解析 -> 修改 -> Push。
*   **Side Effects**: 直接操作 DOM History API，触发 `popstate` (模拟)。

## 2.1 Persistence 系统 (silex::persist)

`silex/src/persist/*`

统一的外部状态绑定层，覆盖 `localStorage`、`sessionStorage` 与 URL query。

### 核心入口
*   `Persistent::builder(key)` -> `PersistentBuilder<...>`
*   Backend 选择：`.local()` / `.session()` / `.query()`
*   Codec 选择：`.string()` / `.parse::<T>()` / `.json::<T>()`
*   构建结果：`Persistent<T>`

### Persistent<T>
*   内部持有 `RwSignal<T>` + `RwSignal<PersistenceState>` + 控制器状态。
*   提供 `get`, `set`, `update`, `reload`, `flush`, `remove`, `reset`。
*   已实现 `View`，因此文本型/可视型值可直接写进 `span(...)`、`p![...]` 等 UI 位置。
*   已实现 `From<Persistent<T>> for RwSignal<T>`（在 `T: Clone + PartialEq + 'static` 下），因此 `bind_value` 等显式要求 `RwSignal<String>` 的常用 API 也可直接接受 `Persistent<String>`。

### Store 宏持久化
*   `#[derive(Store)]` 现已解析 `#[persist(...)]`，不再支持旧 `#[storage]`。
*   struct 级别支持 `#[persist(prefix = "...")]`。
*   字段级支持：
    *   `#[persist(local, codec = "string")]`
    *   `#[persist(session, codec = "parse")]`
    *   `#[persist(query, key = "q", codec = "string")]`
*   持久化字段生成 `Persistent<T>`，非持久化字段仍生成 `RwSignal<T>`。

### Component: Router
`silex/src/router.rs` -> `struct Router`
*   **Function**: 初始化路由上下文，监听 `popstate`，根据 `child` 闭包渲染视图。
*   **mount**: 
    1. 计算初始 `path` (strip `base_path`)。
    2. 创建 `path`, `search` 信号。
    3. `provide_context(RouterContext)`.
    4. 挂载子视图容器 `div`。
    5. `Effect` 监听路由变化并重新执行 `child` 工厂函数。

### component: Link
`silex/src/router/link.rs` -> `struct Link`
*   **Signature**: `Link(to: ToRoute, child: impl View)`
*   **Wrapper**: 封装 `<a>` 标签。
*   **Behavior**: `click` 事件中调用 `e.prevent_default()`，然后使用 `Navigator::push`。
*   **Enhancement**: `active_class` 根据当前 `path` 信号自动切换 CSS 类。

## 3. 流程控制 (silex::flow)

提供声明式的视图控制流，替代命令式逻辑，优化 DOM 更新。

### For Loop (silex::flow::For)
`silex/src/flow/for_loop.rs`
*   **Algorithm**: Keyed Reconciliation (Diff 算法)。
*   **Input**: `ItemsFn` (Accessor 返回 `Vec<T>`), `KeyFn` (Mapper -> Key), `MapFn` (Item -> Mount).
*   **Mechanism**:
    1. 追踪 `active_rows` (Map<Key, (Nodes, ScopeId)>)。
    2. 当列表变化时，计算新旧 Keys 差异。
    3. **Create**: 对新 Key 创建 Scope 和 View (Fragment)。
    4. **Delete**: 对消失的 Key 销毁 Scope 并移除 DOM Nodes。
    5. **Move**: 对位置变化的 Key，移动 DOM Nodes (InsertBefore)。
*   **Performance**: O(N) 复杂度，最小化 DOM 操作。

### Show (silex::flow::Show)
`silex/src/flow/show.rs`
*   **Logic**: 条件渲染 (`If-Else`)。
*   **Optimization**: 缓存上一次的 `bool` 状态，仅当状态翻转 (True <-> False) 时才重建 DOM。
*   **Sugar**: `SignalShowExt` 为 `ReadSignal<bool>` 提供 `.when(view)` 方法。

### Dynamic (silex::flow::Dynamic)
`silex/src/flow/dynamic.rs`
*   **Logic**: 任意 `Fn() -> Mount` 的动态挂载点。
*   **Implementation**: 使用 Marker Comments (`dyn-start`, `dyn-end`) 定位，每次 Effect 运行时清空区间并挂载新 View。

### Switch (silex::flow::Switch)
`silex/src/flow/switch.rs`
*   **Logic**: 多路分支选择 (`switch-case`).
*   **Mechanism**: 
    1. 接受一个 `Accessor<T>` 和一系列 `(value, view_fn)` case。
    2. 当 Accessor 值变化时，查找匹配的 case。
    3. 如果匹配索引改变，清理旧 View 并挂载新 View。
    4. 具有 `fallback` 机制。

### Index (silex::flow::Index)
`silex/src/flow/index.rs`
*   **Logic**: 基于索引的列表渲染。
*   **Difference with For**: `For` 基于 Key 移动 DOM；`Index` 原地复用 DOM，只更新数据 Signal。
*   **Mechanism**:
    1. 比较新旧列表长度。
    2. 对公共长度部分：更新对应 Item 的 Signal 值 (不触碰 DOM)。
    3. 对新增部分：创建新 Row (Signal + View) 并挂载。
    4. 对移除部分：销毁 Scope 并移除 DOM。
*   **Use Case**: 基础类型列表，或者无 ID 列表，或者列表项内容频繁变动但顺序/数量较稳定的场景。

### Portal (silex::components::Portal)
`silex/src/components/portal.rs`
*   **Logic**: 跨 DOM 层级渲染。
*   **Mechanism**:
    1.  **Target Selection**: 默认挂载到 `document.body`，也可通过 `mount_to` 属性指定特定的 `web_sys::Node`。
    2.  **Container**: 在目标节点内创建一个 `div` 容器，并设置 `display: contents` 以最小化对 CSS 布局的影响。
    3.  **Mounting**: 调用 `children.mount_ref` 将视图渲染进该容器。
    4.  **Context Preservation**: 由于挂载逻辑在父组件的作用域内运行，响应式上下文（Scope, Context）自动保持连通。
    5.  **Lifetime**: 注册 `on_cleanup` 回调。当 `Portal` 被卸载时，自动从目标位置执行 `remove_child` 移除容器，实现清理。
    6.  **Return**: Portal 在组件声明的原始位置返回空视图（`()`），不产生任何物理节点。

## 4. UI 组件 (silex::components)

### Layout (Stack, Center, Grid)
`silex/src/components/layout.rs`
*   **Stack**: 弹性容器，默认纵向 (`flex-direction: column`)。支持 `direction`, `align`, `justify`, `gap` 作为 Signal 传入。
*   **Center**: 居中容器，对应 `display: flex; align-items: center; justify-content: center;`。
*   **Grid**: 网格容器，支持 `columns` 和 `gap` 属性的快速设定。
*   **机制**: 均基于 `styled!` 宏构建，允许通过 `.style(Style::new()...)` 或 `style` props 将样式传入。

### Theme (主题系统)
`silex/src/css/theme.rs`
*   **ThemeVariables**: 零开销插入机制。通过扩展方法 `div(...).apply(theme_variables(theme_signal))` 直接将主题变量注入 `element.style`，无需额外 DOM。
*   **ThemePatch (局部补丁)**: 支持增量微调。局部补丁仅覆盖特定变量，未覆盖变量通过 CSS 继承链回退。
*   **theme! 自动化**: 
    *   **Patch 生成**: 自动生成 `{Name}Patch` 结构体，支持链式 Setter（如 `AppThemePatch::default().primary(...)`）。
    *   **强类型常量**: 自动生成 `pub const NAME: CssVar<T>`。
    *   **自动主题关联**: 使用 `#[theme(main)]` 标记后，宏会自动生成 `type Theme = ...;`。样式宏（`css!`, `styled!`）在编译时会自动探测并关联此别名以支持 `$Path::TO::CONST` (如 `$AppTheme::PRIMARY`) 静态验证。
*   **IntoSignal 兼容**: 所有主题 API 现已归一化，接收 `impl IntoSignal`。支持 `Signal`, `ReadSignal`, `Rx` (宏生成) 甚至常量，极大提升了 API 的人体工程学。
*   **全局模式**: `set_global_theme(theme_signal)` 可将主题挂载到 `:root` 上。

### ErrorBoundary
`silex/src/components/error_boundary.rs`
*   **Purpose**: 捕获子组件树中的 Errors 和 Panics，防止局部故障传播至全局。
*   **Mechanism**:
    1.  **Context Injection**: 通过 `provide_context(ErrorContext)` 注入一个错误处理闭包。该闭包捕获冒泡上来的 `SilexError`。
    2.  **Panic Capture**: 实现 `Mount` trait 时，在渲染子组件的逻辑外层使用 `std::panic::catch_unwind` (配合 `AssertUnwindSafe`)。
    3.  **State Management**: 内部持有一个 `Signal<Option<SilexError>>`。一旦捕获到错误或 Panic，通过 `wasm_bindgen_futures::spawn_local` 异步更新该信号，从而切换渲染分支。
    4.  **Fallback**: 当信号为 `Some(err)` 时，调用 `fallback` 闭包渲染备用 UI；否则渲染正常的 `children`。
    5.  **Layout**: 使用 `display: contents` 的 `div` 作为外观包装，确保对布局的侵入性最小。

### Suspense (Component)
`silex/src/components/suspense.rs`
*   **Update**: `Suspense` 现已完全组件化，由 `#[component]` 宏定义，不再使用 Builder 模式。`SuspenseBoundary` 逻辑已合并入 `Suspense`。
*   **Usage**: 
    1.  `Suspense(move || { ... })`: 接收一个工厂闭包。返回 `SharedView` 以支持复用。
    2.  内部逻辑：工厂闭包内创建的 `Resource` 会通过 `SuspenseContext` 自动注册到该 Suspense 边界。
    3.  `.fallback(view)`: 设置加载时的占位视图（接受 `impl Into<SharedView>`）。
    4.  `.mode(SuspenseMode)`: 设置 `KeepAlive` (隐藏) 或 `Unmount` (移除) 模式。
*   **Mechanism**:
    1.  **资源稳定性 (Hook-style)**: `SuspenseContext` 内部维护一个资源注册表（Resource Registry）和调用顺序索引。`Resource::new` 会首先检查该注册表，确保在 `Unmount` 模式下重新挂载（闭包重新执行）时，Resource 实例及其状态被复用，避免重复请求。
    2.  **状态重置**: 在 `Unmount` 模式下，当资源加载完成进入就绪状态时，`Suspense` 会重新调用工厂闭包生成全新的 DOM 节点。这既利用了稳定的资源数据，又实现了对本地 DOM 状态（如 input 内容）的彻底重置。
    3.  **生命周期保护**: 组件在初始化阶段会执行一次“预热”运行（Warm-up run），将资源实例锚定在稳定的组件作用域中，防止在 `Unmount` 模式切换中因临时作用域销毁而导致信号失效。

## 5. 网络系统 (silex::net)

提供统一的异步网络通信接口，支持 HTTP, WebSocket 和 SSE。

### HTTP Client (HttpClient)
`silex/src/net/builder.rs`
*   **Builder Pattern**: `HttpClient::get(url)` -> `HttpClientBuilder`。
*   **功能支持**:
    *   **动态解析**: 支持 `Signal`/`Memo` 作为 Header, Query, Path Param，请求发起时自动 resolve。
    *   **Body 类型**: 支持 Empty, Text, JSON, Form。
    *   **重试策略**: `RetryPolicy` 支持最大尝试次数、指数退避延迟及 Jitter（抖动）。
    *   **缓存策略**: 支持 `None`, `NetworkFirst`, `CacheFirst`, `StaleWhileRevalidate`。集成持久化系统。
    *   **拦截器**: 支持渲染前拦截 (`intercept`) 及响应后、重试时、出错时的回调。
*   **响应式桥接**:
    *   `.as_resource(source)`: 生成 `Resource<T, NetError>`，自动处理加载态。
    *   `.as_mutation()`: 生成 `Mutation<(), T, NetError>`，用于执行副作用请求。

### 实时通信
`silex/src/net/backend.rs`
*   **WebSocket**: `WebSocket::connect(url)` -> `WebSocketBuilder`。
    *   提供 `state` (ConnectionState), `message`, `error` 响应式信号。
    *   支持 JSON 消息的自动序列化与反序列化。
*   **EventStream (SSE)**: `EventStream::builder(url)` -> `EventStreamBuilder`。
    *   支持监听特定命名的事件或默认消息。
    *   提供消息历史 `messages` 信号及 `last_message` 辅助方法。

### 编解码器 (Codec)
`silex/src/net/codec.rs`
*   **ResponseCodec<T>**: 定义如何将 http 响应文本转换为目标类型。
*   **CacheCodec<T>**: 集成持久化时的缓存编解码逻辑。
*   **内置实现**: `TextCodec` (String), `NetJsonCodec<T>` (Serde)。

## 6. 宏支持 (Macros Support)

`silex` 通过 `silex_macros` Crate 提供编译时能力，这些宏在 `silex::prelude` 中重新导出。

### `css!` 宏集成
*   **Compile Time**: 计算 Hash，生成 Scoped CSS，压缩。
*   **Runtime**: 生成的代码自动调用 `silex::css::inject_style`。
*   **Flow**: `Macro Expansion` -> `Hash & Compress` -> `Code Gen (inject_style)` -> `Runtime Execution`.

### `#[component]` 宏集成
*   **Expansion**: 展开为 `struct Props` 和 `impl View`。
*   **Runtime**: 依赖 `silex::dom::view::View` trait 和 `silex::core::reactivity` (用于 Signal Props)。

详细实现逻辑请参考 `docs/src/ai_docs/silex_macros`。
