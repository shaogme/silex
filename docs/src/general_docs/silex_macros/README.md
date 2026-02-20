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
Button()
    .label("Click me") // 必须
    .opacity(0.8)      // 可选
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
如果组件返回多个根节点（例如返回元组或 `Fragment`），属性会采用**首个匹配策略**：属性会被转发给第一个能消费属性的子节点（通常是第一个 DOM 元素），后续节点不受影响。

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
let (color, _) = signal("white".to_string());
let scale = signal(1.0).0;

let btn_class = css!(r#"
    background-color: blue;
    color: $(color); /* 支持动态 Signal 插值 */
    transform: scale($(scale)); /* 自动处理任何实现了 IntoSignal 的类型 */
    padding: 10px;

    &:hover {
        background-color: darkblue;
    }
"#);

button(()).class(btn_class).text("Styled Button")
```

宏会返回一个唯一的类名（如 `slx-1a2b3c`），并将样式自动注入到页面 `<head>` 中。

## 3. 类型安全路由 (`#[derive(Route)]`)

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

    // 支持多个 Guard，执行顺序由外向内: LogGuard -> AuthGuard -> View
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
        .unwrap_or(signal("Guest".to_string()).0);
    
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

## 4. 全局状态 Store (`#[derive(Store)]`)

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

## 5. 样式与类名助手

### `style!`
快速生成内联样式元组。
```rust
div(())
    .style(style! {
        "color": "red",
        "margin-top": "10px"
    })
```

### `classes!`
动态生成类名列表。
```rust
div(())
    .attr("class", classes![
        "container",
        "is-active" => is_active_signal.get() // 仅当 true 时添加
    ])
```

## 6. 简化变量克隆 (`clone!`)

在编写回调函数（Callback）或副作用（Effect）时，经常需要将外部变量的所有权移动到闭包中，但又希望保留外部变量的引用以供他用。传统的做法是手动克隆：

```rust
let name = name_signal.clone();
let age = age_signal.clone();
let callback = move || {
    println!("Name: {}, Age: {}", name.get(), age.get());
};
```

使用 `clone!` 宏可以大大简化这一过程：

```rust
let callback = clone!(name_signal, age_signal => move || {
    println!("Name: {}, Age: {}", name_signal.get(), age_signal.get());
});
```

宏会自动生成 `let variable = variable.clone();` 语句，并将其包裹在一个新的作用域中，使得闭包捕获的是克隆后的变量。

### 内部克隆 (Inner Clone)

如果闭包是 `FnMut` 且你在闭包内部 `move`（消耗）了变量的所有权（例如传给 `async move` 块），你需要确保每次执行闭包时都拥有该变量的独立副本。

使用 `@` 前缀可以指示宏除了在闭包外部克隆一次（用于捕获），还在闭包内部的最前端再次注入克隆语句。

```rust
// id 需要被消费（传递给 add_project），但闭包本身会被多次调用
let callback = clone!(store, @id => move |_| {
    // 宏会自动在此处生成: let id = id.clone();
    store.add_project(id); 
});
```
