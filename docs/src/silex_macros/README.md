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

// 使用
Button()
    .label("Click me") // 必须
    .opacity(0.8)      // 可选
```

## 2. 编写 CSS (`css!`)

使用 `css!` 宏可以在 Rust 代码中直接编写 CSS，并享受自动哈希（Scoped CSS）和压缩功能。

```rust
let btn_class = css!(r#"
    background-color: blue;
    color: white;
    padding: 10px;

    &:hover {
        background-color: darkblue;
    }
"#);

button(()).class(btn_class).text("Styled Button")
```

宏会返回一个唯一的类名（如 `slx-1a2b3c`），并将样式自动注入到页面 `<head>` 中。

## 3. 类型安全路由 (`#[derive(Route)]`)

通过宏自动从 Enum 生成路由匹配和渲染逻辑。

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

## 4. 全局状态 Store (`#[derive(Store)]`)

快速创建深层响应式的数据结构。

```rust
#[derive(Clone, Default)]
struct UserConfig {
    theme: String,
    notifications: bool,
}

#[derive(Store, Clone, Copy)]
struct GlobalStore {
    config: UserConfig, // 注意：derive(Store) 目前只展开一层 Struct
                        // 若需嵌套，建议扁平化或手动组合
}
```
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
