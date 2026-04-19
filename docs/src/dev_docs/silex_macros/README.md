# Silex Macros 模块文档

## 1. 概要 (Overview)

`silex_macros` 是 Silex 框架的过程宏（Procedural Macros）集合，提供了构建现代 Web 应用所需的编译时元编程能力。

*   **定义**：Silex 的编译器扩展库，包含用于定义组件、处理 CSS、生成路由和管理状态的宏。
*   **作用**：该模块旨在消除 Rust 开发 UI 时的样板代码（Boilerplate），提供类似 JSX 的声明式体验，并在编译时进行静态分析和优化。生成的代码直接调用 `silex_core` 和 `silex_dom` 的底层 API。
*   **目标受众**：希望了解 Silex 魔法（如 `#[component]` 如何工作、`css!` 如何作用域化）的开发者，以及计划为框架贡献新宏功能的贡献者。

## 2. 理念和思路 (Philosophy and Design)

### 设计背景
Rust 的静态类型系统虽然安全，但在 UI 开发中往往伴随着繁琐的类型定义和生命周期管理。为了提供与 React/SolidJS 媲美甚至超越其的开发体验（DX），我们需要利用宏来隐藏底层的复杂性。

### 核心思想
1.  **零成本抽象 (Zero-Cost Abstractions)**：宏生成的代码应等价于手写的高性能 Rust 代码，不引入额外的运行时开销。
2.  **人体工学 (Ergonomics)**：通过属性宏和类函数宏，让 Rust 语法更接近 Web 开发者的直觉（例如自动生成 Props 结构体、自动处理 Styles）。
3.  **编译时验证**：尽可能在编译阶段捕获错误。例如，`css!` 宏会在编译时解析和验证 CSS 语法，而不是在运行时报错。

### 方案取舍 (Trade-offs)
*   **构建时间 vs 运行时性能**：我们选择增加编译时间（引入 `syn`, `quote`, `lightningcss` 等依赖）来换取更小的运行时体积和更快的执行速度。
*   **魔法 vs 显式**：虽然宏被称为“魔法”，但我们尽量保持宏的行为可预测。例如 `#[component]` 生成的结构体和方法都遵循统一的命名规范，便于调试。

## 3. 模块内结构 (Internal Structure)

`silex_macros` 根据功能特性（Features）组织代码：

```text
src/
├── lib.rs                     // [宏入口] 聚合所有功能模块的宏定义声明与 Feature 转发
├── component.rs               // [组件系统] #[component] 核心解析与 Props 转换逻辑
├── css.rs                     // [样式系统] CSS 模块父入口：内置属性校验与编译器调度
├── css/                       // [样式核心] 编译器子模块：
│   ├── ast.rs                 // 语法解析：基于 syn 的强通配 CSS AST 实现
│   ├── compiler.rs            // 编译器：负责动静态分析、哈希生成与压缩优化
│   ├── styled.rs              // 样式化组件：styled! 与 global! 的脱糖实现
│   ├── theme.rs               // 强类型主题：theme! 桥接与变量自动关联器
│   └── classes.rs             // 类名合并：classes! 宏的高效动态拼接逻辑
├── route.rs                   // [路由系统] #[derive(Route)]：Radix Tree 核心编译器
└── store.rs                   // [状态系统] #[derive(Store)]：全局 Context 接入与信号封装
```

### 核心组件关系
宏模块本身不依赖 `silex` 的其他 crate（为了避免循环依赖，只在生成的代码中引用 `silex`）。它主要通过生成 `TokenStream` 来操纵 AST。

*   **输入**：Rust 源代码片段（函数、Enum、Struct、宏调用）。
*   **输出**：展开后的 Rust 代码，这些代码实现了 `silex_core` (如 `Signal`) 和 `silex_dom` (如 `View`, `AttributeBuilder`) 定义的 Traits。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 组件宏 `#[component]` (`component.rs`)

该宏将普通 Rust 函数转换为实现了 `View` trait 的结构体。

**关键数据结构**：
*   **Props 处理**：宏会扫描函数参数，将其区分为 *Required*（必填）和 *Optional*（选填）。
    *   **Required**: 转换为 `Option<T>` 字段，初始化为 `None`。如果 `mount` 时仍为 `None`，则 panic。
    *   **Optional**: 带有 `#[prop(default)]` 的参数，转换为 `T` 字段，初始化为默认值。
*   **Builder 模式**：为每个参数生成 `pub fn param_name(mut self, val: T) -> Self` 方法，支持链式调用。
*   **入口函数构造**：宏会生成一个与组件函数同名的构造函数。如果组件的第一个参数是 `children`，则构造函数会强制要求传入该参数（作为 `impl View`），从而支持 `Parent(children)` 的嵌套写法。对于其他属性，依然使用链式 Builder 模式设置。
*   **属性转发**：生成的结构体包含 `_pending_attrs: Vec<PendingAttribute>`，用于存储 `.class()`, `.id()` 等基础 HTML 属性，最终在 `mount` 时应用到根元素。

**特殊技巧**：
*   **自动 Into 推导**：为了提升 DX，对于 `Children`, `AnyView`, `String`, `Callback` 等常用类型，宏会自动生成接受 `impl Into<T>` 的 Builder 方法，减少用户手动调用的 `.into()`。
*   **泛型与生命周期支持 (`PhantomData` 注入)**：为了解决未在组件的 props 字段中直接使用的泛型参数（或复杂生命周期）引起的 `parameter is never used` 编译报错，宏会提取函数签名的所有泛型参数，并自动在生成的组件结构体中注入包裹了元组函数签名的原生 `_phantom: std::marker::PhantomData<fn() -> (#(#phantom_types,)*)>`，不仅消除了编译警告，还防止了破坏任何 `Send`/`Sync`/`Drop` 语义。

### 4.2 CSS 宏 `css!` (`css/ast.rs` & `css/compiler.rs`)

实现了 CSS-in-Rust 的核心逻辑，由重构后的强类型解析和编译引擎支撑。现在 `css!` 等入口均归一化由 `css_impl` 驱动。

**核心流程**：
1.  **AST 解析 (`ast.rs`)**：利用 `syn::parse` 将原生的输入 TokenStream 逐层解析。它现在支持代码块写法（取代了字符串字面量）并完整支持：
    *   **@-Rules**   **theme! 自动化**: 
    *   **Patch 生成**: 自动生成 `{Name}Patch` 结构体，支持链式 Setter。
    *   **强类型常量**: 自动生成 `pub const NAME: CssVar<T>`。
    *   **自动别名**: 通过 `#[theme(main)]` 自动生成 `type Theme = ...;`，简化样板代码。
2.  **语义遍历与萃取 (`compiler.rs`)**：负责遍历上述的 AST 节点树：
    *   **智能空格恢复**: 在将 TokenStream 转回 CSS 字符串时，解决了诸如 `1px solid black` 的连词问题。
    *   **Theme 自动关联**: 如果未显式提供主题类型，编译器会自动在当前作用域查找 `Theme` 别名。
3.  **哈希计算与变量下发**：针对输入的 Token 进行特征哈希生成局部的随机后缀 `slx-{hash}`。将上文埋设的所有占位符 `--slx-tmp-*` 实化为局部的独设 CSS 变量 `--slx-{hash}-*`。
4.  **作用域封装**：将纯净的静态 CSS 载入到一个全局包裹对象层 `.slx-{hash} { ... }` 之中以防止污染。
5.  **语法校验与极致压缩 (Minification)**：调用外部强引擎 `lightningcss` 对此静态 CSS 解析，实施语法层验证和体积极优化。### `css!` 宏集成
*   **Compile Time**: 将输入 (TokenStream) 交由 `css_impl` 处理。支持代码块 `{ ... }` 语法，计算 Hash，生成 Scoped CSS，压缩。
*   **Runtime**: 生成的代码自动调用 `silex::css::inject_style`。
*   **Theme Integration**: 自动查找 `Theme` 别名并进行静态变量验证。
*   **Flow**: `Macro Expansion` -> `css_impl` -> `Hash & Compress` -> `Code Gen (inject_style)` -> `Runtime Execution`.
 转换。这意味着宏不需要硬编码属性列表，而是直接尝试寻找 `::silex::css::types::props` 下对应的 ZST 标签，将验证职责解耦。
    *   返回 `silex::css::DynamicCss` 结构体，该结构体实现了 `ApplyToDom`。
    *   **Codegen 类型注入**：通过宏代码生成将捕获的属性标量（例如 `width` 获取到由 `get_prop_type` 转换后的强类型 `props::Width`），进而实施多态 Trait Bounds (`ValidFor<P>`) 限制。
    *   **强拦截与显式越权 (`UnsafeCss`)**: 废弃泛用 `&str` 的放行，若需越过类型检查，必须显式包装进 `UnsafeCss::new()`。

### 4.3 路由宏 `#[derive(Route)]` (`route.rs`)

为 Enum 自动实现前端路由逻辑。

**关键实现**：
*   **`match_path` (Radix Tree)**：宏内部构建了一个路由 Trie 树，并将其编译为嵌套的 `match` 语句。
    *   **静态优先**：优先匹配静态路径段，利用 Rust 的 `match` 优化（跳转表）。
    *   **优先级管理**：严格遵循 `Static > Param > Wildcard` 的匹配顺序。
    *   支持静态段 (`/users`)、参数段 (`/:id`) 和通配符 (`*`)。
    *   **嵌套路由** (`#[nested]`)：当遇到嵌套字段时，宏会递归调用子路由的 `match_path`，处理剩余路径。
*   **`to_path`**：反向生成 URL。对于嵌套路由，宏实现了智能路径拼接，避免双斜杠问题。
*   **`render`**：
    *   利用 `view` 属性指定的组件函数。
    *   **Guard 机制**：支持像洋葱模型一样层层包裹 Guards (`guard = [Outer, Inner]`)，通过 `quote!` 循环生成嵌套调用的代码结构。

### 4.4 状态宏 `#[derive(Store)]` (`store.rs`)

简化 Context API 的使用。

**转换逻辑**：
*   将原始 Struct 的字段类型 `T` 转换为 `RwSignal<T>`。
*   生成一个新的 `StoreStruct`，并实现 `silex::store::Store` trait，使其能够自动从 Context 中获取。
*   生成辅助 Hook `use_{struct_name_snake_case}`，封装 `use_context` 和错误处理逻辑。

### 4.5 样式组件宏 `styled!` (`styled.rs`)

引入了类似 `styled-components` 的“样式即组件”范式。

**核心机制**：
*   **脱糖 (Desugaring)**：`styled!` 宏会将内部定义的组件（包括可见性、底层 HTML 标签、Props 等）在 AST 层面脱糖为一个标准的 `#[component]` 函数。这意味着它完美兼容现有的组件体系和属性透传 (`AttributeBuilder`)。
*   **编译期提取与变量隔离**：复用了 `css::compiler::CssCompiler` 的逻辑，提取静态 CSS 并生成唯一类名，将仅存在于属性值内的动态插值 `$(expr)` 转换为 CSS 变量绑定 (`--slx-{hash}-{index}`)。
*   **主题聚合与强类型断言**：通过探测内部对于形如 `$theme.field` 的标识符调用，自动构建为对应 CSS 变量 `--slx-theme-field` 并由解析器分析属性上下文。
    *   **自动主题感知**: 宏会优先使用自动生成的 `Theme` 别名进行类型验证，极大减少了 `#[theme(...)]` 的显式书写工作。
*   **动态规则树分片 (Dynamic Rules)**：在词法解析阶段 (TokenTree Parsing)，如果宏检测到选择器层面（或嵌套属性名前缀）包含 `$(...)`，会将这段包含大括号的规则块从主 CSS 静态树中剥离，形成游离分片，并依托 `DynamicStyleManager` 实例以闭包的方式按需利用 DOM 的 `<style>` 重置方法直接重塑热更新规则！借此彻底突破了原生 CSS Variable 不可用于选择器的天生局限。
*   **Variants 静态架构**：完全支持 `variants:` 语法块。通过在编译阶段静态合成各变体的 CSS 并生成类名，在运行时利用模式匹配直接返回对应属性值的静态类字符串。不仅具备极高的代码表现力，还有效避开了基于 CSS 变量进行多属性赋值产生的性能代价。

### 4.6 强类型主题宏 `theme!` (`css/theme.rs`)

`theme!` 的主要职责是建立受约束的结构体并搭接主题字典。

**结构重定义**：
由于目前过程宏难以对原本存在的 Struct 获取到深度的泛型类型反射信息，该宏通过重塑整个强类型字段体系来工作：
*   **映射机制**: 生成隐式内部 Trait `<TheName>Fields` 用以辅助。
*   **生成约束及串联**: 为其实现原生地 `ThemeToCss` 与 `Display`：其依靠读取其上所有定义好的标量自动遍历串联为形如 `--slx-theme-xxx: val;` 的大型全局行内样式字串。随后可将其赋能在全域 `GlobalTheme` 下或者局部作用域。

## 5. 存在的问题和 TODO (Issues and TODOs)

### 已知限制 (Limitations)
*   **Tuple Variants in Route**：`derive(Route)` 目前对 Tuple Variants 的支持有限，建议用户主要通过 Struct Variants 来进行路由参数绑定。

### 计划中的强类型演进 (Future Expansions for Type-Safe CSS)
*   **结构化复合样式支持 (Composite Properties)**：对 `border`、`box-shadow` 支持复合构建，比如接受由开发者新建实现好 `ValidFor<props::Border>` 接口的 `struct BorderDesc { w: Px, s: BorderStyle, c: Rgba }`，并让其实现一个格式化的复杂的 `Display`。
*   **原生防沉淀强算力支持 (Math Operators)**：计划为包裹单位注入基础的 `Add` / `Sub` 重载，允许写出基于组件级别的 `px(300) + px(50)` 及基于计算 Signal 环境的相加，进而自动演变为 CSS 中合规支持的基础类型或者在构建时直接化简为单值。
