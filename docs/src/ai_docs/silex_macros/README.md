# Crate: silex_macros

`silex_macros` 提供了 Silex 框架所需的编译时元编程能力，主要包括组件定义、CSS 处理、路由生成和状态管理。

## 1. 组件宏 `#[component]`

将函数转换为 View 组件，并自动生成 Props 结构体。

### 签名
```rust
#[component]
fn MyComponent(props...) -> impl View
```

### 转换逻辑
1.  **Parsing**: 解析函数签名，提取参数。
2.  **Struct Generation**: 生成 `MyComponentComponent` 结构体 (命名规则为 `{FnName}Component`)。
    *   **Fields**: 每个函数参数映射为一个结构体字段。
        *   REQUIRED: `Option<T>` (初始化为 None)。
        *   OPTIONAL (`#[prop(default)]`): `T` (初始化为 `Default::default()`).
    *   **Internal Fields**: `_pending_attrs: Vec<PendingAttribute>` 用于存储链式调用的各个属性。
    *   **Builder Methods**: 为每个字段生成链式调用方法 `fn prop_name(self, val: T) -> Self`。
3.  **Impl AttributeBuilder**:
    *   为组件结构体实现 `AttributeBuilder` Trait。
    *   允许组件直接调用 `.class()`, `.id()`, `.on_click()` 等方法。
    *   这些调用生成的属性被存储在 `_pending_attrs` 中。
4.  **Impl View**: 实现 `View` trait。
    *   **Mount**:
        1.  运行时检查 REQUIRED 字段是否为 `Some`，否则 Panic。
        2.  解构 Props。
        3.  调用原始函数体获取 View 实例。
        4.  **Attribute Forwarding**: 调用 `view_instance.apply_attributes(_pending_attrs)`，将属性传递给内部视图。
        5.  挂载 View 实例。
5.  **Constructor**: 生成同名函数 `fn MyComponent() -> MyComponentComponent` 作为入口。

### 属性支持
*   `#[prop(default)]`: 使用 `Default::default()` 填充默认值。
*   `#[prop(default = expr)]`: 使用指定表达式填充默认值。
*   `#[prop(into)]`: 自动调用 `.into()`，支持 `impl Into<T>`。
    *   **自动推导**: 如果类型是 `Children`, `AnyView`, `String`, `PathBuf`, `Callback`, `Signal`，宏会自动开启 `into` 行为。

---

## 2. CSS 宏 `css!`

编译时 CSS 处理与注入。

### 工作流
1.  **Input**: CSS 字符串字面量。
2.  **Hashing**: 计算内容 Hash，生成唯一类名 `slx-{hash}`。
3.  **Scoping**: 将 CSS 内容包裹在 `.slx-{hash} { ... }` 中。
4.  **Validation & Minification**: 使用 `lightningcss` 解析、验证并压缩 CSS。
5.  **Codegen**:
    *   生成 `silex::css::inject_style("style-slx-{hash}", "{css_content}")` 调用。
    *   返回类名字符串 `"slx-{hash}"`。

---

## 3. 路由宏 `#[derive(Route)]`

为 Enum 自动实现 `Routable` 和 `RouteView` Traits。

### 核心机制

#### `fn match_path(path: &str) -> Option<Self>`
*   **Segment Matching**: 将路径按 `/` 分割。
*   **Static Segment**: 字符串精确匹配。
*   **Param Segment (`:id`)**: 尝试解析为目标字段类型 (`ident.parse()`)。
*   **Wildcard (`*`)**: 匹配剩余所有内容。
*   **Nested Route**:
    *   识别 `#[nested]` 标记的字段。
    *   递归调用子路由的 `match_path`，传入剩余路径段。

#### `fn to_path(&self) -> String`
*   根据 Enum Variant 的字段值反向构建 URL 字符串。
*   自动处理嵌套路由的路径拼接 (`/base/child`).

#### `fn render(&self) -> AnyView`
需要 `#[route(..., view = ComponentFunction)]`。
可选 `#[route(..., guard = GuardComponent)]` 或 `#[route(..., guard = [OuterGuard, InnerGuard])]`。

*   **Guard Wrapping**:
    *   宏会读取 `guard` 参数（单个 Path 或 Path 列表）。
    *   在生成渲染代码时，View 表达式会被 Guard 组件层层包裹。
    *   包裹顺序：列表定义的顺序即为执行/嵌套顺序。`guard = [A, B]` -> `A(children=B(children=View))`.
    *   代码生成逻辑中使用 `.rev()` 迭代 guards，通过 `quote!` 不断包裹 `view_expr`。
*   **Binding**: 将 Enum Variant 的字段映射为 Component 的 props。
    *   要求 Variant 字段名与 Component Prop 名一致。
    *   自动调用 `.clone()`。
*   **Fallback**: 若无 view，返回 `()` (Empty View).

---

## 4. 状态宏 `#[derive(Store)]`

将普通 Struct 转换为细粒度响应式 Store，并生成 Context 管理代码。

### 属性支持 (`#[store(...)]`)
*   `name = "fn_name"`: 指定生成的 Hook 函数名称（默认为 `use_{snake_case_struct_name}`）。
*   `err_msg = "message"`: 指定 Context 缺失时的 Panic 消息。

### 转换逻辑
输入 Struct:
```rust
#[derive(Store)]
#[store(name = "use_user")]
struct User { name: String, age: i32 }
```
输出 Store Struct 及辅助代码:
```rust
// 1. 生成 Store 结构体 (字段为 RwSignal)
#[derive(Clone, Copy)]
struct UserStore {
    pub name: RwSignal<String>,
    pub age: RwSignal<i32>,
}

impl UserStore {
    // 初始化
    pub fn new(source: User) -> Self { ... }
    // 获取快照
    pub fn get(&self) -> User { ... }
}

// 2. 实现 Store Trait
impl ::silex::store::Store for UserStore {
    fn get() -> Self {
        use_context::<Self>().expect("Context for UserStore not found")
    }
}

// 3. 生成 Hook 函数
fn use_user() -> UserStore {
    <UserStore as ::silex::store::Store>::get()
}
```

### 核心机制
*   **Struct Wrapping**: 原始结构体的每个字段 `T` 被映射为 `RwSignal<T>`。
*   **Context Integration**: 通过实现 `Store` trait，获得 `provide()` 能力。
*   **Ergonomic Hook**: 自动生成全局函数（如 `use_user`），封装了 `use_context` 和错误处理逻辑，提供类似 React Hooks 的体验。

---

## 5. 辅助宏

### `style!`
*   语法: `style! { "color": "red", width: "100px" }`
*   输出: `silex::dom::attribute::group(("color", "red"), ("width", "100px"))`

### `classes!`
*   语法: `classes![ "btn", "active" => is_active ]`
*   输出: `silex::dom::attribute::group("btn", ("active", is_active))`

---

## 6. Clone 宏 `clone!`

简化闭包场景下的变量克隆。

### 用法
```rust
let data = vec![1, 2, 3];
let callback = clone!(data => move || {
    println!("{:?}", data);
});
```

### 转换逻辑
1.  **Input**: 变量列表 + `=>` + 表达式（通常是闭包）。
2.  **Expansion**:
    *   对列表中的每个变量生成 `let var = var.clone();`。
    *   将这些克隆语句置于新的块中，后跟原始表达式。
    *   注意：生成的变量会 shadow 外部变量，这在 `move` 闭包前非常有用。

### 内部克隆 (Inner Clone)

*   **Syntax**: `clone!(ident, @inner_ident => ...)`
*   **Behavior**:
    *   `ident`: 仅生成外部 `let ident = ident.clone();`。
    *   `@inner_ident`:
        1.  生成外部 `let inner_ident = inner_ident.clone();` (用于捕获)。
        2.  这是关键：如果 `=>` 后是闭包，宏会解析闭包体，并在其开头注入 `let inner_ident = inner_ident.clone();`。
*   **Use Case**: 适用于 `FnMut` 闭包中需要消费（consume/move）捕获变量的场景，确保每次闭包调用都有新的克隆副本可用，避免 `use of moved value` 错误。
