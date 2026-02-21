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

#### `Suspense` (Builder) & `SuspenseBoundary`
位于 `silex/src/components/suspense.rs`。
*   **架构变更**：采用了 Builder 模式简化了“Context Layout”模式的使用。
*   **New Flow**:
    1.  `suspense()`: 启动一个 Builder。
    2.  `.resource(|| Resource::new(...))`: 注册资源创建函数。Builder 内部会自动在 `SuspenseContext` 中执行它。
    3.  `.children(|resource| ...)`: 接收创建好的 Resource，并返回最终视图（通常包含 `SuspenseBoundary`）。
*   **`SuspenseBoundary`**: 仅负责 UI 切换逻辑（Loading / Fallback / Content）。
    *   **Context Capture**: 必须在 Builder 的 `.children` 闭包内（即 Context 作用域内）使用。
    *   **Modes**: 依然支持 `KeepAlive` (CSS Toggle) 和 `Unmount` (Physical DOM removal) 两种策略。

#### `Portal`
位于 `silex/src/components/portal.rs`。
*   **Context 连通性**：这是 Portal 最重要的特性。尽管 DOM 节点被挂载到了 `body` 或其他位置，但由于 `Reactive Scope` 是在组件创建时建立的，Portal 内部的代码仍然可以访问其书写位置（Lexical Scope）的 Context。这使得在 Portal 内部使用 `use_context` 依然有效。

### 4.4 CSS 工具 (CSS Tools)

#### 强类型 CSS 运行时 (Type-Safe CSS Runtime)
位于 `silex/src/css.rs` 及 `silex/src/css/types.rs`。
*   **架构设计**：它不单纯是传统意义上的 CSS Runtime 工具链，而是与 `silex_macros` 协同构筑的前后端一体化防线。抛弃单纯接受一切 `Display` 给字符串 `+` 的行为。
*   **Property Tags (属性感知)**：基于 MDN (Mozilla Developer Network) 的标准 CSS 属性数据自动化生成。内置了数以百计零开销的 Trait Bounds Tag 结构体（ZST）充当标识（诸如 `props::Width`，`props::Color` 等）。
*   **验证流**：伴随 `DynamicCss` 产生的每一次插值的验证绑定，将宏层面所追踪到的标签经由此处的 `make_dynamic_val_for::<P, S>(source: S)` 落入限制。使得 `ValidFor<P>` 这个 Trait 得以在运行时构建前提前在编译期就成功实施阻断由于随意插值引发的语法错误！实现严丝合缝的闭环。
*   **封锁隐式逃逸与 `UnsafeCss`**：彻底废除对 `&str`、`String` 及 `Any` 属性的泛用 `ValidFor` 实现，转而要求开发者在需要越过类型检查时显式声明使用 `UnsafeCss::new(...)`，显式标明非安全 CSS 越境边界。
*   **复合复合与工厂函数 (Factory Functions)**：对于 `border` 这类需要多种类型排版的复杂属性，我们抽离宏层面的不确定推断，彻底采用 Rust `const fn border(width, style, color)` 工厂函数及其对应类型 `BorderValue` 来处理，保障属性间的调用签名依然 100% 安全。
#### 类型安全构建器 (Type-Safe Builder: Style)
位于 `silex/src/css/builder.rs`。
*   **设计动机**：为了满足对极致编译性能（零宏路径）和 100% Rust 原生补全极致追求的场景。
*   **核心逻辑**：
    *   **链式 API**：提供一系列强类型方法如 `.width(px(100))`。它不仅仅是字符串拼接，而是利用 `IntoSignal` 和 `ValidFor` trait 在编译期拦截类型不匹配的属性赋值。
    *   **智能分配**：
        *   当属性是**常量**时，合并进 `static_rules`。多处使用相同 `Style` 的静态部分会被哈希成同名 Class，共享样式注入到 `<head>`。
        *   当属性是**信号/闭包**时，进入 `dynamic_rules`。在 DOM 挂载时通过响应式 Effect 绑定 **CSS 变量**。
    *   **高频更新优化**：这是 Silex 的核心优化点。对于动态属性（如 `width: $(w)`），系统会为对应的 class 生成一个唯一的 CSS 变量占位（例如 `--sb-hash-0`）。当信号更新时，Effect 只执行轻量的 `element.style.setProperty('--sb-hash-0', val)`。这避免了修改内联 style 字符串导致的浏览器样式重计算压力，且能与静态 Class 完美配合。
    *   **伪类响应式支持**：通过 `on_hover(|s| ...)` 定义的样式。如果其中包含动态部分，Style 引擎会为该元素分配唯一的 `slx-bldr-dyn-N` 类名，并在全局 `<style>` 标签中实时更新该类名的伪类定义（由 `DynamicStyleManager` 管理），解决了内联样式（inline-style）无法覆盖伪类的局限。

## 5. 存在的问题和 TODO (Issues and TODOs)

1.  (已解决) **Flow 组件类型简化**: `For` 和 `Show` 的泛型参数已大幅简化，移除了结构体上的冗余泛型，改用辅助 Trait 进行推导。
