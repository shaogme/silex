# Silex 核心库文档

`silex` 是框架的核心库，它整合了 DOM 操作、响应式系统、路由和标准组件，为您提供一站式的 Web 开发体验。

## 快速开始

在您的 `Cargo.toml` 中引用 `silex`：
```toml
[dependencies]
silex = { path = "../../silex" } # 或者使用 git/crates.io 版本
```

## 功能特性 (Feature Flags)

`silex` 提供以下功能开关，以优化编译时间和依赖体积：

| Feature | Description | Default |
| :--- | :--- | :--- |
| `macros` | 启用 `css!`, `#[component]`, `#[derive(Store)]` 等宏支持。 | Yes |
| `persistence` | 启用统一持久化系统 (`silex::persist`)。 | No |
| `json` | 启用基于 Serde 的 JSON 编解码支持 (`JsonCodec`)。 | No |
| `net` | 启用网络通信支持 (`HttpClient`, `WebSocket`, `SSE`)。 | No |

推荐在代码中导入 prelude：
```rust
use silex::prelude::*;
```

## 1. 路由系统 (Router)

Silex 提供了一个类型安全且易于使用的客户端路由。

### 基本用法

使用 `<Router>` 组件包裹您的应用，并定义路由规则：

```rust
fn App() -> impl Mount + MountRef {
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
                    Dynamic(move || {
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

### 查询参数与外部状态 (Query Parameters & Persistence)

Silex 提供了统一的持久化入口来处理 URL 查询参数、浏览器存储和双向绑定：

*   **`use_query_map()`**:
    *   返回 `Memo<HashMap<String, String>>`。
    *   使用 `web_sys::UrlSearchParams` 标准解析，自动处理 URI 编码。
    *   响应式：当 URL 变化时自动更新。

*   **`Persistent::builder(key)`**:
    *   统一后端：`.local()`、`.session()`、`.query()`。
    *   统一 codec：`.string()`、`.parse::<T>()`、`.json::<T>()`。
    *   返回 `Persistent<T>`，可直接 `get/set/update`，也可直接用于常见 View / `bind_value` 场景。

    ```rust
    let search = Persistent::builder("q")
        .query()
        .string()
        .default(String::new())
        .build();

    input()
        .bind_value(search);
    ```

## 2. 流程控制 (Flow Control)

Silex 提供了一组组件来处理常见的逻辑控制，这比手动编写 `move ||` 闭包更具可读性且性能更好。

### Show (条件渲染)
```rust
let (is_logged_in, set_log) = Signal::pair(false);

Show(is_logged_in)
    .children(UserDashboard())
    .fallback(LoginButton())
```
或者使用语法糖：
```rust
is_logged_in.when(UserDashboard())
```
`Show` 的 `children` 和 `fallback` 都是渲染型参数，传入普通 `View` 即可。

### Switch (多路分支)
类似于 `match` 语句，根据值选择渲染的内容。
```rust
let (tab, set_tab) = Signal::pair(0);

Switch(tab)
    .fallback(div("Fallback"))
    .case(0, TabA())
    .case(1, TabB())
```
`Switch` 会在构建阶段检查重复 `case` 值，并在分支未变化时避免重复重建。

### Portal (传送门)

将组件渲染到当前 DOM 树之外的节点（默认是 `document.body`）。适用于模态框（Modals）、全局通知、浮动菜单等。

**核心优势**：
*   **Context 连通**：即便 DOM 位于 body 下，依然能无缝访问定义处的响应式上下文（Signals, Context）。
*   **自动清理**：当 `Portal` 组件销毁时，它会自动从目标节点中移除渲染的内容。

```rust
Portal(div!(
    h2("我是模态框"),
    button("关闭")
))
.mount_to(custom_node) // 可选，默认为 body
```

### For (列表渲染)
高效渲染列表数据，支持 Keyed Diff 算法。

```rust
let (users, set_users) = Signal::pair(vec![
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
]);

For(
    users,           // 数据源 (Signal)
    |u| u.id,        // Key 提取函数 (必须唯一且稳定)
)
.children(|u, idx| div((idx.get(), ": ", u.name)))
.error(|err| div(format!("For 出错: {}", err)))
```
如果不传 `.error(...)`，默认会调用 `handle_error`。

### Index (索引列表渲染)
当列表项没有唯一 ID，或者列表项是基础类型（如 `Vec<String>`），或者列表长度固定仅内容变化时，使用 `Index` 比 `For` 更高效。它**复用** DOM 节点，仅更新 Signal。

```rust
let (logs, set_logs) = Signal::pair(vec!["Log 1", "Log 2"]);

Index(logs).children(|item, index| {
    // item 是 Signal<T>，内容变化时直接更新文本节点
    div((index, ": ", item))
})
```

## 3. 错误处理 (Error Handling)

使用 `<ErrorBoundary>` 可以捕获子组件树中的 **Panic** 或 **SilexError**。它能有效防止由于局部组件故障导致整个应用崩溃，并展示友好的备用 UI。

### 基本用法

```rust
ErrorBoundary(move || DangerousComponent())
    .fallback(|err| {
        div!(
            h3("糟糕，出错了"),
            p(format!("错误详情: {}", err)),
            button("重试").on_click(|_| {
                // 逻辑处理，例如刷新页面
                let _ = web_sys::window().unwrap().location().reload();
            })
        )
        .style("background: #fff1f0; border: 1px solid #ffa39e; padding: 16px; border-radius: 8px;")
    })
```

### 核心特性
*   **捕获同步 Panic**: 自动使用 `std::panic::catch_unwind` 包装子组件的渲染过程。
*   **捕获逻辑错误**: 捕获子组件通过 `ErrorContext` 向上冒泡的 `SilexError`（例如在事件处理器或异步 Resource 中抛出的错误）。
*   **状态隔离**: 错误界限会隔离故障，父级组件和其他不相关的组件树分支将保持正常工作。
*   **异步兼容**: 错误状态的更新是异步调度的，避免了在渲染阶段直接修改状态导致的潜在问题。

## 4. 异步加载 (Suspense)

配合 `Resource` 使用，优雅处理异步数据加载状态。

```rust
// 组件化 API
Suspense(move || {
    let data = Resource::new(source_signal, fetcher);
    div(rx!(data.get().unwrap_or_default()))
        .style("color: green")
})
.fallback(div("Loading..."))

// 卸载模式 (Unmount)
Suspense(move || {
    let data = Resource::new(source_signal, fetcher);
    div(rx!(data.get().unwrap_or_default()))
})
.mode(SuspenseMode::Unmount) // <--- 启用卸载模式
.fallback(div("Loading..."))
```

## 5. UI 与布局 (UI & Layout)

Silex 提供了一些基础的原子组件来迅速搭建响应式应用布局结构以及实现主题隔离机制：

### 布局组件 (Stack, Center, Grid)
内置实现了高度复用的布局原语，均已自动提供类型及属性信号响应绑定功能：
```rust
use silex::components::layout::*;

// 纵向 Flex，子元素以 10px 间隔
Stack(view_chain!(
    div("Child 1"),
    div("Child 2")
))
.gap(px(10))
.direction(FlexDirectionKeyword::Row) // 切换为横排

// 居中包围
Center(div("I am in the center"))

// 3 列网格网格
Grid(view_chain!(
    div("Cell 1"),
    div("Cell 2"),
    div("Cell 3"),
))
.columns(3)
.gap(px(8))
```

### 主题系统 (Theme System)

Silex 提供了一个能够与 CSS 变量无缝集成的强类型主题系统：

*   **强类型校验**：定义主题后自动生成的常量（如 `AppTheme::PRIMARY`）自带属性类型，防止将颜色误传给尺寸。
*   **全局模式**：使用 `set_global_theme(signal)` 为整个应用设置基础视觉方案。
*   **局部补丁**：使用 `theme_patch(patch_signal)` 进行增量微调，利用 CSS 变量继承实现精准局部覆盖。
*   **零损耗**：主题变量直接注入现有元素的属性中，不会引入额外的 DOM 包裹层。

```rust
// 1. 设置全局主题
set_global_theme(theme_signal);

// 2. 局部增量覆盖
// 仅修改 primary 变量，其余变量自动从环境继承
let patch = rx!(|| AppThemePatch::default().primary(hex("#ff69b4")));
div("局部变色卡片").apply(theme_patch(patch))

// 3. 在样式中使用主题变量 (具备 IDE 补全与类型检查)
sty().color(AppTheme::PRIMARY)
     .border_radius(AppTheme::RADIUS)
```

## 7. 网络请求 (Networking)

`silex` 提供了简洁且功能强大的 API 来处理 HTTP 请求、WebSocket 消息和 SSE 流。

### HTTP 请求 (HttpClient)

使用流式接口构建请求，并集成响应式系统：

```rust
// 1. 获取 JSON 数据并转化为 Resource (自动触发加载态)
let user_id = Signal::pair(1);
let user_data = HttpClient::get("https://api.example.com/users/{id}")
    .path_param("id", user_id)
    .json::<User>()
    .as_resource(user_id);

// 2. 提交数据 (Mutation)
let login = HttpClient::post("/api/login")
    .json_body(login_info)
    .json::<Token>()
    .as_mutation();

// 3. 配置重试与缓存
let api = HttpClient::get("/api/config")
    .retry_policy(3, Duration::from_secs(1))
    .cache(CachePolicy::StaleWhileRevalidate)
    .json::<Config>();
```

### WebSocket

提供状态和消息的完整响应式绑定：

```rust
let ws = WebSocket::connect("ws://localhost:8080/chat")
    .on_open(|| println!("Connected!"))
    .build();

// 获取实时消息信号 (自动 JSON 解码)
let messages = ws.message::<ChatMessage>();

// 发送消息
ws.send_json(&msg)?;
```

### Server-Sent Events (SSE)

```rust
let stream = EventStream::builder("/api/notifications")
    .event("update") // 监听特定事件
    .build();

// 获取最后一条消息
let last_msg = stream.last_message::<Notify>();
```

## 8. 常用宏与工具 (Macros & Utilities)

Silex 提供了一系列宏来简化开发，这些宏都已包含在 prelude 中。

### 组件定义 (`#[component]`)
```rust
#[component]
fn MyComp(name: String, #[prop(default)] age: i32) -> impl Mount + MountRef {
    div(format!("Name: {}, Age: {}", name, age))
}
```

### 属性助手 (`classes!`)
*   `classes!`: `div(()).class(classes!["btn", "active" => is_active])`

详细文档请参阅 [silex_macros 文档](../silex_macros/README.md)。
