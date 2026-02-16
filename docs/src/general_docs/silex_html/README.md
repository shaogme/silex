# Silex HTML

`silex_html` 提供了构建用户界面的 DSL (Domain Specific Language)。它是一组简单的工厂函数，用于创建强类型的 HTML 元素。

## 基础用法

不再需要手写 `Element::new("div")`，而是直接使用对应的函数：

```rust
use silex_html::{div, p, button};

let view = div((
    h1("Hello Silex"),
    p("This is a fine-grained reactive framework."),
    button("Click me")
));
```

## 组合与嵌套

由于 Rust 不支持变长参数函数，多个子元素需要包裹在元组中：

```rust
ul((
    li("Item 1"),
    li("Item 2"),
    li("Item 3"),
))
```

为了减少括号噪声，我们提供了一组同名的宏（推荐）：

```rust
use silex_html::{ul, li};

ul!(
    li!("Item 1"),
    li!("Item 2")
)
```

## 类型安全属性

`silex_html` 产生的元素是强类型的（如 `TypedElement<Input>`）。这让 IDE 可以提供更好的补全，并且在编译时检查属性的合法性。

例如，只有 `input` 才有 `type` 属性，只有 `a` 才有 `href` 属性：

```rust
// ✅ 合法
input().type_("text").value("Silex");
a("Link").href("https://github.com");

// ❌ 编译错误：div 没有 href 方法
// div("Content").href(...) 
```

## 支持的标签

目前覆盖了绝大多数标准的 HTML5 标签：

*   **结构**: `div`, `section`, `header`, `footer`...
*   **文本**: `p`, `span`, `h1`-`h6`, `strong`, `em`...
*   **列表**: `ul`, `ol`, `li`.
*   **表单**: `form`, `input`, `button`, `select`, `option`...
*   **媒体**: `img`, `video`, `audio`...
*   **SVG**: 完整的 SVG 标签支持 (`svg`, `path`, `circle`...)。
