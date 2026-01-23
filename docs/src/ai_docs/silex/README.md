# Crate: silex

`silex` 是框架的主入口 Crate (Facade)，重新导出了核心组件并提供了上层抽象（Router, Flow Control, UI Components）。

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
*   **Side Effects**: 直接操作 DOM History API，触发 `popstate` (模拟)。

### Component: Router
`silex/src/router.rs` -> `struct Router`
*   **Function**: 初始化路由上下文，监听 `popstate`，根据 `child` 闭包渲染视图。
*   **mount**: 
    1. 计算初始 `path` (strip `base_path`)。
    2. 创建 `path`, `search` 信号。
    3. `provide_context(RouterContext)`.
    4. 挂载子视图容器 `div`。
    5. `create_effect` 监听路由变化并重新执行 `child` 工厂函数。

### component: Link
`silex/src/router/link.rs` -> `struct Link`
*   **Wrapper**: 封装 `<a>` 标签。
*   **Behavior**: `click` 事件中调用 `e.prevent_default()`，然后使用 `Navigator::push`。
*   **Enhancement**: `active_class` 根据当前 `path` 信号自动切换 CSS 类。

## 3. 流程控制 (silex::flow)

提供声明式的视图控制流，替代命令式逻辑，优化 DOM 更新。

### For Loop (silex::flow::For)
`silex/src/flow/for_loop.rs`
*   **Algorithm**: Keyed Reconciliation (Diff 算法)。
*   **Input**: `ItemsFn` (Accessor 返回 `Vec<T>`), `KeyFn` (Mapper -> Key), `MapFn` (Item -> View).
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
*   **Logic**: 任意 `Fn() -> View` 的动态挂载点。
*   **Implementation**: 使用 Marker Comments (`dyn-start`, `dyn-end`) 定位，每次 Effect 运行时清空区间并挂载新 View。

## 4. UI 组件 (silex::components)

### ErrorBoundary
`silex/src/components/error_boundary.rs`
*   **Purpose**: 捕获子组件树中的 Errors 和 Panics。
*   **Mechanism**:
    1. `provide_context(ErrorContext)`: 注入错误处理闭包。
    2. `catch_unwind`: 在 `mount` 阶段捕获同步 Panic。
    3. `SilexError`: 通过上下文捕获异步或逻辑错误。
    4. **Fallback**: 出错时替换正常子树为 `fallback` 视图。

### Suspense
`silex/src/components/suspense.rs`
*   **Purpose**: 处理异步资源加载状态 (Async/Await coordination)。
*   **Mechanism**:
    1. `provide_context(SuspenseContext)`: 计数器 (`pending_count`)。
    2. 子组件 (Resource) 加载时 `count.inc()`，完成时 `count.dec()`。
    3. `create_effect`: 监控 `count`。
        *   `count > 0`: 显示 `fallback` (display: block), 隐藏 `children` (display: none)。
        *   `count == 0`: 显示 `children`, 隐藏 `fallback`。
*   **Note**: 目前使用 CSS `display` 切换，而非 DOM 卸载，保留子组件状态。

## 5. CSS 工具 (silex::css)
`silex/src/css.rs`
*   **inject_style(id, content)**: 
    *   检查 `<head>` 中是否存在 `id`。
    *   若不存在，创建 `<style id="...">` 并注入 CSS 内容。
    *   **Idempotent**: 多次调用无副作用。

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
