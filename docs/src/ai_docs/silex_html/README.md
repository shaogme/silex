# Crate: `silex_html`

**Functional HTML builder DSL for Silex.**

此 Crate 基于 `silex_dom` 构建，提供了一套能够直接生成 `TypedElement<T>` 的函数式 API。它对应 HTML 规范中的标签集。

## Macros & Definition Mechanism

源码路径: `silex_html/src/tags/html.rs`, `silex_html/src/tags/svg.rs` (由 `silex_codegen` 基于 MDN 数据自动生成)

### `silex_dom::define_tag!`

此宏是定义标签的核心工具，负责生成 Struct、Function 和 Macro。

*   **Signature**: `silex_dom::define_tag!(StructName, "tag_name", function_name, construction_method, void_flag, [Traits...])`
*   **Parameters**:
    *   `StructName`: ZST 结构体名称 (PascalCase)，如 `Div`。
    *   `"tag_name"`: 传给 `document.createElement` 的标签名字符串。
    *   `function_name`: 构造函数名称 (snake_case)，如 `div`。
    *   `construction_method`: `new` (HTML) 或 `new_svg` (SVG)。
    *   `void_flag`: `void` (空元素，无子节点) 或 `non_void` (容器元素，接受子节点)。
    *   `[Traits...]`: 需要实现的额外 Marker Traits 列表 (如 `TextTag`, `FormTag`)。

### Generated Output Example

对于 `define_tag!(Div, "div", div, new, non_void, [TextTag])`，宏将生成：

1.  **Type Definition**:
    ```rust
    pub struct Div;
    impl Tag for Div {}
    impl TextTag for Div {} // 以及其他传入的 Trait
    ```

2.  **Constructor Function**:
    ```rust
    pub fn div<V: View>(child: V) -> TypedElement<Div> { ... }
    ```

3.  **Shortcut Macro**:
    ```rust
    macro_rules! div {
        ($($child:expr),* $(,)?) => { ... }
    }
    ```

## Supported Tags (Codegen)

所有标签均由 `tools/silex_codegen` 工具根据 MDN 数据 (`tags.json`) 自动生成，确保覆盖率。

*   **HTML**: 所有标准 HTML5 标签。
*   **SVG**: 所有标准 SVG 标签 (使用 `new_svg` 构造，属于 `http://www.w3.org/2000/svg` 命名空间)。

### Key Marker Traits & Injection

生成的代码不仅定义了 Struct，还根据 `codegen` 工具中的逻辑（In-Memory Patching）为每个 Struct 实现了特定的 Marker Traits：

*   `TextTag`: 允许包含文本节点。
*   `FormTag`: 标记该元素支持表单属性。
*   `MediaTag`: 标记该元素支持媒体属性.
*   `SvgTag`:所有 SVG 元素.

Codegen 随后会为实现了这些 Marker 的 `TypedElement<T>` 生成属性 Trait 的具体实现（见 `silex_html/src/attributes.rs` 和生成的 `html.rs`）。
