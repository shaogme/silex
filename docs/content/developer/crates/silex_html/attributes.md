+++
title = "属性 facade 与 DOM 目标"
description = "silex_html 命名属性 trait、IntoStorable、attribute/property 和响应式绑定边界。"
weight = 30
+++

# 属性 facade 与 DOM 目标

`silex_html::attributes` 只提供面向 HTML 的命名方法。它们通过
`silex_dom::attribute::AttributeBuilder::attr` 创建 `AttrOp`，在标签真正
mount 时才应用到 DOM。属性的生命周期、响应式 effect、事件资源和清理由
`silex_dom` 的 owner 管理。

这些 trait 不改变 `AttributeBuilder` 的核心接口：需要动态选择目标时仍可
直接调用 `.attr(name, value)`、`.prop(name, value)` 或 `.apply(value)`。
命名方法只是固定 HTML attribute 名称的便捷 facade。

## trait 分组

这些 trait 都对实现 `AttributeBuilder<'scope>` 的类型提供 blanket impl，
所以可以链式调用在 `TypedElement`、untyped `Element` 或支持属性转发的
view 上：

| trait | 方法到 HTML name 的映射 |
| --- | --- |
| `FormAttributes` | `type_`→`type`、`value`、`checked`、`disabled`、`readonly`、`required`、`placeholder`、`name`、`autocomplete`、`autofocus`、`min`、`max`、`step`、`pattern`、`multiple`、`accept`、`selected`、`rows`、`cols`、`action`、`method`、`form`、`novalidate`、`formaction`、`formenctype`、`formmethod`、`formnovalidate`、`formtarget` |
| `LabelAttributes` | `for_`→`for` |
| `AnchorAttributes` | `href`、`target`、`rel`、`download` |
| `MediaAttributes` | `src`、`alt`、`width`、`height`、`autoplay`、`controls`、`loop_`→`loop`、`muted`、`poster`、`preload`、`srcset`、`sizes`、`loading`、`decoding`、`crossorigin` |
| `OpenAttributes` | `open` |
| `TableCellAttributes` | `colspan`、`rowspan`、`headers` |
| `TableHeaderAttributes` | `scope`、`abbr` |
| `DataAttributes` | `data_slot`、`data_state`、`data_orientation`、`data_disabled`、`data_value`、`data_side`、`data_align`、`data_active`、`data_open` |
| `PopoverAttributes` | `popover`、`popovertarget`、`popovertargetaction` |

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
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex_html::{FormAttributes, input};

let field = input()
    .id("email")
    .type_("email")
    .value("initial@example.com")
    .required(true);

let controlled_field = input().prop("value", "current@example.com");
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
`Signal`/`ReadSignal`/`RwSignal`/`Computed`/`StoredValue` 等响应式值。

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
`silex_dom::attribute` 导入：

```rust
use silex_dom::attribute::{
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

## 当前能力边界

`FormTag`、`AnchorTag`、`MediaTag` 等 marker 是生成标签的分类 metadata，但
`silex_html` 的属性分组实现是 `impl<T: AttributeBuilder> Trait for T`，因此
编译器当前不会阻止 `div().href(...)` 或 `span().value(...)`。文档和调用
代码应把这些 trait 当作命名 facade，而不是完整的 HTML 内容模型校验。

这也是维护时的重要取舍：如果未来要按标签限制方法，不能只修改
`attributes.rs` 的宏列表，还需要重新设计 trait 与 `TypedElement<T>` marker
的约束，并补充 compile-fail 契约；否则会破坏现有自定义 view 和属性转发代码。
