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

### 查询参数 (Query Parameters)

Silex 提供了方便的 Hooks 来处理 URL 查询参数：

*   **`use_query_map()`**:
    *   返回 `Memo<HashMap<String, String>>`。
    *   使用 `web_sys::UrlSearchParams` 标准解析，自动处理 URI 编码。
    *   响应式：当 URL 变化时自动更新。

*   **`use_query_signal(key)`**:
    *   实现了 **双向绑定**。
    *   返回 `RwSignal<String>`。
    *   **读**: 读取 URL 中的参数值。
    *   **写**: 修改 Signal 会自动更新 URL (pushState) 并触发导航。
    *   **防抖/防循环**: 内部实现了智能的循环检测，只有当值真正变化时才同步，避免无限循环和重复导航。

    ```rust
    // 示例：将输入框绑定到 ?q=...
    let search = use_query_signal("q");
    
    input(())
        .bind_value(search) // 双向绑定到 input value
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
    // item 是 Signal<T>，内容变化时直接更新文本节点
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
use silex::components::{suspense, SuspenseBoundary, SuspenseMode};

// Builder 模式 (推荐)
suspense()
    .resource(|| Resource::new(source_signal, fetcher))
    .children(move |data| {
        SuspenseBoundary::new()
            .fallback(|| div("Loading..."))
            .children(move || div(data.get()))
    })

// 卸载模式 (Unmount)
suspense()
    .resource(|| Resource::new(source_signal, fetcher))
    .children(move |data| {
        SuspenseBoundary::new()
            .mode(SuspenseMode::Unmount) // <--- 启用卸载模式
            .fallback(|| div("Loading..."))
            .children(move || div(data.get()))
    })
```

## 5. UI 与布局 (UI & Layout)

Silex 提供了一些基础的原子组件来迅速搭建响应式应用布局结构以及实现主题隔离机制：

### 布局组件 (Stack, Center, Grid)
内置实现了高度复用的布局原语，均已自动提供类型及属性信号响应绑定功能：
```rust
use silex::components::layout::*;

// 纵向 Flex，子元素以 10px 间隔
Stack((
    div("Child 1"),
    div("Child 2")
))
.gap(10)
.direction(FlexDirectionKeyword::Row) // 切换为横排

// 居中包围
Center(div("I am in the center"))

// 3 列网格网格
Grid((
    div("Cell 1"),
    div("Cell 2"),
    div("Cell 3"),
))
.columns(3)
.gap(8)
```

### 主题注入 (Theme System)
为了解决深层组件组件主题透传问题且杜绝包裹产生多余的 `<div class="theme-provider">` DOM 节点致使 `Flex/Grid` 失效：
```rust
// 通过宏预先构建具有强类型校验保障的系统 （细节见 silex_macros 文档）
#[theme(MyTheme)]
struct AppTheme { ... }

let my_theme_signal = signal(AppTheme { ... });

// 1. 全局生效方案:
set_global_theme(my_theme_signal); 

// 2. 将主题直接应用（注入 CSS Vars）到已经建立在流里的组件上进行范围挂载:
Stack(...)
    .apply(theme_variables(my_theme_signal))
```

## 6. 常用宏与工具 (Macros & Utilities)

Silex 提供了一系列宏来简化开发，这些宏都已包含在 prelude 中。

### 组件定义 (`#[component]`)
```rust
#[component]
fn MyComp(name: String, #[prop(default)] age: i32) -> impl View {
    div(format!("Name: {}, Age: {}", name, age))
}
```

### CSS 编写 (`css!` 与 `styled!`)
Silex 拥有极为强大的**原生态类型安全 CSS 体系**！避免了一般框架所面临的字符串拼接引发的各类不安全 CSS Bug。由于其杜绝了通配符隐式字符串转化逃逸，您需要显式地通过我们提供的 Builder 或 Enum 类型组合属性。

**基础插值：**
```rust
use silex::css::types::{px, pct, hex};

let w = signal(px(100));
let c = signal(hex("#ff0000"));

let cls = css!("
    color: $(c); 
    width: $(w); /* 编译期类型校验，保障不会错写成单纯数字或者错用其他强单位 */
    &:hover { color: blue; }
");
div("Hello").class(cls)
```
**性能注记：** 所有的动态插值 $(...) 现在大部分都通过 **CSS 变量 (CSS Variables)** 进行高效更新。这意味着当信号变化时，框架仅调用一次极轻量的 `style.setProperty`，而无需操作 DOM 结构，在高频更新场景下性能表现极其卓越。
对于无法用内联变量表示的插值（例如嵌套伪类中的动态值），Silex 内置了拥有**引用计数 (Reference Counting)** 及 **LRU 缓存回收**机制的 `DynamicStyleManager`，它会在后台自动计算哈希生成独特类名并复用 `<style>` 标签，既实现了无死角的完全响应式，又使得长期运行的应用不至于出现 `<style>` DOM 节点污染与内存溢出。

**复杂复合类型（工厂与 Builders）：**
使用专用模块工厂快速安全打包例如 `margin`，`border` 等复合元素。
```rust
use silex::css::types::{border, padding, BorderStyleKeyword};

let border_style = signal(border(px(1), BorderStyleKeyword::Solid, hex("#ccc")));
let pad = signal(padding::x_y(px(8), px(16)));

styled! {
    pub StyledDiv<div>(
        #[prop(into)] p_val: Signal<UnsafeCss>, // 如果确实需要越过系统拦截
    ) {
        border: $(border_style);
        padding: $(pad);
        margin: $(p_val);

        variants: {
            size: {
                small: { font-size: 12px; }
                large: { font-size: 20px; }
            }
        }
    }
}
```

### 样式构建器 (Style Builder)

除了宏，Silex 还提供了一套纯 Rust 的样式构建 API，适用于希望完全避免过程宏开销、或需要极致类型安全提示的场景。

```rust
use silex::css::builder::Style;
use silex::css::types::{px, hex, DisplayKeyword};

let (width, _) = signal(px(200));

div("I am styled by Builder")
    .style(
        Style::new()
            .display(DisplayKeyword::Flex)
            .width(width) // 支持响应式信号
            .background_color(hex("#f0f0f0"))
            .padding(px(20))
            .on_hover(|s| { // 支持伪类
                s.background_color(hex("#e0e0e0"))
                 .color(hex("#00bfff"))
            })
    )
```

**对比优势：**
*   **零开销**：不使用过程宏进行字符串解析，纯泛型展开，编译速度极快。
*   **强类型提示**：Rust Analyzer 可以准确提示每一个属性的合法参数（如 `Display` 只能传 `DisplayKeyword` 枚举）。
*   **CSS 变量级优化**：静态样式自动提升到 `<head>` 共享；动态属性自动绑定为 CSS 变量，通过响应式 Effect 进行原子化更新，避免重绘抖动。

### 属性助手 (`style!`, `classes!`)
*   `style!`: `div(()).style(style! { "color": "red", "margin": "10px" })`
*   `classes!`: `div(()).class(classes!["btn", "active" => is_active])`

详细文档请参阅 [silex_macros 文档](../silex_macros/README.md)。
