+++
title = "标签函数、宏与命名空间"
description = "silex_html 生成的 HTML/SVG 标签、typed marker、void 规则和宏命名。"
weight = 20
+++

# 标签函数、宏与命名空间

`src/tags/html.rs` 和 `src/tags/svg.rs` 不是手写标签表，而是
`silex_codegen` 根据标签配置生成的 Rust 源码。每一行
`silex_dom::define_tag!` 同时定义 marker、`TypedElement` 构造函数和标签
能力 trait 实现；非 void 标签随后还会生成一个同名宏。

## 两种调用形态

生成器把标签分为 `non_void` 和 `void`：

```rust
use silex_dom::attribute::AttributeBuilder;
use silex_html::{button, div, input, path, svg};

let html = div!(button!("Save"), input());
let same_html = div(button("Save"));
let icon = svg(path().attr("d", "M0 0"));
```

`div!`、`button!` 负责把多个 child 组合成 `ViewCons`；函数形式适合需要
显式传入一个已经组合好的 view，或需要避开宏名称冲突的场景。`input()`、
`path()` 是 void 函数，不能通过 child 宏追加内容。

对 non-void 标签，生成函数的有效形状是：

```text
tag<'scope, V>(child: V) -> TypedElement<'scope, Marker>
where V: View<'scope> + 'scope
```

生成宏接受 `tag!()` 或逗号分隔的 `tag!(child1, child2)`，并把空调用转换
为 `ViewNil`。宏只负责组合 view，不执行 mount。

## HTML 与 SVG namespace

- HTML 标签使用 `TypedElement::new`，最终调用
  `Document::create_element`。
- SVG 标签使用 `TypedElement::new_svg`，最终调用
  `Document::create_element_ns`，namespace 是
  `http://www.w3.org/2000/svg`。
- HTML 和 SVG 标签分别位于 `silex_html::html` 与 `silex_html::svg`；crate
  根同时重导出两者，只有名字不冲突时才适合无区别地使用根级导入。

SVG 的函数名遵循生成器的 Rust snake case 转换，例如 `linearGradient` 变为
`linear_gradient`、`foreignObject` 变为 `foreign_object`。与 HTML 宏名冲突
的 SVG 函数使用前缀，例如 SVG `<a>`、`<script>`、`<style>` 和 `<title>`
对应 `svg_a`、`svg_script`、`svg_style` 和 `svg_title`。

## marker 与 DOM 类型

每个生成标签都有一个公开 marker，例如 `Div`、`Input`、`Svg`。marker
实现 `silex_dom::element::tags::Tag`，其 `DomElement` 由生成器选择：

| 标签类别 | 当前 `DomElement` |
| --- | --- |
| 普通 HTML 标签 | `web_sys::HtmlElement` |
| `input`、`textarea`、`select`、`button` | 对应的 HTML 控件类型 |
| `option`、`optgroup`、`form`、`a` | 对应的 HTML 类型 |
| `img`、`canvas`、`audio`、`video`、`dialog`、`details`、`iframe` | 对应的 HTML 类型 |
| SVG 标签 | `web_sys::SvgElement`，但当前 SVG `a` 命中 HTML `a` 特判，详见限制 |

生成标签还会实现 `TextTag`、`FormTag`、`AnchorTag`、`MediaTag`、`OpenTag`、
`TableCellTag`、`TableHeaderTag` 或 `SvgTag`。这些 trait 是 metadata；当前
`silex_html` 属性 trait 的 blanket impl 并没有据此限制方法的调用范围。

## Rust 关键字和名称冲突

`tags.rs` 先从原始 tag name 生成 PascalCase，再通过内存 patch 修正 Rust
关键字或模块冲突：

| HTML/SVG tag | marker / 函数名 |
| --- | --- |
| `type` | `TypeEl` / `type_el` |
| `data` | `DataTag` / `data_tag` |
| `option` | `OptionTag` / `option_tag` |
| SVG `a` | `SvgA` / `svg_a` |
| SVG `script`、`style`、`title` | `SvgScript` / `svg_script` 等 |

属性方法也使用尾部下划线规避关键字，例如 `FormAttributes::type_`、
`LabelAttributes::for_` 和 `MediaAttributes::loop_`。

## 直接使用模块和自定义标签

需要明确区分 HTML/SVG 同名标签时使用模块路径：

```rust
let html_title = silex_html::html::title("HTML title");
let svg_title = silex_html::svg::svg_title("SVG title");
```

应用自己的标签不应修改生成的 HTML/SVG 文件。可以在应用或其他 facade 中
调用 `silex_dom::define_tag!`：

```rust
silex_dom::define_tag!(Icon, web_sys::SvgElement, "x-icon", icon, new_svg, non_void, [SvgTag, TextTag]);

let view = icon("content");
```

这个片段需要在调用模块中具备正确的 `web_sys` feature 和 marker trait
上下文；它是 API 形状示例，不是 `silex_html` 的文档 CI 示例。完整标签集
的维护仍应走 codegen 输入和生成流程。

## 运行时边界

构造标签不会创建节点，`TypedElement` 实现的是 `silex_dom::View`。真正
mount 时，标签的 child、属性和事件在一个新的 provisional owner 下创建；
失败会由 `silex_dom` 回滚，owner 关闭时移除节点和关联资源。因此不要把
标签函数的返回值误当成已经存在的 `web_sys::Element`，也不要从它推导出
跨 mount 的 DOM 所有权。

## 生成产物的限制

当前生成的 SVG 宏体使用 `$crate::view_chain!`，但公开 crate 没有这个宏。
因此调用 SVG 宏会在展开阶段失败；SVG 函数形式不受此问题影响。修改
`codegen.rs` 时应把 SVG 宏生成逻辑与 HTML 的 `$crate::chain!` 保持一致，
并增加一次实际宏调用的编译测试。
