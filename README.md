# Silex

Silex 是一个用于构建 Web 应用程序的下一代 Rust 库。它深受 **SolidJS** 的细粒度响应式设计启发，但专为 Rust 语言特性进行了优化。它摒弃了虚拟 DOM (VDOM)，通过信号 (Signal) 和副作用 (Effect) 直接驱动真实 DOM 的更新，从而实现极致的性能。

Silex 的核心设计理念是 **"Rusty & Fluent"** —— 提供一套符合 Rust 编程习惯的、类型安全的、基于构建者模式 (Builder Pattern) 的 API，而不是过度依赖宏或为了模仿 JSX 而牺牲 Rust 的强类型优势。

## 🌟 核心设计思路 (Design Philosophy)

### 1. 无虚拟 DOM (No VDOM) & 细粒度响应式
Silex 不使用虚拟 DOM Diff 算法。相反，它采用细粒度的响应式系统。
- **即时更新**：当状态 (Signal) 发生变化时，只有依赖该状态的具体 DOM 属性或文本节点会更新，不会有组件级的重渲染开销。
- **精确依赖追踪**：系统自动收集依赖关系，开发者无需手动声明依赖数组。

### 2. 流式构建者 API (Fluent Builder API)
Silex 提倡使用**构建者模式**来组装 UI，而不是使用类似 JSX 的宏。
- **类型安全**：所有属性、样式和事件绑定都是类型检查的。
- **IDE 友好**：利用 Rust 强大的类型系统，提供优秀的代码补全和重构体验。
- **组合优于继承**：组件仅仅是实现了 `View` 特征的结构体或函数，易于组合。

```rust
// 示例：流式 API
div()
    .class("container")
    .style("display: flex")
    .child(
        button()
            .on_click(|| println!("Clicked!"))
            .text("Click Me")
    )
```

### 3. 类型安全与多态视图 (Type-Safe Attributes & Polymorphic Views)
Silex 充分利用 Rust 的类型系统，实现了编译时的 HTML 属性检查：
- **类型安全 (Type-Safe)**：DOM 元素被强类型化（如 `TypedElement<Div>` vs `TypedElement<Input>`）。只有合法的 HTML 属性才能被调用。
- **属性多态**：所有属性方法（如 `.id()`, `.value()`）都支持多态参数，不仅接受静态值，还接受 `Signal`、闭包等。
- **视图多态**：`View` 特征广泛支持各种 Rust 类型，UI 结构定义自然。

## 🧩 宏系统 (Macros System)
Silex 提供了强大的过程宏来提升开发体验 (DX)：

- **`#[component]`**：自动为组件结构体生成构建者模式 (Builder Pattern) 方法，并支持对 `Children`, `AnyView`, `String` 等类型的自动 `into` 转换，消除样板代码。
- **`css!`**：支持 CSS-in-Rust。编写标准的 CSS 语法，宏会在编译时进行解析、验证、压缩，并生成唯一的哈希类名 (e.g., `slx-1a2b3c`)，实现局部作用域样式。
- **`#[derive(Routable)]`**：声明式路由核心。通过在 Enum 上标注 `#[route("/path/:param")]`，自动生成路径匹配与生成逻辑，实现完全的类型安全路由。
- **`#[derive(Store)]`**：为结构体自动生成细粒度的响应式 Store，将每个字段转换为 `RwSignal`，方便深层状态管理。

## 🏗️ 设计架构与实现 (Architecture & Implementation)

Silex 的架构主要由三个核心模块组成：**Reactivity (响应式核心)**、**DOM (视图层)** 和 **Flow (控制流)**。

### 1. 响应式核心 (`src/reactivity`)
这是驱动整个框架的引擎，位于 `silex/src/reactivity.rs` 和 `runtime.rs`。

- **Runtime (运行时)**：
    - **Split Store 架构**：使用 `SlotMap` 存储响应式节点拓扑，配合 `SecondaryMap` 分离存储 Signal 数据和 Effect 逻辑。这种数据局部性优化提高了缓存效率。
    - **BFS Update Queue**：使用广度优先队列调度副作用更新，不仅避免了深层依赖导致的栈溢出，还确保了更新顺序的可预测性。
    - **稳定引用**：通过 `NodeId` 句柄访问节点，巧妙避开了 Rust 的借用检查器限制，实现了灵活的反应式图结构。

- **Dependency Tracking (依赖追踪)**：
    - **自动收集**：在副作用执行期间读取 Signal 会自动记录依赖。
    - **动态修剪**：每次 Effect 重新运行时会清空旧依赖并重新收集，确保依赖图始终精确最小。
    - **O(1) 去重**：利用版本号机制（Versioning）实现高效的依赖查重，避免重复注册。

- **Scope System (作用域系统)**：
    - 树状层级管理 (`Owner/Parent-Child`)。
    - **自动垃圾回收**：当 Scope 被销毁时，递归清理所有子节点（Signal, Effect）并触发 `on_cleanup` 回调，有效防止内存泄漏。

- **Primitives (原语)**：
    - `create_signal`: 创建基础的读写信号对。
    - `create_rw_signal`: 创建合并了读写功能的信号对象。
    - `create_effect`: 创建自动追踪依赖的副作用。
    - `create_memo`: 创建带缓存的派生信号，通过 `PartialEq` 减少下游不必要的更新。
    - `create_resource`: 集成 `Future` 的异步资源，内置防竞态机制（Request ID）和生命周期管理。
    - `untrack`: 在不被追踪的情况下读取信号。
    - `provide_context` / `use_context`: 基于类型 ID 的依赖注入系统。

### 2. DOM 抽象层 (`src/dom`)
这一层连接响应式系统与浏览器的 `web-sys` API。

- **Typed Element System (`dom/core/*`)**：
    - **TypedElement<T>**：对 `web_sys::Element` 的类型安全包装，其中 `T` 是标签标记（如 `Div`, `Input`）。
    - **Trait-Based Props**：属性方法通过 `GlobalAttributes`, `FormAttributes` 等特征按需实现。
    - **智能绑定**：属性方法接收 `impl AttributeValue`。如果传入 `ReadSignal` 或闭包，会自动创建 Effect 保持同步；静态值则只设置一次。
- **View Trait (`dom/core/view.rs`)**：
    - 定义了 `mount(self, parent: &Node)` 方法。
    - **Range Cleaning**：闭包作为 View 时（`Fn() -> impl View`），实现了基于锚点（Marker）的区域清理策略 ("Virtual Fragment")，无需额外的包裹节点即可实现细粒度的动态更新。
    - 广泛类型支持：包括 `Option`, `Result`, `Vec`, Tuple (Fragment) 等。
- **Suspense (`dom/components/suspense.rs`)**：
    - 结合 `SuspenseContext`，通过监控异步任务计数器，在数据加载时自动切换显示（通过 CSS display）`fallback` 视图和实际 `children` 视图。

### 3. 控制流组件 (`src/flow`)
Silex 不使用编译器魔法处理 `if` 或 `for`，而是提供高效的组件。

- **Show (`flow/show.rs`)**：
    - 条件渲染组件。
    - 提供了 `.when(condition).otherwise(fallback)` 的语法糖。
    - 优化：当条件变化时，只挂载/卸载必要的分支。
- **For (`flow/for_loop.rs`)**：
    - 列表渲染组件。
    - **Keyed Diffing**：通过 `key` 函数追踪列表项。当列表数据变化时，它会对比新旧 key 集合，仅对新增、删除或移动的项进行 DOM 操作，复用已有的 DOM 节点和 Scope，性能远超全量重建。
- **Dynamic (`flow/dynamic.rs`)**：
    - 通用动态组件，根据闭包返回的 View 类型动态切换内容。

### 4. 路由系统 (`src/router`)
`Router` 组件提供了类型安全的客户端导航能力，无需页面刷新。
- **Typed Routing**：通过 `#[derive(Routable)]` 定义 Enum 路由，编译器保证路由定义的正确性。支持参数匹配 (`:id`) 和通配符 (`*`)。
- **Match Enum**：`Router::new().match_enum(...)` 模式，将路由匹配逻辑与视图渲染解耦。
- **RouterContext**：提供 `use_navigate`, `use_location_path` 等 Hooks 方便在任何组件中获取路由状态。

## 📝 核心模块概览

| 模块 | 路径 | 描述 |
|------|------|------|
| **Dom** | `dom/core/*` | 类型安全的 DOM 封装、属性 Trait 系统、SVG支持。 |
| **View** | `dom/core/view.rs` | `View` 特征定义、动态视图更新策略 (Range Cleaning)。 |
| **Reactivity** | `reactivity.rs`, `reactivity/runtime.rs` | 信号、副作用、Scope 管理、运行时状态。 |
| **Flow** | `flow/*.rs` | `Show`, `For` 等控制流组件。 |
| **Router** | `router.rs`, `router/context.rs` | 类型安全路由、History API 集成。 |
| **Macros** | `silex_macros/src/*` | `css!`, `component`, `Routable`, `Store` 等宏实现。 |
| **Suspense** | `dom/components/suspense.rs` | 异步边界处理，支持 Loading 状态回退。 |
| **ErrorBoundary** | `dom/components/error_boundary.rs` | 错误边界，捕获子组件 Panic 和 Result::Err。 |

## 🚀 示例代码

以下代码展示了如何使用 Props Builder 模式、Router、Context API 和控制流组件来构建应用：

```rust
use silex::prelude::*;

fn main() {
    create_scope(move || {
        let (count, set_count) = create_signal(0);
        // 全局状态注入
        provide_context(count); 

        div()
            .class("app")
            .child((
                // 导航栏
                nav().child((
                    link(AppRoute::Home.to_path().as_str()).text("Home"),
                    link(AppRoute::About.to_path().as_str()).text("About"),
                )),

                // 路由配置
                Router::new()
                    .match_enum(|route: AppRoute| match route {
                        AppRoute::Home => HomeView().into_any(),
                        AppRoute::About => AboutView().into_any(),
                        AppRoute::NotFound => NotFound().into_any(),
                    })
            ))
            .mount(&document.body().unwrap());
    });
}

// 定义路由 Enum
#[derive(Clone, PartialEq, Routable)]
enum AppRoute {
    #[route("/")]
    Home,
    #[route("/about")]
    About,
    #[route("/*")]
    NotFound,
}

fn HomeView() -> impl View {
    // 使用 Context
    let count = use_context::<ReadSignal<i32>>().unwrap();

    div().child((
        h1().text("Home"),
        p().text(move || format!("Global Count: {}", count.get().unwrap()))
    ))
}
```

## 🛠️ 当前状态与未来目标 (Status & Roadmap)

Silex 目前已达到 **Core Feature Complete** 状态，核心架构稳定，可以用于构建中小型 CSR 应用。

### ✅ 已完成特性 (Completed)
- **核心架构**: 细粒度响应式系统 (Signals/Effects), Scope 内存管理, Auto-Tracking.
- **视图层**: **Fully Typed DOM**, 泛型 View 系统, Range Cleaning Fragment.
- **开发体验**: 完整的宏支持 (`css!`, `#[component]`, `Routable`, `Store`)，Builder API.
- **功能组件**: `Router` (Type-safe), `Suspense` (Async), `ErrorBoundary`, `Show`, `For` (Keyed Diffing).
- **样式**: First-party Scoped CSS (CSS-in-Rust).

### 🚧 下一步目标 (Roadmap)
- **1. 工具链与生态 (Ecosystem)**:
    - 开发 CLI 工具 (`silex-cli`) 以支持快速脚手架和构建。
    - 提供更多开箱即用的 UI 组件 (headless components)。
- **2. 服务端渲染 (SSR & Hydration)**:
    - *Long-term Goal*: 尽管目前仅支持 CSR，但在架构上已预留了 SSR 的可能性 (e.g. `mount` 抽象)。未来将探索无 VDOM 的流式 SSR 和部分水合 (Partial Hydration) 方案。
- **3. 性能优化 (Performance)**:
    - 进一步优化 `For` 循环的 reconcile 算法。
    - 引入编译时优化 (Compiler Optimizations)，预编译静态模板。
- **4. 测试与文档**:
    - 增加单元测试覆盖率，特别是针对边缘情况。
    - 完善英文文档和示例库。
