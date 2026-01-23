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
2.  **Struct Generation**: 生成 `MyComponentProps` 结构体。
    *   **Fields**: 每个函数参数映射为一个结构体字段。
        *   REQUIRED: `Option<T>` (初始化为 None)。
        *   OPTIONAL (`#[prop(default)]`): `T` (初始化为 `Default::default()`).
    *   **Builder Methods**: 为每个字段生成链式调用方法 `fn prop_name(self, val: T) -> Self`。
3.  **Impl View**: 实现 `View` trait。
    *   **Mount**:
        1.  运行时检查 REQUIRED 字段是否为 `Some`，否则 Panic。
        2.  解构 Props。
        3.  调用原始函数体获取 View 实例。
        4.  挂载 View 实例。
4.  **Constructor**: 生成同名函数 `fn MyComponent() -> MyComponentProps` 作为入口。

### 属性支持
*   `#[prop(default)]`: 使用 `Default::default()` 填充默认值。
*   `#[prop(default = expr)]`: 使用指定表达式填充默认值。
*   `#[prop(into)]`: 自动调用 `.into()`，支持 `impl Into<T>`。
    *   **自动推导**: 如果类型是 `Children`, `AnyView`, `String`, `PathBuf` 或 `Callback`，宏会自动开启 `into` 行为。

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
*   **Binding**: 将 Enum Variant 的字段映射为 Component 的 props。
    *   要求 Variant 字段名与 Component Prop 名一致。
    *   自动调用 `.clone()`。
*   **Fallback**: 若无 view，返回 `()` (Empty View).

---

## 4. 状态宏 `#[derive(Store)]`

将普通 Struct 转换为细粒度响应式 Store。

### 转换逻辑
输入 Struct:
```rust
struct User { name: String, age: i32 }
```
输出 Store Struct:
```rust
struct UserStore {
    pub name: RwSignal<String>,
    pub age: RwSignal<i32>,
}
```

### 方法
*   `new(source: User) -> UserStore`: 使用 `RwSignal::new` 初始化每个字段。
*   `get(&self) -> User`: 读取所有 Signal 的当前值并重组为原始 Struct (Snapshot)。

---

## 5. 辅助宏

### `style!`
*   语法: `style! { "color": "red", width: "100px" }`
*   输出: `silex::dom::attribute::group(("color", "red"), ("width", "100px"))`

### `classes!`
*   语法: `classes![ "btn", "active" => is_active ]`
*   输出: `silex::dom::attribute::group("btn", ("active", is_active))`
