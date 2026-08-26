+++
title = "属性、property 与响应式绑定"
description = "说明 silex_view 的 attribute/property 分离、class/style 合并和响应式属性。"
weight = 30
+++

# 属性、property 与响应式绑定

元素和 typed element 都实现 `AttributeBuilder`。builder 不立即修改 DOM，而是把
`AttrOp` 存入 View；元素 mount 时先用 `consolidate_attributes` 合并 class/style
来源，再在当前 `MountContext` 中应用。这样属性值可以是静态值、响应式值、数组、
`Option`、`AttributeGroup` 或自定义 callback。

## `attr`、`prop` 与 `apply`

`attr(name, value)` 和 `prop(name, value)` 都先把名称解析成 `ApplyTarget`，再决定
写普通 attribute、element property、`class` 或 `style`。`ApplyTarget::attr` 和
`ApplyTarget::prop` 会把 `class`、`style` 映射到专门的目标，并把已知 property
名称识别为 `KnownProp`：

| 输入名称 | `KnownProp` |
| --- | --- |
| `value` | `Value` |
| `checked` | `Checked` |
| `disabled` | `Disabled` |
| `readOnly` 或 `readonly` | `ReadOnly` |
| `required` | `Required` |

已知 property 和显式 `prop` 都走 `DomContext::set_property`；只有未被识别为
`KnownProp` 的 `attr` 名称才走 `set_attribute`。因此 `.attr("value", ...)`、
`.attr("checked", ...)` 等名称也会写 property，不能据此推断 SSR 会输出同名
HTML attribute。property 不会进入 SSR HTML。`bool` 作为 attribute 值时，`true`
写空 attribute，`false` 删除；作为 property 值时，除 `value` 外的空值语义是
布尔 `true`，`value` 的空值是空字符串。

```rust
let view = Element::new("input")
    .attr("data-state", "ready")
    .prop("value", "runtime value")
    .prop("checked", true)
    .attr("hidden", false);
```

`apply(value)` 使用 `ApplyTarget::Apply`，通常用于接收已经实现
`ApplyToDom` 的属性片段、`AttrOp` 或 `AttributeGroup`。对于 `(name, value)` 对，
`ApplyTarget::Apply` 会把 name 再次交给 `ApplyTarget::attr` 解析：未识别名称是
普通 attribute，`class`、`style` 和已知 property 则进入相应目标。需要写 CSS
时使用 `style`，不要把 pair 交给 `apply`。

## 全局、ARIA 和便捷 builder

引入 `GlobalAttributes` 后可使用 `id`、`class`、`style`、`title`、`lang`、`dir`、
`tabindex` 和 `hidden`。引入 `AriaAttributes` 后可使用 `role`、`aria_label`、
`aria_hidden`、`aria_expanded`、`aria_controls`、`aria_disabled` 和
`aria_checked`。这些方法只是 `attr` 的类型安全命名包装，不会改变 attribute
的 SSR 转义和清理规则。

`GlobalEventAttributes` 提供 `classes`、`class_toggle`、`node_ref`、常用
`on_click`/`on_input`/`on_change`、pointer/mouse 事件以及 `bind_value`。例如：

```rust
let view = Element::with_child("button", "Save")
    .id("save")
    .class("button")
    .aria_label("Save changes")
    .on_click(|| Ok(()));
```

上面代码省略了 error handler 与 mount 外层，属于 API 片段。事件 callback 的
`Result` 不能被省略；完整错误处理见[事件与 backend-neutral payload](events.md)。

## class 与 style 的合并

多个 class 来源会合并为一个 `CombinedClasses`：普通 `.class("a b")`、
`.classes(...)`、`.class_toggle(name, signal)` 和动态 class 字符串都会参与同一轮
effect。token 会按空白拆分、去重并排序后写入 backend-neutral class attribute。
cleanup 只恢复静态 class 来源，不保留已经由当前 owner 动态写入的 token。

多个 style 来源会合并为 `CombinedStyles`：`.style("color:red")` 作为静态声明，
`(property, value)` 作为单个 style property，动态 stylesheet 字符串则按分号和
冒号解析。合并后的来源按静态值、响应式 property、动态 stylesheet 的阶段写入
同一个 map；同一阶段中后写入的同名 property 会覆盖先前值，SSR 输出由有序 map
生成，因此输出顺序稳定。无效或不含冒号的动态 style 声明会被跳过。

```rust
let view = Element::new("div")
    .class("base")
    .class("wide compact")
    .style("color:red")
    .style(("display", "block"));
```

例如 `.apply(("display", "block"))` 的实际结果是普通
`display="block"` attribute，而不是 `style="display: block;"`。

class/style 的合并只发生在同一个元素 mount 的属性操作集合中；它们会从其他属性
操作中抽出并按固定阶段应用，所以不同类别的 builder 调用顺序不等于最终 DOM
写入顺序。class 来源形成去重后的 token 集合并按稳定顺序输出，不存在“后出现的
同名 class 覆盖前者”。直接调用 `set_class_value` 会替换整个 class 属性；需要
增删 token 并保留其他来源时使用 `update_class_tokens`。这两个函数都接收显式
`DomContext` 和 `DomElement`，不会自动加入 owner cleanup。

## 响应式值

实现 `IntoStorable` 的 `Signal`、`ReadSignal`、`Computed`、`StoredValue` 或
`Rx` 会被转换为 reactive binding。mount 时 binding 立即执行一次初始写入，之后
由 owner effect 监听 source；owner 关闭时 binding cleanup 删除或恢复它负责的
attribute/property/style。

普通值可直接用于 attribute：

```rust
let title = context.access().signal(String::from("before"))?;
let view = Element::with_child("div", "value").attr("title", title);
context.mount_unit(view, handler.view())?;
title.set(String::from("after"))?;
```

这是 mount 回调中的片段，`context` 与 `handler` 来自
`MountedApp::mount`；其中每个可失败操作都通过 `?` 传播。响应式文本 View、
`Signal<T>` 直接作为 View，以及动态闭包的规则见[动态 View、分支与列表](flow.md)。

## 双向 value 绑定

`bind_value(signal)` 同时做两件事：把 signal 作为 `prop("value", signal)` 写入
控件，并安装 `input` handler；handler 读取 `DomEvent::input_value()`，再调用
signal 的 `set`。因此它要求 source 同时满足 `RxGet` 和 `RxWrite`，而且值类型能
从 `String` 构造并可比较。

```rust
let value = context.access().signal(String::from("before"))?;
let view = vec![
    Element::new("input").bind_value(value),
    Element::with_child("output", value),
];
context.mount_unit(view, handler.view())?;
```

`bind_value` 只处理 input 事件中可取得的 value；不支持 value payload 的 backend
或事件不会凭空生成字符串。输入事件中的 signal 更新会再通过普通响应式 View
更新其他 consumers。

## 自定义属性与 commit phase

`AttrOp::custom(callback)` 在 staging phase 执行；`AttrOp::on_commit(callback)`
把 callback 注册到当前 `MountTransaction`，只有 mount transaction commit 才执行。
callback 可以返回 `SilexResult<()>`，失败会沿着当前 error handler 和 mount
transaction 返回。自定义属性 callback 收到的是 `&DomElement` 与
`&MountContext`，应继续使用 context 提供的 backend 和 owner，不应捕获另一个
context 的节点。

`group!(...)` 宏把多个 `ApplyToDom` 值打包成 `AttributeGroup`；`group` 函数则从
iterator 创建同样的 group。属性 group 只是操作集合，不承担独立的生命周期；
其中的 reactive binding 和 custom callback 仍归当前元素 owner 管理。

实现与测试入口：`src/kernel/attributes/model.rs`、`operation.rs`、`binding.rs`、
`apply.rs`、`tests/kernel.rs` 中的 attribute 测试，以及 `tests/browser.rs` 中的
browser property、class、style 和 `bind_value` 测试。
