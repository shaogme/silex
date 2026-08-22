+++
title = "属性、事件与响应式绑定"
description = "silex_dom 的 attribute/property 目标、class/style 合并、事件 handler 和作用域转换。"
weight = 40
+++

# 属性、事件与响应式绑定

`silex_dom::attribute` 把属性输入先保存成带 scope 的 `AttrOp`，等元素
真正 mount 时才安装到 DOM。这样同一套 builder 可以接收 `&str`、owned
`String`、`Option`、primitive、`Rx` 和自定义 DOM 操作，同时让响应式绑定
与元素 owner 使用同一条 cleanup 路径。

## builder 的四个基本入口

`AttributeBuilder` 是 `Element`、`TypedElement` 以及声明 `#[attrs]` 的组件
builder 的属性构造扩展 trait。`AnyView`、fragment、列表和其它通用 View
wrapper 不实现该 trait：它们没有可推断的唯一 DOM root。

| 方法 | 目标 |
| --- | --- |
| `attr(name, value)` | 普通 HTML/SVG attribute；`class`、`style` 和已知 property 名会走规范化目标。 |
| `prop(name, value)` | 直接写 DOM property；适合 attribute 与 property 语义不同的控件值。 |
| `on(event, callback)` | 根据 `EventDescriptor` 安装 typed DOM listener。 |
| `apply(value)` | 让 `ApplyToDom` 或 `AttributeGroup` 自己决定如何应用。 |

`GlobalAttributes` 和 `AriaAttributes` 在所有 `AttributeBuilder` 上提供
`id`、`class`、`style`、`role`、`aria_*` 等方法；`GlobalEventAttributes`
提供 `on_click`、`on_input`、`on_change`、`bind_value`、`node_ref` 和
owner-bound host 操作。`silex_html` 的 `FormAttributes`、`AnchorAttributes`
等 trait 只是同一 builder 的更具体命名 facade。

## `ApplyTarget` 的语义

```text
attr("value", x) ──► Known(Value) / Attr("value")
prop("value", x) ──► Known(Value) / Prop("value")
attr("class", x) ──► Class accumulator
attr("style", x) ──► Style accumulator
apply(mixin)     ──► ApplyToDom::apply
```

`ApplyTarget::attr` 和 `prop` 会把 `class`/`style` 转换成特殊目标，并把
`value`、`checked`、`disabled`、`readOnly`、`required` 识别成
`KnownProp` fast path。`KnownProp::parse` 接受 `readonly` 与 `readOnly`，
规范化输出名是 `readOnly`。

attribute 与 property 不要混用：HTML attribute 是字符串/存在性状态，
DOM property 可能直接改变控件当前状态。尤其是表单 `value`、checkbox
的 `checked` 和 `disabled`，应根据要同步的是初始 markup 还是当前控件
状态选择 `.attr` 或 `.prop`；`bind_value` 则同时安装 input listener 和
响应式 value effect。

## 静态值与 `Attr`

`Attr` 明确表示 attribute 的三种状态：

| 值 | DOM 行为 |
| --- | --- |
| `Attr::Removed` | `remove_attribute`。`Option::None` 和 `false` 可以转换到这里。 |
| `Attr::Empty` | `set_attribute(name, "")`，用于布尔标记存在但没有字符串值。 |
| `Attr::String(value)` | `set_attribute(name, value)`。 |

`bool` 的 `.attr` 转换为 Empty/Removed；空 `String`/`&str` 转换为 Empty；
`Option<T>` 的 None 转换为 Removed。这个规则只描述 attribute 目标，不能
把布尔 attribute 的存在性错误地理解成任意 DOM property 的布尔写入。

## class/style 合并

元素 mount 前会调用 `consolidate_attributes`，把同一元素上的 class/style
操作合并为一个 `CombinedClasses` 或 `CombinedStyles`：

- 静态 class token 去重；同名 `class_toggle` 以最后一个计划为准；
- 动态 class 字符串、class toggle 和静态 token 各自保留，更新动态部分
  不会删除其它来源的 static class；
- style 的静态 key 以最后一个静态值为准；同一 style property 的 reactive
  plan 以最后一个为准；dynamic stylesheet 与 property 分开保留；
- cleanup 按 binding 记录撤销动态贡献，测试中可见静态 `display` 或静态
  `color` 在 reactive style dispose 后仍保留。

常用写法如下：

```rust
let view = Element::new("div")
    .class("panel")
    .class_toggle("active", is_active)
    .style(("display", "block"))
    .style(("color", color.into_rx()));
```

这里的 `is_active`、`color` 必须是当前 `scope` 可接受的 reactive input；
片段依赖外层 owner，作为 API 形状示例，不是独立 CI 编译示例。要提交整段
动态 stylesheet，使用 `.style(rx_string)`；要提交一个 CSS property，
使用 `.style((name, rx_string))`。

## 响应式绑定的安装与清理

`IntoStorable<'scope>` 把输入转成可保存在 view 中的类型：

- `&str` 保留借用，要求其 lifetime 不短于 view scope；
- `&String` clone 为 `String`，避免保存临时引用；
- `Signal`、`ReadSignal`、`RwSignal`、`Computed` 和 `StoredValue` 转换为
  `Rx`；
- tuple、array、`Option`、`Vec` 和 `Prop` 递归转换其成员；
- `Attr`、`AttrOp`、`AttributeGroup` 可作为已构造的 DOM instruction。

响应式 plan 首次 mount 时运行 initial effect，source 变化时运行 update，
owner close 时运行 cleanup。source 读取错误进入传入的
`MountErrorHandler`；安装失败会回滚已经建立的 effect、DOM 和局部 owner。
不要把 `Rx` 当成 `'static` 配置保存，也不要在 view 之外继续使用已经关闭
scope 的句柄。

## 事件 handler

`EventDescriptor` 将事件名与 `web_sys` 事件类型绑定，例如 `click` 对应
`MouseEvent`、`input` 对应 `InputEvent`。`EventHandler` 支持两种回调形态：

```rust
button.on_click(|event| {
    record_client_x(event.client_x());
    Ok(())
});

button.on_click(|| {
    record_click();
    Ok(())
});
```

上面只展示 handler 的两种签名，`button` 和业务函数省略，因而不是独立
CI 编译示例。

回调的 `Result` 错误不会从浏览器 listener 同步返回，而是通过该 owner 的
error handler 报告。listener、JS `Closure` 和 completion destination 由
owner resource 注册表管理；owner close 会取消 callback 并移除
`remove_event_listener`。

`on_input`/`on_change` 先从 `Event.target` 提取 input、textarea 或 select
的 string value，再调用 handler；需要保留 target 不匹配错误时使用
`event_target_value_result`，不要用默认空字符串的 `event_target_value`
掩盖错误。`on_untyped::<E>` 允许自定义事件名，但 `E` 的 `JsCast` 必须
与真实 payload 一致；错误类型由调用者承担。

`bind_value` 是双向绑定：input listener 将 string 写回 `RxWrite`，owner
effect 将 signal 当前值写到 known `value` property。`T` 必须能从 String
构造且实现 `AsRef<str> + Clone + PartialEq`，以便完成两个方向的转换。

## `NodeRef` 与自定义操作

`.node_ref(node_ref)` 在 mount 时把元素 cast 为请求的 `N` 并 load 到
`NodeRef`；cleanup 时 clear。cast 类型不匹配是 `SilexErrorKind::Dom`，
owner 已失效时 clear 的 `NoSuchNode` 会被视为正常清理完成。

对于 mixin、theme variable 或非标准 DOM 操作，使用 `ApplyToDom`、
`AttrOp::new_scoped`、`AttrOp::custom` 或 `AttributeGroup`：

```rust
let attrs = silex_dom::group![
    ("data-state", "ready"),
    AttrOp::new_listener(|element| {
        element.set_attribute("data-source", "mixin")
            .map(|_| ())
            .map_err(silex_core::SilexError::fatal)
    }),
];

let view = Element::new("div").apply(attrs);
```

该片段省略了必要的 `use` 和外层 scope，只展示 instruction 组合方式，不是
独立 CI 编译示例。`new_scoped` 和 `custom` 的闭包接收
`(&Element, &MountContext)`；需要事件、timer 或响应式状态时，应从 context
取得 owner 和 error handler，并在该 owner 上注册资源。

属性阶段必须显式区分：

- `AttrOp::custom(...)` 是 staging 阶段，不能依赖 `is_connected`、真实
  `parent_element()` 或已经提交到文档的 DOM 状态；可以通过
  `context.ancestry()` 查询逻辑祖先。
- `AttrOp::on_commit(...)` 在 root transaction commit 后执行，适合布局、
  focus、stylesheet 注入和其它只能在真实 DOM 中完成的操作。

组件关系不要通过 `queue_microtask` 或 Fragment 的 DOM 遍历推断。提交回调
使用 `context.on_commit(...)` 注册，失败会交给 context 的 error handler，
而 rollback 会取消尚未执行的 callback。

`AttributeGroup` 通过 `group!` 把异构输入立即擦除为 `Vec<AttrOp>`，避免
自定义组件透传属性时产生递归泛型。组为空时是 `Noop`，只有一个操作时
会直接复用该操作，多个操作时形成 `Sequence`。

## 对应测试与维护风险

- `tests/reactive_attribute.rs`：字符串、借用值、Cow、class/style 合并、
  property restore 和失败 mount cleanup。
- `tests/host_resources.rs`：element listener 添加/移除、panic、window
  listener 替换和 owner scope 清理。
- `tests/owner.rs`：combined reactive style、错误 handler 和 owner close。
- `tests/ui/fail_pending_attribute_escape.rs`：属性闭包不能逃逸当前 scope。

修改属性合并时，要同时验证“一个 binding 的 cleanup 不会覆盖另一个
binding 的静态/动态贡献”；修改事件安装时，要验证 add/remove listener、
destination cancel 和 callback error 的顺序。
