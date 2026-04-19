# Silex

**下一代高性能 Rust Web 框架 | Next Generation High-Performance Rust Web Framework**

Silex 是一个基于 **细粒度响应式 (Fine-Grained Reactivity)** 和 **无虚拟 DOM (No Virtual DOM)** 架构的 Rust Web 框架。它结合了 **SolidJS** 的极致性能与 **SwiftUI** 的 **流式声明式 API**，旨在为 Rust 开发者提供最符合直觉的 Web 开发体验。

---

## 🌟 核心特性 (Key Features)

### 1. 🚀 极致性能 (Blazing Fast)
Silex 摒弃了传统的虚拟 DOM Diff 算法。通过精确的依赖追踪，应用状态 (Signal) 的变化会直接更新对应的 DOM 节点。
*   **O(1) 更新复杂度**：无论应用多大，更新成本仅与变化的数据量相关。
*   **零运行时开销**：构建者模式和宏在编译时优化，运行时极为轻量。

### 2. 🦀 锈式美学 (Rusty & Fluent)
Silex 提供了一套完全符合 Rust 习惯的流式构建者 API (Builder API)。
*   **Children-First**：像 SwiftUI 一样编写 UI，结构清晰，层级分明。
*   **类型安全**：从 HTML 属性到事件处理，一切皆有类型检查，彻底告别运行时拼写错误。
*   **灵活风格**：支持 **宏风格 (`div![...]`)**、**函数风格 (`div(...)`)** 以及 **混合风格**，满足不同开发偏好。

### 3. 🛠️ 全栈工具链 (Batteries Included)
Silex 不仅仅是一个视图库，它提供了构建现代 Web 应用所需的一切：
*   **路由系统 (`silex_router`)**：类型安全的客户端路由，支持嵌套和参数配置。
*   **状态管理 (`silex_store`)**：基于宏的细粒度全局状态管理。
*   **CSS-in-Rust (`silex_css`)**：支持局部作用域的 CSS 宏，编译时压缩与校验。
*   **异步原语**：内置 `Resource` 和 `Suspense`，轻松处理异步数据加载。

---

## 📦 快速开始 (Quick Start)

### 1. 添加依赖

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
silex = "0.1.0-beta.6" # 请使用最新版本
```

### 2. 编写你的第一个应用

```rust
use silex::prelude::*;

#[component]
fn Counter() -> impl View {
    // 创建响应式信号
    let (count, set_count) = Signal::pair(0);
    
    // 派生状态 (Memo)
    let double_count = Memo::new(move |_| count.get() * 2);

    div![
        h1("Silex Counter Demo"),
        
        div![
            button("-").on_click(move |_| set_count.update(|n| *n -= 1)),
            
            // 文本节点自动响应信号变化
            span(move || format!("Count: {}", count.get()))
                .style("margin: 0 10px; font-weight: bold;"),
                
            button("+").on_click(move |_| set_count.update(|n| *n += 1)),
        ],

        // 控制流组件
        Show::new(
            move || count.get() > 5,
            || p("Count is greater than 5!").style("color: red;")
        ),
        
        p(move || format!("Double: {}", double_count.get()))
    ]
    .style("padding: 20px; text-align: center;")
}

fn main() {
    // 挂载应用到 Body
    mount_to_body(|| Counter());
}
```

---

## 🧩 模块概览 (Modules Overview)

Silex 采用模块化设计，核心功能拆分为多个 Crate 以保持架构清晰。

| Crate | 描述 | 文档重点 |
| :--- | :--- | :--- |
| **`silex`** | **主入口 (Facade)** | 重新导出所有核心功能，提供顶层 API。 |
| **`silex_core`** | **核心逻辑** | `Signal`, `Effect`, `Resource`, `Context` 等响应式原语。 |
| **`silex_dom`** | **DOM 绑定** | `TypedElement`, `View` Trait, 以及属性系统实现。 |
| **`silex_html`** | **HTML DSL** | 包含 HTML5 规范的所有标签构造函数 (`div`, `span`, `input`...)。 |
| **`silex_macros`** | **宏支持** | `#[component]`, `css!`, `#[derive(Route)]`, `#[derive(Store)]`。 |
| **`silex_reactivity`** | **响应式引擎** | 底层无类型的响应式图谱实现 (Runtime, NodeId, Graph)。 |

---

## 🎨 核心功能展示

### 1. 声明式路由 (Router)

通过 `Enum` 定义路由，享受编译时类型检查带来的安稳。

```rust
#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = Home)]
    Home,
    #[route("/about", view = About)]
    About,
    #[route("/users/:id", view = User)]
    User { id: u32 }, // 自动解析 URL 参数
    #[route("/*", view = NotFound)]
    NotFound,
}

#[component]
fn App() -> impl View {
    Router::new().match_route::<AppRoute>()
}
```

### 2. CSS-in-Rust

不再需要单独的 CSS 文件，支持完整的 CSS 语法（嵌套、媒体查询、主题变量），并享受编译时校验。

```rust
let btn_class = css! {
    background-color: #007bff;
    color: white;
    padding: 8px 16px;
    border-radius: 4px;
    transition: all 0.2s;
    
    &:hover {
        background-color: #0056b3;
        transform: translateY(-2px);
    }

    @media (max-width: 600px) {
        width: 100%;
    }
};

button("Click Me").class(btn_class)
```

### 3. 全局状态 (Store)

复杂状态管理变得简单而直观。

```rust
#[derive(Store, Clone, Default)]
#[persist(prefix = "settings-")]
struct UserSettings {
    #[persist(local, codec = "string")]
    theme: String,
    #[persist(local, codec = "parse")]
    notifications: bool,
}

let settings = expect_context::<UserSettingsStore>();
settings.theme.set("Dark".to_string());
```

### 4. 外部状态持久化 (Persistence)

`Persistent::builder(key)` 统一封装了 `localStorage`、`sessionStorage` 和 URL query 三类外部状态后端。

```rust
let theme = Persistent::builder("theme")
    .local()
    .string()
    .default("Light".to_string())
    .build();

let page = Persistent::builder("page")
    .query()
    .parse::<u32>()
    .default(1)
    .build();

input().bind_value(theme);
span(page);
```

---

## 🤝 贡献 (Contributing)

Silex 处于快速迭代阶段，欢迎任何形式的贡献！无论是提交 Issue、PR，还是完善文档。

详情请参考 `docs/` 目录下的开发文档：
- [Silex Reactivity Design](docs/src/general_docs/silex_reactivity/README.md)
- [Silex Macro Guide](docs/src/general_docs/silex_macros/README.md)
- [Silex Core API](docs/src/general_docs/silex_core/README.md)

---

## 📄 许可证 (License)

[MIT License](LICENSE-MIT)

[Apache License 2.0](LICENSE-APACHE)
