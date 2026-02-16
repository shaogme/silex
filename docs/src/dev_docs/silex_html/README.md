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

该 crate 的结构非常扁平，大部分逻辑通过宏生成。

```text
silex_html/
└── src/
    └── lib.rs       // 包含所有标签定义、构造函数宏和辅助特质
```

*   **核心组件关系**：
    *   **Tag Structs**：`pub struct Div;`, `pub struct Span;` 等。这些结构体实现了 `silex_dom::Tag` 以及其他标记 trait（如 `TextTag`, `FormTag`）。
    *   **Constructors**：`pub fn div(...) -> TypedElement<Div>`。这些是用户实际调用的函数。
    *   **Macros**：`div!(...)`。这是对 constructor 的简单封装，用于简化多子节点的语法。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 标签定义宏 (`define_tags!`)

为了避免手动编写数百个几乎相同的结构体，我们使用 `define_tags!` 宏批量生成标签定义。

```rust
macro_rules! define_tags {
    // 基础定义，生成 ZST 结构体并实现 Tag trait
    (@basic $($tag:ident),*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $tag;
            impl Tag for $tag {}
        )*
    };
    // 为标签实现特定标记 trait (Marker Traits)
    (@impl $trait:ident for $($tag:ident),*) => { ... };
}
```

这些结构体自身不包含数据，它们存在的唯一目的是作为泛型参数 `T` 传递给 `TypedElement<T>`，从而携带类型信息。

### 4.2 元素构造器 (`define_container!` & `define_void!`)

HTML 元素可以分为两类：
1.  **Container Elements**：可以包含子节点的元素（如 `<div>`, `<p>`）。
2.  **Void Elements**：不能包含子节点的元素（如 `<input>`, `<br>`）。

我们分别使用宏来生成对应的构造函数：

```rust
// 生成容器元素构造函数
macro_rules! define_container {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        // V: View 约束允许传入任何实现了 View 的类型（如 String, 其他 Element, Component 等）
        pub fn $fn_name<V: View>(child: V) -> TypedElement<$tag_type> {
            let el = TypedElement::new($tag_str);
            child.mount(&el.element.dom_element); // 立即挂载子节点
            el
        }
    };
}

// 生成空元素构造函数
macro_rules! define_void {
    ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
        pub fn $fn_name() -> TypedElement<$tag_type> {
            TypedElement::new($tag_str)
        }
    };
}
```

**逻辑解析**：
*   `TypedElement::new("div")` 创建底层的 DOM 节点。
*   `child.mount(...)` 是 `silex_dom::view::View` trait 的核心方法，负责将子节点（无论它是文本、另一个元素还是组件）追加到父节点中。

### 4.3 SVG 支持

SVG 元素需要特殊的命名空间 (`http://www.w3.org/2000/svg`)。因此我们有专门的 `define_svg_container!` 和 `define_svg_void!` 宏。

它们内部调用的是 `TypedElement::new_svg($tag_str)` 而不是 `TypedElement::new`。这确保了 `document.createElementNS` 被正确调用，否则 SVG 将无法在浏览器中正确渲染。

### 4.4 语法糖宏 (`define_tag_macros!`)

为了支持类似 `div!( child1, child2 )` 的语法，我们导出了一系列与函数同名的宏。

```rust
macro_rules! div {
    // 无子节点
    () => { silex_html::div(()) };
    // 有子节点，自动包装为 tuple
    ($($child:expr),+ $(,)?) => {
        silex_html::div(($($child),+))
    };
}
```

这利用了 `silex_dom` 中 `impl View for (A, B, ...)` 的实现，使得 tuple 也可以作为一个整体的 `View` 被挂载。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **标签覆盖率**：目前的标签列表手动维护在 `lib.rs` 中，虽然覆盖了常用标签，但可能遗漏了一些不常用的 HTML5 标签或 SVG 标签。
*   **属性绑定**：当前的 crate 仅负责 **创建** 元素。属性的类型安全绑定（Attributes）主要由 generic trait 在 `silex_dom` 中定义，但在 `silex_html` 中通过 Tag Structs 关联。未来可能需要更紧密的结合，或者通过代码生成（codegen）完全覆盖 MDN 规范。