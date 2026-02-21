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
    *   **Internal Fields**: `_pending_attrs: Vec<PendingAttribute>` 用于存储链式调用的各个属性。此外还会提取出所有的泛型和生命周期，并在组件结构体注入 `_phantom: std::marker::PhantomData<fn() -> (Generics...)>`，以完美支持函数声明了泛型或生命周期但未直接在参数字段中使用时带来的 `unused parameter` 各类潜在错误。
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
1.  **Input**: CSS 块，包含各种 CSS 规则和 `$(expr)` 动态值插值。
2.  **AST Parsing (`ast.rs`)**:
    *   使用 `syn` 将输入的 TokenStream 解析为强类型的 CSS 抽象语法树 (`CssBlock`, `CssRule`, `CssDeclaration`, `CssNested`, `CssAtRule`, `CssValue`)。
3.  **Processing & Tracking (`compiler.rs`)**:
    *   遍历 AST，在 `compiler.rs` 中进行语义处理，将静态 Token 拼接为字符串，并提取 `CssValue::Dynamic`。
    *   感知所处的 CSS 属性上下文以供后续强类型检查。
    *   在提取出的静态 CSS 模板中将插值替换为 CSS 变量占位符 `--slx-tmp-{index}`（然后再统一替换为带 Hash 的 `--slx-{hash}-{index}`）。
    *   将包含动态插值的嵌套规则提取为局部的动态规则分片 (`DynamicRule`)。
4.  **Hashing**: 计算输入 TokenStream 的 Hash，生成类名 `slx-{hash}` 及其对应的样式隔离域。
5.  **Scoping**: 根据哈希将全局的 CSS 类包裹在 `.slx-{hash} { ... }` 之中。
6.  **Validation & Minification**: 使用 `lightningcss` 解析、验证并压缩提取好的静态样式组合。
7.  **Codegen & Type Checking**:
    *   生成 `silex::css::inject_style` 调用。
    *   **自动类型推导**: 宏内部 `get_prop_type` 方法通过字符串处理实现 `kebab-case` 到 `PascalCase` 的映射。例如插值处属性名为 `font-size`，则自动映射到 `props::FontSize`。
    *   通过生成的代码块调用 `make_dynamic_val_for::<P, S>` 时，实施非常严密的基于 `ValidFor<P>` 类型的编译期类型断言检查。
    *   **Token 拼接与间隙策略**: 在 `append_token_stream_strings` 中实现了对 Ident 和 Literal 连续出现时的自动空格补全（Space Padding），保证生成的 CSS 语义正确。
    *   **杜绝隐式逃逸与使用 `UnsafeCss`**：彻底移除针对 `&str`/`String` 类型提供的泛用兜底验证。任何脱离基础包裹类型或工厂构建器的越权插入，必须由开发者显式封装为 `UnsafeCss::new(...)`，宏引擎与运行时将对其安全放行。
    *   **复合复合工厂**：对于 `border` 等属性不再尝试在宏内部做危险的字面量混排切分，而是统一要求接收诸如 `border()` 或 `margin::x_y()` 等原生 Rust 函数验证安全后生成的特定结果进行单点插值。
    *   若**无动态值**：返回静态类名字符串 `"slx-{hash}"`。
    *   若**有动态值**：返回 `DynamicCss` 结构体，包含类名和变量更新闭包列表 (Updaters)。

---

## 3. 样式组件宏 `styled!`

提供 "CSS-in-Rust" 的高阶组件(HOC) 范式，减少样板代码并提供安全的样式透传与多态支持。

### 语法与签名
```rust
styled! {
    pub ComponentName<html_tag>(
        /* ... 标准 props ... */
    ) {
        /* ... CSS 规则，支持纯属性值 $(expr) 静态变量插值 ... */
        /* ... 及强大的构造性选择器 &(expr) 独立动态分片更新 ! ... */
        
        &:$(pseudo_prop) {
            color: $(hover_color);
        }

        variants: {
            prop_name: {
                variant_val: { /* static CSS */ }
            }
        }
    }
}
```

### 转换逻辑
1.  **Parsing**: 分别解析出 Visibility、组件结构名、底层依托 HTML 标签 (Tag)、Props 工具签名以及 CSS 块（包括 `variants` 控制块）。
2.  **CSS Compile (AST Fragmentation)**: 将 CSS 交由 `CssCompiler` 处理。
    * 属性侧表达式转为 CSS 层级临时变量进行分离提取。
    * 选择器侧表达式及其闭包触发规则树分片，形成 `DynamicRule` 并抛出返回。
3. **Variant Codegen**: 禁止在 `variants` 中使用动态插值 `$(...)` 及动态规则块。内部块会被展开为 `match` 匹配分支，直接返回变体对应构建的纯静态 CSS 类名字符串。
4. **Theme 解析与校验**: 编译器会提取组件 CSS 块内对形如 `Theme.field` 的访问模式。自动转换向基于该字段推导出的 `var(--slx-theme-field)` 引用。并且识别宏头部的 `#[theme(AppTheme)]` 属性，强制生出对主题被应用属性类型的安全断言约束（如 `assert_valid(&AppTheme.color)`）。
5. **Desugaring**: 宏在 AST 的根节点上展开为一段附带了 `#[::silex::prelude::component]` 定理的代码块，享有等额的属性代理分配和透传层（返回为 `impl View`）。
    * 生成的静态 CSS 变量推入底层的 `.style()` 属性注入器方法上。
    * 分离提取出的 `DynamicRule` （即像伪类这样的规则动态构建），依托新加组件局部单例管理器构建额外的 Effect 进行实时 `.update()` 按需生成和抛弃。
    * Variants 类挂载到多路 `.class()` 生成器上。

---

## 4. 路由宏 `#[derive(Route)]`

为 Enum 自动实现 `Routable` 和 `RouteView` Traits。

### 核心机制

#### `fn match_path(path: &str) -> Option<Self>`
*   **Radix Tree Generation**: 宏在编译时构建路由的前缀树 (Trie)，将所有路由规则合并为一个高效的查找结构。
*   **Tree Traversal**:
    *   **Static Matches**: 优先匹配静态路径段 (HashMap/Match)。
    *   **Param Segment**: 若无静态匹配，尝试匹配并解析参数节点。
    *   **Wildcard/Nested**: 作为后备选项 (Fallback)，匹配剩余路径。
*   **Performance**: 查找复杂度由 O(Routes) 降低为 O(Depth)，极大提升了大量路由下的匹配性能。

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

## 5. 状态宏 `#[derive(Store)]`

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

## 6. 辅助宏

### `style!`
*   语法: `style! { "color": "red", width: "100px" }`
*   输出: `silex::dom::attribute::group(("color", "red"), ("width", "100px"))`

### `classes!`
*   语法: `classes![ "btn", "active" => is_active ]`
*   输出: `silex::dom::attribute::group("btn", ("active", is_active))`

---

## 7. Clone 宏 `clone!`

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

---

## 8. Theme 宏 `define_theme!`

提供主题变量定义的构建体系，配合 CSS 编译器进行强类型验证。

### 用法
```rust
define_theme! {
    pub struct AppTheme {
        pub primary_color: ::silex::css::types::props::Color,
        // ...
    }
}
```

### 转换逻辑
1.  **Parsing**: 解析出 `struct` 名称及其定义的强类型字段。
2.  **Expansion**:
    *   构建原名结构体 (如 `AppTheme`)，包含定义的相同字段。
    *   生成映射 Trait 并为 `AppTheme` 实现，使得在编译期能够解析各字段实际类型（如通过 `ThemeFields::primary_color` 或类似形式参与 `styled!` 中的类型萃取与断言推导）。
    *   实现 `::silex::css::theme::ThemeType` 和 `::silex::css::theme::ThemeToCss`。后者赋予主题数据向 CSS Variables 展开的能力：在运行时生成形如 `--slx-theme-primary_color: #123456;` 规格的字符串串联用于 DOM 插接。
    *   提供 `Display` Trait 使得输出为拼接的 CSS var。
