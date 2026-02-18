# Silex 框架核心文档

## 1. 概要 (Overview)

`silex` 是整个框架的主入口 Crate (Facade)，它不仅重新导出了底层核心库（`silex_core`, `silex_dom`, `silex_html`），还提供了构建完整 Web 应用所需的高级抽象。

*   **定义**：Silex 框架的应用层封装，提供路由、流控制、状态管理和常用 UI 组件。
*   **作用**：开发者主要通过此 Crate 与框架交互。它整合了响应式系统和 DOM 操作，提供了类似 React/Solid 的开发体验，但保持了 Rust 的高性能和类型安全。
*   **目标受众**：面向所有使用 Silex 构建应用的开发者。

## 2. 理念和思路 (Philosophy and Design)

*   **统一入口**：通过 `silex::prelude` 提供一站式导入，减少开发者对底层模块结构的认知负担。
*   **组件即函数**：Silex 中没有特殊的“组件实例”概念，组件本质上就是返回 `View` 的函数。`silex` Crate 中提供的 `For`, `Show`, `Router` 等都是遵循此约定的普通函数或结构体。
*   **精细化控制流**：不依赖编译器的特殊语法（如 JSX 中的控制流），而是提供基于闭包和响应式原语的控制流组件（`silex::flow`），确保只有变化的部分才会重新执行和渲染。
*   **类型安全路由**：在提供字符串路由的同时，鼓励使用 Enum 和 Pattern Matching 进行类型安全的路由匹配，充分利用 Rust 类型系统的优势。

## 3. 模块内结构 (Internal Structure)

代码组织清晰地按照功能划分：

```text
silex/src/
├── lib.rs              // 根模块，负责重导出和 Prelude 定义
├── css.rs              // 简单的 CSS 运行时注入工具
├── store.rs            // 全局状态管理 Trait
├── components/         // 内置 UI 组件
│   ├── error_boundary.rs // 错误边界
│   ├── portal.rs         // 传送门（跨 DOM 渲染）
│   └── suspense.rs       // 异步资源挂起
├── flow/               // 逻辑控制流组件
│   ├── dynamic.rs        // 动态组件
│   ├── for_loop.rs       // 列表渲染 (Keyed)
│   ├── index.rs          // 索引渲染 (Non-keyed)
│   ├── show.rs           // 条件渲染
│   └── switch.rs         // 多路分支
└── router.rs           // 客户端路由系统 (核心逻辑)
└── router/             // 路由子模块
    ├── context.rs        // 路由状态上下文
    └── link.rs           // 链接组件
```

核心关系：
*   **`Router`** 依赖 **`silex_core::reactivity`** 进行状态管理，依赖 **`web_sys`** 进行 History API 操作。
*   **`Flow` 组件** 是连接 **数据信号** (Signal) 和 **DOM 更新** (View) 的桥梁，它们内部持有闭包并在 Effect 中执行，以响应数据变化。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 路由器系统 (Router System)

Silex 的路由系统基于浏览器 History API，实现了单页应用 (SPA) 的客户端导航。

*   **`RouterContext`**: 存储路由的核心状态，包括 `path`, `search` (Query Params) 和 `navigator`。
    *   **设计细节**：使用 `ReadSignal<String>` 存储路径，确保路径变化时只通知依赖该路径的组件。`base_path` 的处理逻辑内建在 Router 和 Link 中，支持应用部署在子路径下。
*   **`Router` 组件**:
    *   **初始化**：在 `mount` 阶段监听 `popstate` 事件，并将当前的路由状态注入到 Context 中。
    *   **渲染**：`Router` 维护一个 `child` 闭包。当使用 `match_enum` 时，它会创建一个依赖于 `path` 信号的计算闭包。一旦路径改变，这个闭包重新执行，进行 Enum 匹配并返回新的 View。
*   **`Link` 组件**:
    *   这是一个封装了 `<a>` 标签的组件。
    *   **拦截点击**：它会拦截 `click` 事件，调用 `event.prevent_default()`，然后使用 `Navigator::push` 进行无刷新跳转。
    *   **Active State**：通过读取 Router Context 中的当前路径，自动计算并更新 `active` CSS 类，通过 `active_class` 方法配置。

*   **Query Params 优化**:
    *   **标准化解析**: `use_query_map` 内部统一使用 `web_sys::UrlSearchParams`，确保与浏览器行为一致。
    *   **原子更新**: `Navigator::set_query` 提供了基于当前 URL 的增量更新能力。
    *   **循环检测**: `use_query_signal` 利用 `StoredValue` 缓存上一次同步值，只有在真正需要时才回写 URL，避免了双向绑定的无限循环和不必要的 History Push。

### 4.2 流程控制 (Flow Control)

这些组件是 Silex 性能优化的核心。它们避免了全量 DOM Diff，直接针对特定逻辑进行更新。

#### `For` (Keyed List)
位于 `silex/src/flow/for_loop.rs`。
*   **核心算法**：实现了一个基于 Key 的 Reconciliation 算法。
*   **零拷贝优化**：引入 `ForLoopSource` trait，允许直接操作 `&[T]` 切片，只在需要创建新行时才克隆 Item。
*   **DOM 操作**：
    *   **Scope 管理**：为列表的每一项创建一个独立的 Reactive Scope (`create_scope`)。这非常关键，意味着列表项内部的信号更新不会影响外部，反之亦然。销毁行时必须调用 `dispose`。
    *   **Diff 逻辑**：维护一个活跃行映射 `active_rows: HashMap<Key, (Nodes, ScopeId)>`。每次更新时：
        1.  **复用**：Key 相同的行直接复用 DOM 节点和 Scope。
        2.  **新建**：新 Key 创建新 DOM 和 Scope。
        3.  **删除**：不存在的 Key 移除 DOM 并销毁 Scope。
        4.  **移动**：通过检查节点在 DOM 中的位置 (`next_sibling`) 来判断是否需要 `insert_before` 进行移动，最小化 DOM 操作。

#### `Show` (Conditional)
位于 `silex/src/flow/show.rs`。
*   **缓存机制**：内部有一个 `prev_state` 记录上一次的布尔值。只有当条件从 `true` 变 `false` 或反之时，才会触发布局更新（清空旧节点，挂载新节点）。这避免了条件并未实质改变时的重复渲染。
*   **Marker 节点**：使用两个注释节点 `<!--show-start-->` 和 `<!--show-end-->` 定位插入点，确保在动态替换内容时不会弄乱父容器中的其他可变内容。

#### `Dynamic`
位於 `silex/src/flow/dynamic.rs`。
*   **通用挂载点**：接受任意返回 View 的闭包。每当闭包依赖的信号变化时，清空两个 Marker 之间的内容并重新挂载新生成的 View。它是实现多态组件的基础。

### 4.3 UI 组件 (Components)

#### `ErrorBoundary`
位于 `silex/src/components/error_boundary.rs`。
*   **双重捕获**：
    1.  **同步 Panic**：使用 `std::panic::catch_unwind` 捕获渲染期间的 Rust Panic。
    2.  **逻辑/异步错误**：通过 `provide_context(ErrorContext)` 注入错误处理句柄。子组件可以通过 `SilexError` 向上抛出错误。
*   **Fallback**：一旦捕获错误，立即卸载子树并渲染 `fallback` 提供的 UI。

#### `Suspense`
位于 `silex/src/components/suspense.rs`。
*   **异步协调**：基于 `SuspenseContext` 中的引用计数。
*   **实现策略**：目前的实现采用了 **CSS 切换** 策略 (`display: none` vs `block`)。
    *   **优点**：保留了正在加载的子组件的状态（如果它已经被部分渲染）。
    *   **流程**：初始化时提供 Context -> 子资源加载 (`inc`) -> 显示 Fallback -> 子资源完成 (`dec`) -> 显示内容。

#### `Portal`
位于 `silex/src/components/portal.rs`。
*   **Context 连通性**：这是 Portal 最重要的特性。尽管 DOM 节点被挂载到了 `body` 或其他位置，但由于 `Reactive Scope` 是在组件创建时建立的，Portal 内部的代码仍然可以访问其书写位置（Lexical Scope）的 Context。这使得在 Portal 内部使用 `use_context` 依然有效。

## 5. 存在的问题和 TODO (Issues and TODOs)

1.  **Suspense 内存优化**: 当前 `Suspense` 加载时同时保留了 Fallback 和 Hidden Content 的 DOM 节点。对于极其庞大的子树，这可能带来内存压力。未来计划支持“卸载模式”，在显示 Fallback 时暂时卸载子树。
2.  **Flow 组件类型简化**: `For` 和 `Show` 的泛型参数非常多，生成的类型签名过长，影响错误信息的阅读。需要探索简化类型签名的方法。
