# Silex 核心库文档

`silex` 是框架的核心库，它整合了 DOM 操作、响应式系统、路由和标准组件，为您提供一站式的 Web 开发体验。

## 快速开始

在您的 `Cargo.toml` 中引用 `silex`：
```toml
[dependencies]
silex = { path = "../../silex" } # 或者使用 git/crates.io 版本
```

推荐在代码中导入 prelude：
```rust
use silex::prelude::*;
```

## 1. 路由系统 (Router)

Silex 提供了一个类型安全且易于使用的客户端路由。

### 基本用法

使用 `<Router>` 组件包裹您的应用，并定义路由规则：

```rust
fn App() -> impl View {
    Router::new()
        .base("/app") // 可选：设置基础路径
        .render(|| {
            // 这里通常放置布局组件（Layout）
            // 根据路由匹配显示不同内容
            let path = use_location_path();
            
            div((
                nav((
                    Link("/", "首页"),
                    Link("/about", "关于"),
                )),
                main((
                    Dynamic::new(move || {
                        match path.get().as_str() {
                            "/" => Home().into_any(),
                            "/about" => About().into_any(),
                            _ => NotFound().into_any(),
                        }
                    })
                ))
            ))
        })
}
```

### 使用枚举管理路由 (推荐)

对于大型应用，建议使用 Enum + `#[derive(Route)]` (需启用宏) 来管理路由：

```rust
#[derive(Route, Clone, PartialEq)]
enum MyRoutes {
    #[route("/")]
    Home,
    #[route("/users/:id")]
    User(i32),
    #[route("/*")]
    NotFound,
}

impl RouteView for MyRoutes {
    fn render(&self) -> AnyView {
        match self {
            MyRoutes::Home => Home().into_any(),
            MyRoutes::User(id) => UserPage(*id).into_any(),
            MyRoutes::NotFound => NotFound().into_any(),
        }
    }
}

// 在 Router 中使用
Router::new().match_route::<MyRoutes>()
```

### 导航

*   **HTML**: 使用 `<Link>` 组件代替 `<a>` 标签。
    ```rust
    Link(MyRoutes::Home, "Go Home")
    ```
*   **Code**: 使用 `use_navigate` hook。
    ```rust
    let nav = use_navigate();
    nav.push("/new-path");
    ```

## 2. 流程控制 (Flow Control)

Silex 提供了一组组件来处理常见的逻辑控制，这比手动编写 `move ||` 闭包更具可读性且性能更好。

### Show (条件渲染)
```rust
let (is_logged_in, set_log) = signal(false);

Show::new(is_logged_in, || UserDashboard())
    .fallback(|| LoginButton())
```
或者使用语法糖：
```rust
is_logged_in.when(|| UserDashboard())
```

### Switch (多路分支)
类似于 `match` 语句，根据值选择渲染的内容。
```rust
let (tab, set_tab) = signal(0);

Switch::new(tab, || div("Fallback"))
    .case(0, || TabA())
    .case(1, || TabB())
```

### Portal (传送门)
将组件渲染到 DOM 树的其他位置（如 `body`），常用于模态框（Modals）、Tooltips。
```rust
Portal::new(div("I am a modal"))
    .mount_to(document.body().unwrap()) // 默认也是 body
```

### For (列表渲染)
高效渲染列表数据，支持 Keyed Diff 算法。

```rust
let (users, set_users) = signal(vec![
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
]);

For::new(
    users,           // 数据源 (Signal)
    |u| u.id,        // Key 提取函数 (必须唯一且稳定)
    |u| div(u.name)  // 渲染函数
)
```

### Index (索引列表渲染)
当列表项没有唯一 ID，或者列表项是基础类型（如 `Vec<String>`），或者列表长度固定仅内容变化时，使用 `Index` 比 `For` 更高效。它**复用** DOM 节点，仅更新 Signal。

```rust
let (logs, set_logs) = signal(vec!["Log 1", "Log 2"]);

Index::new(logs, |item, index| {
    // item 是 ReadSignal<T>，内容变化时直接更新文本节点
    div((index, ": ", item))
})
```

## 3. 错误处理 (Warning & Error)

使用 `<ErrorBoundary>` 可以捕获子组件中的 Panic 或 `SilexError`，防止整个应用崩溃。

```rust
ErrorBoundary(ErrorBoundaryProps {
    fallback: |err| div(format!("发生错误: {}", err)).style("color: red"),
    children: || {
        // 可能出错的组件
        DangerousComponent()
    }
})
```

## 4. 异步加载 (Suspense)

配合 `Resource` 使用，优雅处理异步数据加载状态。

```rust
Suspense::suspense()
    .fallback(|| div("Loading..."))
    .children(|| {
        let data = Resource::new(...); // 异步资源
        div(move || data.get().unwrap_or_default())
    })
```

## 5. 常用宏与工具 (Macros & Utilities)

Silex 提供了一系列宏来简化开发，这些宏都已包含在 prelude 中。

### 组件定义 (`#[component]`)
```rust
#[component]
fn MyComp(name: String, #[prop(default)] age: i32) -> impl View {
    div(format!("Name: {}, Age: {}", name, age))
}
```

### CSS 编写 (`css!`)
```rust
let cls = css!("color: red; &:hover { color: blue; }");
div("Hello").class(cls)
```

### 属性助手 (`style!`, `classes!`)
*   `style!`: `div(()).style(style! { "color": "red", "margin": "10px" })`
*   `classes!`: `div(()).class(classes!["btn", "active" => is_active])`

详细文档请参阅 [silex_macros 文档](../silex_macros/README.md)。
