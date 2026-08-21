+++
title = "`silex_html`"
description = "由代码生成的 HTML/SVG 标签、标签宏和类型化属性 facade。"
template = "section.html"
sort_by = "weight"
+++

# `silex_html`

`silex_html` 是 Silex 面向应用层的 HTML/SVG facade。它不直接管理浏览器
DOM，而是把标签名、命名空间、标签类别和常用属性方法组织成可组合的
`silex_dom::TypedElement`。真正的节点创建、挂载、事件和 owner 清理由
`silex_dom` 完成。

## 在 Silex 架构中的位置

```text
应用组件 / silex
        │  div!(...), input().value(...)
        ▼
       silex_html
   生成的标签函数、宏、属性 trait
        │  TypedElement<'scope, T>
        ▼
       silex_dom
  View · AttributeBuilder · MountOwner
        │
        ▼
      web_sys DOM
```

因此，`silex_html` 解决的是“如何用稳定、可发现的 Rust API 描述 HTML/SVG
视图”，而不是“如何把视图挂到某个 host”。需要处理 `Runtime`、
`MountedApp`、错误 handler、响应式 effect 或清理时，应阅读
[`silex_dom` 总文档](@/developer/crates/silex_dom/_index.md)。

## 稳定入口

crate 根 `lib.rs` 暴露以下入口：

| 入口 | 内容 |
| --- | --- |
| `html` | 生成的 HTML 标签函数、marker 类型和非 void 标签宏所在模块。 |
| `svg` | 生成的 SVG 标签函数、marker 类型和非 void 标签宏所在模块。 |
| 根级标签导出 | `div`、`input`、`svg` 等函数，以及生成的 HTML/SVG marker。 |
| 根级标签宏 | 非 void 标签的 `div!`、`button!`、`svg!` 等宏。 |
| `FormAttributes` 等 trait | HTML 常用属性的命名 facade。 |
| `chain`、`ViewCons`、`ViewNil` | 由 `silex_dom` 重导出的子视图组合工具。 |

最小导入通常如下：

```rust
use silex_dom::attribute::{AriaAttributes, AttributeBuilder, GlobalAttributes};
use silex_html::{FormAttributes, button, div, input};

let view = div!(
    input()
        .type_("search")
        .placeholder("Search")
        .aria_label("Search"),
    button!("Submit"),
)
.id("search-panel");
```

这段代码只构造 view factory；它没有创建 DOM。`div!` 和 `button!` 是宏，
而 `input()` 是 void 标签函数。可直接编译并运行的版本见下面的文档示例。

## 标签函数与标签宏

代码生成器为每个标签生成一个 marker 类型和一个函数：

- non-void 标签函数形如
  `fn div<'scope, V>(child: V) -> TypedElement<'scope, Div>`，要求 child
  实现 `View<'scope>`；
- void 标签函数形如
  `fn input<'scope>() -> TypedElement<'scope, Input>`，不接收 child；
- non-void 标签还生成零个或多个 child 的宏，例如 `div!()`、
  `div!(header, body)`；宏内部使用 `ViewNil` 或 `chain!`；
- void 标签不生成同名 child 宏，应使用 `input()`、`img()`、`path()` 等
  函数；
- HTML 和 SVG 的同名函数通过 `html`/`svg` 模块区分，根级重导出时应注意
  名称冲突。

`TypedElement<'scope, T>` 的 `T: Tag` marker 记录目标 `web_sys` 类型和
标签能力。`TextTag`、`FormTag`、`MediaTag`、`SvgTag` 等 marker trait 在
`silex_dom` 中定义，由生成的 `define_tag!` 调用实现。

## 属性 facade

`attributes.rs` 用一个宏生成常用命名方法；每个方法最终调用
`AttributeBuilder::attr`，也就是 HTML attribute 目标，而不是 DOM
property。主要分组如下：

| trait | 代表方法 |
| --- | --- |
| `FormAttributes` | `type_`、`value`、`checked`、`disabled`、`placeholder`、`name`、`min`、`max`、`action`、`method` |
| `LabelAttributes` | `for_` |
| `AnchorAttributes` | `href`、`target`、`rel`、`download` |
| `MediaAttributes` | `src`、`alt`、`width`、`height`、`controls`、`poster`、`preload` |
| `OpenAttributes` | `open` |
| `TableCellAttributes` | `colspan`、`rowspan`、`headers` |
| `TableHeaderAttributes` | `scope`、`abbr` |
| `DataAttributes` | `data_slot`、`data_state`、`data_orientation`、`data_value` 等 |
| `PopoverAttributes` | `popover`、`popovertarget`、`popovertargetaction` |

所有方法的值都必须满足 `IntoStorable<'scope>`，所以可以传入借用字符串、
owned `String`、基础类型、`Option`、响应式值或 `AttrOp`。全局属性、ARIA、
事件和 `prop` 入口仍来自 `silex_dom::attribute`，不是
`silex_html::attributes` 自己重新实现的运行时。

`value()`、`checked()` 等 facade 方法写 attribute。需要同步控件当前状态
时，应显式使用 `.prop("value", value)`、`.prop("checked", checked)`，或
使用 `GlobalEventAttributes::bind_value`；attribute/property 的区别和清理
语义见 [`silex_dom` 属性文档](@/developer/crates/silex_dom/attributes.md)。

## 生命周期、平台与所有权

- 标签函数和标签宏只保存 tag name、namespace、child view 与待应用属性，
  构造阶段不会访问 `window` 或 `document`。
- 每次 `View::mount` 都通过 `silex_dom` 创建新的 DOM 节点；同一个
  `TypedElement` 描述可以被多次挂载，但不应保存旧的物理节点来实现复用。
- 属性、事件、响应式 effect 和节点清理由传入的 `MountOwner` 管理；
  `silex_html` 不提供独立的 dispose API。
- crate 使用 `web_sys` 的 HTML/SVG 类型。native 构建可以检查标签组合和
  属性类型，但真实 mount、事件和浏览器对象访问需要 `wasm32` 环境。
- `TypedElement` 带有 `'scope`，因此属性中的借用值和响应式值不能逃逸
  对应的 mount scope；不要把带 scope 的 view 当作 `'static` 配置保存。

## 最小可运行流程

下面的源码同时用于页面展示和 `silex_html` 的文档示例测试：

{% set source = load_data(path="examples/silex_html/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

这个示例在 native 测试中验证标签函数、HTML 宏、属性 facade 和 SVG 函数
可以编译并构造 view；它不在没有浏览器的进程中尝试 mount。

## 模块地图与源码索引

| 模块/文件 | 责任 |
| --- | --- |
| `src/lib.rs` | 模块声明、标签与公共类型重导出。 |
| `src/attributes.rs` | `FormAttributes` 等命名属性 trait 的定义和 blanket impl。 |
| `src/tags/html.rs` | 由 `silex_codegen` 生成的 HTML 标签函数、marker 和宏。 |
| `src/tags/svg.rs` | 由 `silex_codegen` 生成的 SVG 标签函数、marker 和宏。 |
| `crates/utils/silex_codegen/src/tags.rs` | MDN 标签数据解析、内存 patch 和 trait 分类。 |
| `crates/utils/silex_codegen/src/tags/codegen.rs` | 标签函数、`define_tag!` 调用和宏文本生成。 |
| `crates/utils/silex_codegen/src/main.rs` | 输入路径选择、生成文件写入和完整 codegen 流程。 |
| `docs/examples/silex_html/basic.rs` | 可执行文档示例。 |
| `crates/silex_html/tests/docs_examples.rs` | 文档示例的 crate 级编译/运行入口。 |

## 专题

- [标签函数、宏与命名空间](tags.md)：解释 void/non-void、typed marker、
  HTML/SVG namespace 和生成命名规则。
- [属性 facade 与 DOM 目标](attributes.md)：解释命名属性 trait、
  `attr`/`prop`、响应式值和 scope 边界。
- [标签代码生成链](codegen.md)：解释 MDN 输入、内存 patch、生成器和维护
  规则。
- [测试、验证与当前边界](testing.md)：说明只验证本 crate 文档示例的命令，
  以及当前生成产物的风险点。

## 当前限制

- `silex_html` 只提供标签和属性 facade，不负责应用 host、runtime 或挂载
  生命周期；这些行为由 `silex_dom` 提供。
- `FormAttributes` 等 trait 当前对所有实现 `AttributeBuilder` 的类型做
  blanket impl；`FormTag` 等 marker 不会阻止不适用标签调用这些方法。它们是
  命名便利和文档分类，不是当前实现中的完整 HTML 内容模型校验。
- 标签集合和 void 判定来自仓库内 MDN compatibility JSON 及
  `tags.rs` 的固定列表，生成器不会根据应用使用情况裁剪标签，也不会自动
  验证浏览器对每个标签的内容模型。

修改标签输入、属性 facade 或生成器后，应同时检查生成文件、文档示例和
`silex_dom` 的 mount/attribute 契约；不要手工编辑 `src/tags/*.rs` 后跳过
codegen 复核。
