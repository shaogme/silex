# Silex 宏工具箱

`silex_macros` 包含了一系列过程宏，旨在减少样板代码，提升开发效率。

## 1. 定义组件 (`#[component]`)

使用 `#[component]` 宏可以将普通函数转换为功能强大的组件构造器。

```rust
#[component]
fn Button(
    // 必填参数
    label: String,
    // 可选参数，默认值为类型的 Default
    #[prop(default)] color: String, // 默认为 ""
    // 可选参数，指定默认值
    #[prop(default = 1.0)] opacity: f64,
    // 自动调用 .into()，接受 &str 等
    #[prop(into)] on_click: Option<Callback<()>>,
) -> impl View {
    button(())
        .style(format!("opacity: {}", opacity))
        .text(label)
}

```rust
// 使用
// 由于 label 不是 children 参数，所以依然使用无参构造 + 链式调用
Button()
    .label("Click me") // 必须
    .opacity(0.8)      // 可选

// 如果第一个参数是 children: Children，则必须使用
// Parent(div("child"))
// 代替
// Parent().children(div("child"))
```

### 属性透传 (Attribute Forwarding)

生成的组件结构体实现了 `AttributeBuilder` Trait，这意味着你可以像操作普通 DOM 元素一样操作组件！

所有标准的 DOM 方法（如 `.class()`, `.id()`, `.style()`, `.on_click()`）都可以直接链式调用：

```rust
Button()
    .label("Submit")
    .class("my-btn")       // 透传给 Button 内部的根元素
    .on_click(|_| { ... }) // 透传点击事件
```

**多根节点 (Fragments) 支持：**
如果组件返回多个根节点，属性会采用**首个匹配策略**：属性会被转发给第一个能消费属性的子节点（通常是第一个 DOM 元素），后续节点不受影响。

### 泛型与生命周期支持

`#[component]` 宏原生支持复杂的泛型和生命周期参数。这意味着你可以定义接受多态类型或带有特定生命周期的引用的组件：

```rust
#[component]
pub fn GenericMessage<'a, T: std::fmt::Display + Clone + 'static>(
    value: T,
    title: &'a str,
) -> impl View {
    div![
        h4(title.to_string()),
        p(format!("Value: {}", value)),
    ]
}

// 使用方式：
GenericMessage()
    .value(42)  // 推导为 i32
    .title("Number") // &'static str
```

在底层生成组件的 Builder 时，宏会自动处理相关的生命周期和泛型类型，并通过注入 `PhantomData` 来确保编译器正确追踪未使用（unused）但在宏块签名前声明了的参数。

## 2. 编写 CSS (`css!`)

使用 `css!` 宏可以在 Rust 代码中直接编写 CSS，并享受自动哈希（Scoped CSS）和压缩功能。

```rust
let (color, _) = Signal::pair("white".to_string());
let scale = Signal::pair(1.0).0;

let btn_class = css! {
    background-color: blue;
    color: $(color); /* 支持动态 Signal 插值 */
    transform: scale($(scale)); /* 自动处理任何实现了 IntoSignal 的类型 */
    padding: 10px;

    &:hover {
        background-color: darkblue;
    }
};

button(()).class(btn_class).text("Styled Button")
```

宏会返回一个唯一的类名（如 `slx-1a2b3c`），并将样式自动注入到页面 `<head>` 中。

**高级类型校验 (Compile-time Type Safety)：**
`css!` 和 `styled!` 宏原生支持编译期类型安全。它们会自动感知插值所处的 CSS 属性名（如 `width`），并限制传入信号或变量的值类型。配合 `silex::css::types::props` 和如 `px(100)`, `pct(50)` 这样的包装类，能够完美防范因忘记写单位引发的 CSS 无效问题：

```rust
use silex::css::types::{px, pct};
use silex::css::types::{border, BorderStyleKeyword, UnsafeCss, hex};

let w = Signal::pair(px(100)); // Px 类型被限定允许给 Width
let bd = Signal::pair(border(px(1), BorderStyleKeyword::Solid, hex("#ccc"))); // 专属工厂函数保障多位组合安全
let custom_calc = Signal::pair(UnsafeCss::new("calc(100% - 20px)")); // 若需超出约束边界请显式包装

let cls = css! {
    width: $(w); /* ✅ 合规 */
    height: $(pct(50.0)); /* ✅ 合规 */
    border: $(bd); /* ✅ 单值化强类型复合体合规 */
    margin: $(custom_calc); /* ⚠️ 显式越权非安全逃逸 */
    /* color: $(123.45); ❌ 编译报错：the trait `ValidFor<Color>` is not implemented for `f64` */
    /* z-index: $(px(99)); ❌ 编译报错：拦住企图把像素单位送给 ZIndex 的不合规行为 */
    /* padding: $("10px 20px"); ❌ 编译报错：阻绝散乱的字符串拼接（除非用 UnsafeCss 或是 padding::x_y 构建器）*/
};
```

**底层解析重构 (AST-driven Compiler)**：
`css!` 的内部机制基于强大的强类型解析引擎。首先由专用语法解析树（`ast.rs`）利用 `syn` 将输入 Token 流递归解析为 `CssDeclaration`、`CssNested` 及 `CssAtRule`（支持 `@media` 等）语法单元。其次交由 `CssCompiler` 进行语义提取：
*   **静态压缩**：通过 `lightningcss` 进行极致压缩和语法验证。
*   **Token 间隙优化**：编译器内置了智能间隙提取逻辑，确保诸如 `font-family` 或自定义值中的标识符与字面量之间保留正确的空格。
*   **自动类型映射**：宏会自动将 kebab-case 的属性名（如 `background-color`）映射到运行时的 PascalCase 类型标签（如 `props::BackgroundColor`），实现无感知识库同步的编译期验证。

## 3. 样式组件 (`styled!`)

使用 `styled!` 宏可以带来类似 `styled-components` 的极致开发体验。它允许直接定义带作用域样式的组件，免去手写类名绑定，并且原生支持变体 (Variants) 和 **局部动态规则 (Dynamic Rules)**。

```rust
styled! {
    pub StyledButton<button>(
        children: Children,
        #[prop(into)] color: Signal<String>,
        #[prop(into)] hover_color: Signal<String>,
        #[prop(into)] size: Signal<String>,
        #[prop(into)] pseudo_state: Signal<String>,
    ) {
        background-color: rgb(98, 0, 234);
        color: $(color); /* 基础值插值 */
        padding: 8px 16px;
        border-radius: 4px;
        border: none;
        cursor: pointer;
        transition: transform 0.1s, color 0.2s, background-color 0.2s;

        /* 动态规则插值：连选择器和部分块属性也能被 Signal 控制！*/
        &:$(pseudo_state) {
            background-color: $(hover_color);
            transform: scale(1.05);
        }

        // 静态变体 (Variants) 支持，通过纯类名直接切换响应无需 CSS 变量分配。
        variants: {
            size: {
                small: { padding: 4px 8px; font-size: 12px; }
                medium: { padding: 8px 16px; font-size: 14px; }
                large: { padding: 12px 24px; font-size: 18px; }
            }
        }
    }
}

// 在任意组件中透明且类型安全地使用：
// 由于 children 是 StyledButton 的第一个参数，它可以直接传入构造函数
StyledButton("Click me!")
    .color(my_color)
    .hover_color("#ff4081")
    .pseudo_state("active") // 可以按需改变触发条件！
    .size("large")
    .class("additional-external-classes") // 完全享受透传能力
    .on(event::click, move |_| console_log("Clicked!"))
```

**核心优势**：
1.  **脱糖直接兼容 `#[component]`**：生成的组件会自动返回基础节点构建并且注入所需属性重载和 `_pending_attrs`，完美支持外部 `.class()`, `.id()`, `.on_click()` 等链式方法调用重写。
2.  **动态规则与纯享原生能力**：允许使用 `&:$(pseudo)` 的超强局部动态注入技术，这意味着我们可以安全地将 Signal 应用于伪类、伪元素乃至媒体查询触发值的热更新上！
3.  **纯静态性能级变体 Variants**：对于非连续动画类的多属性集合变化（如主/从色彩模式、按键大中小模式），使用纯 CSS 类生成的 Variant 来规避运行时频繁覆盖及下发样式的系统开销。

## 4. 类型安全路由 (`#[derive(Route)]`)

通过宏自动从 Enum 生成**基于 Radix Tree 的高性能**路由匹配和渲染逻辑。

```rust
#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    // 静态路径
    #[route("/", view = Home)]
    Home,

    // 带参数路径 (:id 会映射到字段 id)
    #[route("/user/:id", view = UserProfile)]
    User { id: String },

    // 嵌套路由
    #[route("/admin")]
    Admin(
        #[nested] AdminRoute // AdminRoute 也是一个实现了 Routable 的 Enum
    ),

    // 404 捕获
    #[route("/*", view = NotFound)]
    NotFound,
}
```

### 路由守卫 (Route Guards)

你可以为路由添加 `guard` 参数来拦截或包装路由渲染。Guard 本质上是一个接收 `children` 的组件（Middleware）。

```rust
#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/dashboard", view = Dashboard, guard = AuthGuard)]
    Dashboard,

    // 支持多个 Guard，执行顺序由外向内: LogGuard -> AuthGuard -> Mount
    #[route("/admin", view = AdminPanel, guard = [LogGuard, AuthGuard])]
    Admin,
}
```

**Guard 组件示例：**

```rust
#[component]
pub fn AuthGuard(children: Children) -> impl View {
    // 假设我们有一个全局用户状态
    let user_name = use_context::<ReadSignal<String>>()
        .unwrap_or(Signal::pair("Guest".to_string()).0);
    
    move || {
         if user_name.get() != "Guest" {
             // 验证通过，渲染子视图
             children.clone()
         } else {
             // 验证失败，显示提示或重定向
             div![
                 h3("🔒 Restricted Access"),
                 p("Please login to view this content."),
             ].style("color: red; border: 1px solid red; padding: 10px;")
             .into_any()
         }
    }
}
```

## 5. 全局状态 Store (`#[derive(Store)]`)

快速创建深层响应式的数据结构，并自动生成 Context 访问钩子。

```rust
#[derive(Clone, Default)]
struct UserConfig {
    theme: String,
    notifications: bool,
}

#[derive(Store, Clone, Copy)]
#[store(name = "use_config", err_msg = "Config not found")]
struct GlobalStore {
    pub config: UserConfig, // 注意：derive(Store) 目前只展开一层 Struct
                            // 若需嵌套，建议扁平化或手动组合
}
```

### 自动生成的代码

宏会自动生成以下内容：

1.  **响应式结构体** `GlobalStoreStore`：所有字段被包装在 `RwSignal` 中。
2.  **构造方法** `GlobalStoreStore::new(source: GlobalStore)`。
3.  **快照方法** `GlobalStoreStore::get(&self) -> GlobalStore`。
4.  **Store Trait 实现**：实现 `silex::store::Store`，提供 `provide()` 等方法。
5.  **全局 Hook**：`use_config()` (根据 `name` 属性或默认生成 `use_global_store`)。

### 使用示例

```rust
// 1. 在根组件提供 Store
let config = UserConfig::default();
let store = GlobalStoreStore::new(GlobalStore { config });
store.provide(); // 注入 Context

// 2. 在子组件使用生成的 Hook 获取
let store = use_config();
let theme_signal = store.config; // RwSignal<UserConfig>
```

### 属性参数 (`#[store(...)]`)

*   `name`: 自定义生成的 Hook 函数名（默认为 `use_{snake_case_struct_name}`）。
*   `err_msg`: 自定义 Context 缺失时的 Panic 信息。

*注意：目前的 implementation 只是简单的字段 Signal 化，对于嵌套结构需要组合使用。*

## 6. 样式与类名助手

### `classes!`
动态生成类名列表。
```rust
div(())
    .attr("class", classes![
        "container",
        "is-active" => is_active_signal.get() // 仅当 true 时添加
    ])
```

## 7. 强类型主题系统 (`theme!`)

Silex 提供了高度集成的强类型主题系统，保障在 CSS 中使用主题变量时的类型安全。

### 定义主题

使用 `theme!` 声明具有严格类型约束的主题结构：

```rust
theme! {
    #[theme(main, prefix = "slx")] // 使用 main 标记为主主题，供其他宏自动关联
    pub struct AppTheme {
        pub primary_color: silex::css::types::props::Color,
        pub base_padding: silex::css::types::props::Padding,
    }
}
```

宏会自动生成：
1. `AppTheme` 结构体。
2. 隐藏的内部 Trait（如 `AppThemeFields`）及其实现，以便在编译期抽取验证每个字段的类型。
3. 实现 `ThemeType` 和 `ThemeToCss`，支持自动将字段展开为原生的全量 CSS 变量 (`--slx-theme-primary_color`) 提供给 DOM 树。

### 与样式组件紧密结合

在 `styled!` 宏定义的组件中，你可以通过 `$Path::TO::CONST` 语法（通常是 `$AppTheme::FIELD`）直接引用主题库中的值。这种方式利用了 Rust 原生的路径解析，提供了绝对的鲁棒性。

```rust
styled! {
    pub ThemedBox<div>(
        children: Children,
    ) {
        // 直接使用 $AppTheme:: 引用常量
        // 系统在编译期利用 Rust 的类型系统强检查字段类型是否适用于属性！
        padding: $AppTheme::BASE_PADDING; 
        background-color: $AppTheme::PRIMARY_COLOR;
        border-radius: 8px;
    }
}
```

`styled!` 宏的编译器会自动识别 `$` 后跟随的 Rust 路径。它不仅能正确解析常量引用的 CSS 变量值，还能在编译期直接捕获类型错误。

> [!IMPORTANT]
> **语法迁移提示**：旧有的 `$theme.field` 语法现已彻底移除。如果你在代码中继续使用它，编译器将抛出一个友好的错误提示，引导你迁移到新的路径语法。
