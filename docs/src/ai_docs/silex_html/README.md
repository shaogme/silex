# Crate: `silex_html`

**Functional HTML builder DSL for Silex.**

此 Crate 基于 `silex_dom` 构建，提供了一套能够直接生成 `TypedElement<T>` 的函数式 API。它对应 HTML 规范中的标签集。

## 宏与定义机制

源码路径: `silex_html/src/lib.rs`

### `define_tags!`
*   **Purpose**: 批量定义标签的空结构体 (Marker Structs) 并实现 `silex_dom::tags` 中的 Marker Traits。
*   **Example Output**:
    ```rust
    pub struct Div;
    impl Tag for Div {}
    impl TextTag for Div {}
    ```

### Container Constructors
*   **Macro**: `define_container!(fn_name, TagType, "tag_string")`
*   **Signature**: `pub fn div<V: View>(child: V) -> TypedElement<Div>`
*   **Semantics**: 创建一个非空元素（Container Element），必须接受一个子 View。如果需要多个子节点，使用元组 `(child1, child2)`。

### Void Constructors
*   **Macro**: `define_void!(fn_name, TagType, "tag_string")`
*   **Signature**: `pub fn input() -> TypedElement<Input>`
*   **Semantics**: 创建一个空元素（Void Element），不接受子节点，但支持链式调用设置属性。

## Supported Tags (按类别)

### 1. Structure (Block & Inline)
*   **Functions**: `div`, `span`, `p`, `h1`..`h6`
*   **Semantic**: `header`, `footer`, `main`, `section`, `article`, `aside`, `nav`, `address`.
*   **Text helpers**: `pre`, `code`, `blockquote`, `em`, `strong`, `s`, `time`, `figure`, `figcaption`.
*   **Layout**: `br` (Void), `hr` (Void).

### 2. Lists
*   `ul`, `ol`, `li`

### 3. Forms
*   `form`, `button`, `label`.
*   `select`, `option`, `textarea`.
*   `input` (Void), `img` (Void).

### 4. Tables
*   `table`, `thead`, `tbody`, `tr`, `td`.

### 5. Links & Media
*   `a`.
*   `link` (Void), `area` (Void).
*   DOM 中定义的 `MediaAttributes` 支持 `img`, `video`, `audio` 等 (部分 construct 函数可能待补充完善，目前已有 `img`)。

### 6. SVG
*   **Container**: `svg`, `g`, `defs`, `filter`.
*   **Void / Leaf**: `path`, `rect`, `circle`, `line`, `polyline`, `polygon`.
*   **Filters**: `fe_turbulence`, `fe_gaussian_blur`, etc. (See source for full list).
*   **Namespace**: All SVG tags use `createElementNS("http://www.w3.org/2000/svg", ...)` internally via `TypedElement::new_svg`.

## Macros

### Helper Macros
为了简化嵌套结构的编写，该 crate 导出了一系列与函数同名的宏（如果 Rust 宏系统允许）。
*(Current implementation details defined in `define_tag_macros!`)*

*   **Usage**: `div!( child1, child2 )` expands to `silex_html::div((child1, child2))`.
*   **Purpose**: 消除手动编写元组括号的噪声。
