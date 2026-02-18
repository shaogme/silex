# Silex HTML (silex_html)

## 1. 概要 (Overview)

*   **定义**：`silex_html` 是 Silex 框架中提供 **类型安全 HTML DSL (Domain Specific Language)** 的核心 crate。
*   **作用**：它基于 `silex_dom` 构建，提供了一套符合 Rust 语法的函数式 API，用于创建 HTML 和 SVG 元素。它充当了用户编写 UI 代码与底层 DOM 操作之间的桥梁，将 `div`, `span` 等常见标签封装为具体的 Rust函数和宏。
*   **目标受众**：框架开发者、希望扩展自定义标签的高级用户。

## 2. 理念和思路 (Philosophy and Design)

*   **设计背景**：直接使用 `web_sys` 或 `silex_dom` 的底层 API 构建 DOM 树既繁琐又缺乏类型安全。我们需要一种符合人体工程学的方式来声明 UI 结构。
*   **核心思想**：
    *   **类型安全 (Type Safety)**：每个 HTML 标签（如 `<div>`）都对应一个唯一的 Zero-Sized Type (ZST) 结构体（如 `struct Div`）。这意味着 `div()` 函数返回的是 `TypedElement<Div>`，而不是通用的 `Element`。这允许我们在编译时检查特定标签所支持的属性（例如，只有 `<a>` 标签才有 `href` 属性，只有 `<input>` 标签才有 `value` 属性）。
    *   **函数式组合 (Functional Composition)**：UI 树的构建通过函数的嵌套调用完成，例如 `div(span("hello"))`。
    *   **零成本抽象 (Zero-Cost Abstraction)**：所有的标签结构体都是 ZST，不会占用运行时内存。函数调用在内联后直接对应到底层的 DOM 创建操作。
*   **方案取舍 (Trade-offs)**：
    *   **宏 vs 函数**：最初的设计可能倾向于只使用函数。但为了支持变长参数（即多个子节点），Rust 的函数必须通过 tuple（如 `(a, b, c)`）来传递。为了提升开发体验，我们在 API 上增加了一层宏封装（如 `div!`），允许用户省略 tuple 的括号。
    *   **强类型标签**：虽然增加了类型系统的复杂度（需要定义数百个 struct），但换来了强大的编译时保障和自动补全能力。

## 3. 模块内结构 (Internal Structure)

该 crate 的核心代码大部分由工具自动生成，目录结构清晰：

```text
silex_html/
├── src/
│   ├── lib.rs           // 模块导出入口
│   └── tags/            // 存放生成的标签定义
│       ├── html.rs      // 所有 HTML 标签
│       └── svg.rs       // 所有 SVG 标签
```

*   **核心组件关系**：
    *   **Codegen Tool**: `tools/silex_codegen` 读取 `tags.json` (来源于 MDN 数据)，生成 `tags/*.rs` 文件。
    *   **Macro Definition**: `silex_dom::define_tag!` 宏定义在底层 crate 中，被生成的代码调用。
    *   **Public API**: 用户通过 `silex_html::div` (函数) 或 `silex_html::div!` (宏) 访问这些生成的代码。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 代码生成机制 (Code Generation)

手动维护数百个 HTML/SVG 标签极易出错且难以同步标准。因此，我们引入了 `silex_codegen` 工具。

1.  **数据源**: `tags.json` 包含了从 MDN 或 HTML 规范中提取的标签元数据（标签名、是否是 void 元素、所属类别等）。
2.  **生成过程**:
    *   读取 JSON 数据。
    *   根据分类（HTML vs SVG）分别生成 Rust 代码。
    *   为每个标签调用 `silex_dom::define_tag!` 宏。

### 4.2 统一宏 `define_tag!`

在 `silex_dom` 中定义的 `define_tag!` 宏是核心抽象。它同时完成了以下三件事：

```rust
// 伪代码展示宏展开逻辑
macro_rules! define_tag {
    ($StructName:ident, $tag_str:literal, $fn_name:ident, $new_method:ident, $void_kind:ident, [$($traits:ident),*]) => {
        // 1. 定义 ZST 结构体和 Marker Traits
        pub struct $StructName;
        impl Tag for $StructName {}
        $( impl $traits for $StructName {} )*

        // 2. 定义构造函数 (Constructor)
        pub fn $fn_name<V: View>(child: V) -> TypedElement<$StructName> {
            // $new_method 是 `new` (HTML) 或 `new_svg` (SVG)
            let el = TypedElement::$new_method($tag_str);
            // $void_kind 决定是否挂载子节点 (void 元素不挂载)
            child.mount(&el.element.dom_element);
            el
        }

        // 3. 定义语法糖宏 (Shortcut Macro)
        #[macro_export]
        macro_rules! $fn_name {
            // 无参数 -> 调用函数传入 ()
            () => { $crate::tags::$module::$fn_name(()) };
            // 有参数 -> 包装为 tuple 传入
            ($($child:expr),+ $(,)?) => {
                $crate::tags::$module::$fn_name(($($child),+))
            };
        }
    };
}
```

这种设计确保了函数 API 和宏 API 的行为绝对一致。

### 4.3 SVG 特殊处理

SVG 元素必须在 `http://www.w3.org/2000/svg` 命名空间下创建。
因此，在生成的 `svg.rs` 中，所有标签都使用 `new_svg` 作为构造方法参数传递给宏，而 HTML 标签使用 `new`。

```rust
// HTML
silex_dom::define_tag!(Div, "div", div, new, non_void, ...);
// SVG
silex_dom::define_tag!(Circle, "circle", circle, new_svg, void, ...);
```

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **文档注释**: 目前自动生成的代码没有包含详细的文档注释（如 MDN 链接、标签描述）。未来可以在 `tags.json` 中丰富元数据，并在 codegen 阶段生成 `///` 注释。
*   **属性绑定增强**: 虽然标签本身是强类型的，但属性绑定目前仍主要依赖 `web_sys` 的反射或通用的 builder pattern。未来可以利用 `TypedElement<T>` 中的 T 来进一步约束可以调用的属性方法（例如，只为 `TypedElement<Input>` 实现 `value()` 方法）。
