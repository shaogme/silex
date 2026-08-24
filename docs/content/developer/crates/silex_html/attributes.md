+++
title = "属性 facade 与 DOM 目标"
description = "silex_html 命名属性 trait、IntoStorable、attribute/property 和响应式绑定边界。"
weight = 30
+++

# 属性 facade 与 DOM 目标

`silex_html::attributes` 只提供面向 HTML 的命名方法。它们通过
`silex_view::attribute::AttributeBuilder::attr` 创建 `AttrOp`，在标签真正
mount 时才应用到 DOM。属性的生命周期、响应式 effect、事件资源和清理由
`silex_dom` 的 owner 管理。

这些 trait 不改变 `AttributeBuilder` 的核心接口：需要动态选择目标时仍可
直接调用 `.attr(name, value)`、`.prop(name, value)` 或 `.apply(value)`。
命名方法只是固定 HTML attribute 名称的便捷 facade。

## trait 分组

七个 HTML 语义 trait 要求类型实现公开的 `HtmlTagCarrier`，且 carrier 的
`Tag` 实现对应 marker；`TypedElement<'scope, T>`、带 HTML 根标签的
`styled!` builder 和 product 都具备这项能力。`DataAttributes` 和
`PopoverAttributes` 仍对所有 `AttributeBuilder<'scope>` 提供通用 facade。
因此语义方法应在类型擦除前调用，`Element`、`AnyView` 和普通
`#[component]` builder 必须使用显式 `attr`、`prop` 或 `apply`。

| trait | 能力约束 | 方法到 HTML name 的映射 |
| --- | --- | --- |
| `FormAttributes` | `V: HtmlTagCarrier`, `V::Tag: FormTag` | `type_`→`type`、`value`、`checked`、`disabled`、`readonly`、`required`、`placeholder`、`name`、`autocomplete`、`autofocus`、`min`、`max`、`step`、`pattern`、`multiple`、`accept`、`selected`、`rows`、`cols`、`action`、`method`、`form`、`novalidate`、`formaction`、`formenctype`、`formmethod`、`formnovalidate`、`formtarget` |
| `LabelAttributes` | `V: HtmlTagCarrier`, `V::Tag: LabelTag` | `for_`→`for` |
| `AnchorAttributes` | `V: HtmlTagCarrier`, `V::Tag: AnchorTag` | `href`、`target`、`rel`、`download` |
| `MediaAttributes` | `V: HtmlTagCarrier`, `V::Tag: MediaTag` | `src`、`alt`、`width`、`height`、`autoplay`、`controls`、`loop_`→`loop`、`muted`、`poster`、`preload`、`srcset`、`sizes`、`loading`、`decoding`、`crossorigin` |
| `OpenAttributes` | `V: HtmlTagCarrier`, `V::Tag: OpenTag` | `open` |
| `TableCellAttributes` | `V: HtmlTagCarrier`, `V::Tag: TableCellTag` | `colspan`、`rowspan`、`headers` |
| `TableHeaderAttributes` | `V: HtmlTagCarrier`, `V::Tag: TableHeaderTag` | `scope`、`abbr` |
| `DataAttributes` | 通用 `AttributeBuilder` | `data_slot`、`data_state`、`data_orientation`、`data_disabled`、`data_value`、`data_side`、`data_align`、`data_active`、`data_open` |
| `PopoverAttributes` | 通用 `AttributeBuilder` | `popover`、`popovertarget`、`popovertargetaction` |

`DataAttributes` 的 `data-*` 方法表达应用扩展属性，不参与 HTML 标签分类。
`PopoverAttributes` 当前也保持通用；未来若引入 `PopoverTargetTag`，应拆出
`PopoverTargetAttributes`，并由 codegen 的显式标签表维护触发器标签。

高级扩展可以为自己的 `AttributeBuilder` 类型显式实现
`HtmlTagCarrier`，但这等同于声明该类型具备对应 HTML 标签能力；库不会从
运行时 tag name 自动探测，也不会替调用者验证自定义声明的正确性。

所有方法的签名本质上都是：

```text
fn method(self, value: impl IntoStorable<'scope>) -> Self
```

这意味着方法会消费当前 view 并返回带有新 pending operation 的 view。借用
值必须至少活到 `'scope`；`&String` 会在 `IntoStorable` 中 clone 成
`String`，从而避免保存临时的 `String` 引用。

## `attr`、`prop` 与命名 facade

命名 facade 统一使用 attribute 目标：

```rust
use silex_view::attribute::{AttributeBuilder, GlobalAttributes};
use silex_html::{FormAttributes, input};

let field = input()
    .id("email")
    .type_("email")
    .value("initial@example.com")
    .required(true);

let controlled_field = input().prop("value", "current@example.com");

let explicit_after_erasure = input()
    .value("before-erasure")
    .into_untyped()
    .attr("value", "after-erasure");
```

HTML attribute 描述 markup 属性或存在性；DOM property 表示控件当前状态。
特别是 `value`、`checked`、`disabled`、`required`，如果目标是同步浏览器
控件当前值，应使用 `.prop(...)` 或 `GlobalEventAttributes::bind_value`，
不要因为方法名相同就把 `.value(...)` 当成 property 写入。

`AttributeBuilder::attr` 和 `.prop` 都支持任意静态或响应式
`IntoStorable<'scope>`。`attr("readonly", ...)` 会在 DOM 应用层被识别为
attribute；`prop("readonly", ...)` 会进入 known property 的
`readOnly` 处理路径，这是两个不同目标。

## 值和清理语义

`IntoStorable` 当前支持借用/owned 字符串、`bool`、数字、字符、
`Option`、`Vec`、数组、`Attr`、`AttrOp`、`AttributeGroup` 和
`Rx`/`Signal`/`ReadSignal`/`Computed`/`StoredValue` 等响应式值。

对 attribute 目标，底层 `Attr` 有三种状态：

| 状态 | mount 时的动作 |
| --- | --- |
| `Attr::Removed` | `remove_attribute` |
| `Attr::Empty` | `set_attribute(name, "")` |
| `Attr::String(value)` | `set_attribute(name, value)` |

因此 `false` 和 `Option::None` 可移除 attribute，空字符串代表空 attribute；
这不是对所有 DOM property 的通用布尔语义。动态属性首次 mount 时建立 effect，
source 改变时更新，owner 关闭时撤销 effect 和动态贡献。

class/style 的合并也由 `silex_dom` 完成：动态 cleanup 只撤销当前 binding
的贡献，不应无条件删除其他静态或其他 binding 写入的值。需要完整解释
`CombinedClasses`、`CombinedStyles` 和错误处理时，参见
[`silex_dom` 属性、事件与响应式绑定](@/developer/crates/silex_dom/attributes.md)。

## 全局属性、ARIA 和事件

以下能力不是 `silex_html::attributes.rs` 声明的，而是从
`silex_view::attribute` 导入：

```rust
use silex_view::attribute::{
    AriaAttributes,
    AttributeBuilder,
    GlobalAttributes,
    GlobalEventAttributes,
};

let view = silex_html::button("Save")
    .class("primary")
    .aria_label("Save changes")
    .on_click(|| Ok(()));
```

`on_click` 的 callback 可以接收 `web_sys::MouseEvent`，也可以不接收参数；
`on_input`/`on_change` 则先读取 event target 的字符串值。callback 返回的
`SilexResult` 错误交给 mount error handler，listener 和 JS closure 绑定到
owner，owner 关闭后不会继续调用应用 callback。跨元素复用多组属性时，可用
`silex_dom::group!` 或 `AttributeBuilder::apply`，不要复制一套新的
`IntoStorable` 转换逻辑。

## 当前能力边界与迁移

错误标签调用会在编译期失败，例如 `div(()).href("/docs")`、
`span(()).value("wrong")`、`img().href("/docs")` 和
`td(()).scope("col")`。对应标签的 `TypedElement` 仍可保留原有链式返回
类型，并继续接受静态值、owned 值、`Option` 和响应式 `IntoStorable` 值。

`Element`、`AnyView`、返回 `impl View` 的组件结果和通用 mixin 没有唯一的
标签 marker，因此不再提供七个受限 facade。迁移时应在类型擦除前完成语义
方法；若目标确实是任意标签、自定义元素或 Web Component，则使用明确的
`.attr("value", value)`、`.prop("value", value)` 或 `.apply(value)`。
这种 escape hatch 不会声称调用者写入的 attribute 适用于当前标签。

这些 marker 只表达粗粒度能力分类，不是完整 WHATWG 内容模型数据库，也不
验证 attribute 值枚举或自动选择 attribute/property。全局属性、ARIA、事件
和 `bind_value` 由 `silex_view` 的通用 blanket impl 提供。
