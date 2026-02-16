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
├── lib.rs          // 入口文件，根据 feature 导出宏
├── component.rs    // #[component] 宏实现：组件转换逻辑
├── css.rs          // css! 宏实现：集成 lightningcss
├── style.rs        // style!, classes! 宏实现
├── route.rs        // #[derive(Route)] 实现：路由匹配与生成
├── store.rs        // #[derive(Store)] 实现：全局状态管理
└── clone.rs        // clone! 宏实现：闭包变量捕获语法糖
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
*   **属性转发**：生成的结构体包含 `_pending_attrs: Vec<PendingAttribute>`，用于存储 `.class()`, `.id()` 等基础 HTML 属性，最终在 `mount` 时应用到根元素。

**特殊技巧**：
*   **自动 Into 推导**：为了提升 DX，对于 `Children`, `AnyView`, `String`, `Callback` 等常用类型，宏会自动生成接受 `impl Into<T>` 的 Builder 方法，减少用户手动调用的 `.into()`。

### 4.2 CSS 宏 `css!` (`css.rs`)

实现了 CSS-in-Rust 的核心逻辑。

**核心流程**：
1.  **哈希计算**：对输入的 CSS 字符串计算 Hash，生成唯一类名 `slx-{hash}`。
2.  **作用域封装**：将 CSS 内容包裹在 `.slx-{hash} { ... }` 选择器中，实现样式隔离。
3.  **处理与压缩**：调用 `lightningcss` 库对 CSS 进行解析、验证语法并压缩（Minify）。
4.  **代码注入**：生成调用 `silex::css::inject_style(id, css_content)` 的代码，并在 UI 中返回对应的类名。

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

### 4.5 Clone 宏 `clone!` (`clone.rs`)

解决 Rust 闭包中捕获变量所有权的痛点。

**难点解析**：
*   **内部克隆 (`@inner`)**：不仅在闭包外部克隆变量，还在闭包内部再次克隆。这对于 `FnMut` 闭包（可能会被多次调用）且每次调用都需要消费变量所有权的场景（如 `async` 块或 `move` 语义）至关重要。
*   **实现方式**：宏解析闭包体，重新构造 `Expr::Closure`，并在原有代码块前插入生成的 `let clone = clone.clone();` 语句。

## 5. 存在的问题和 TODO (Issues and TODOs)

### 已知限制 (Limitations)
*   **`#[component]` 泛型支持**：目前组件宏对泛型参数的处理较为基础，对于复杂的生命周期或常量泛型支持可能不完善。
*   **Tuple Variants in Route**：`derive(Route)` 目前对 Tuple Variants 的支持有限，建议用户主要通过 Struct Variants 来进行路由参数绑定。
*   **错误提示**：当宏展开失败时，生成的编译器错误信息有时不够直观，难以定位到具体的宏参数问题。

### 待办事项 (TODOs)
*   [ ] **增强 CSS 支持**：支持在 `css!` 中使用动态值（类似于 styled-components 的 props 插值）。
*   [x] **优化路由匹配算法**：已实现基于 Radix Tree 的匹配结构生成，解决了路由数量巨大时的性能瓶颈。
