+++
title = "属性与 property"
description = "silex_dom 的 attribute、property、class token 和 style 写入模型。"
weight = 20
+++

# 属性与 property

`silex_dom` 有意把 HTML attribute、DOM property 和 style property 分成三条写入
路径。这样 SSR serializer 不会把只能存在于运行时对象上的 property 错误写进
HTML，也避免调用方通过一个模糊的字符串接口绕过安全的值模型。

本文的片段省略外层函数，只展示真实类型和 `Result` 传播；它们不是独立可编译的
CI 示例。

## AttributeTarget 与 AttributeValue

`AttributeTarget` 有三个稳定入口：`Named(String)`、`Class` 和 `Style`。推荐使用
`AttributeTarget::named(name)` 创建普通属性；`Class` 和 `Style` 会分别映射为
`class`、`style`。

`AttributeValue` 只有四种形式：

| 值 | 语义 |
| --- | --- |
| `Removed` | 删除 attribute。 |
| `Empty` | 保留 attribute，但值为空字符串。 |
| `Text(String)` | 写入普通文本值，可用 `AttributeValue::text` 创建。 |
| `ClassTokens { add, remove }` | 增加或删除 class token，不接受 raw HTML。 |

```rust
context.set_attribute(AttributeRequest::new(
    &element,
    AttributeTarget::named("data-state"),
    AttributeValue::text("ready"),
))?;

context.set_attribute(AttributeRequest::new(
    &element,
    AttributeTarget::Class,
    AttributeValue::ClassTokens {
        add: vec![String::from("active")],
        remove: vec![String::from("stale")],
    },
))?;
```

browser backend 对 `ClassTokens` 使用 `DomTokenList`；SSR backend 使用
`BTreeSet` 合并 token，因此 SSR 的 class 输出是去重且稳定排序的。该排序行为
是 SSR 实现细节，不应作为 browser `classList` 的顺序契约。

`ClassTokens` 在 browser backend 固定操作 `classList`，而 SSR backend 按
`AttributeTarget::name()` 更新属性。因此调用方应始终把 `ClassTokens` 与
`AttributeTarget::Class` 配对；当前类型模型没有阻止把它传给 named target，
这会造成 browser 与 SSR 语义不一致。

`AttributeRequest` 会 clone `DomElement`，但不会 clone backend DOM 节点本身或触发
写入。真正写入由 `DomContext::set_attribute` 委托给 backend，并在错误时返回
`DomResult<()>`。

## PropertyValue 与 SSR

`PropertyRequest` 的值通过 `PropertyValue` 表示：`Removed`、`String`、`Bool` 或
`Number(f64)`。browser backend 使用 JavaScript `Reflect::set`/`delete_property`
操作 element object；SSR backend 则把 property 保存在内存节点状态中。

property 不进入 SSR HTML。比如把 `value` 写成 property 不会产生
`value="..."`；若值必须出现在初始 markup 中，应显式发送
`AttributeRequest`。这一区分也是 hydration 时避免把运行时对象状态伪装成
服务器输出的依据。

```rust
context.set_property(PropertyRequest::new(
    &element,
    "value",
    PropertyValue::string("runtime value"),
))?;
```

`get_attribute` 是独立的读取能力：browser backend 从真实 `Element` 读取并返回
`Option<String>`；当前 SSR backend 没有实现该可选能力，会返回
`DomError::Unsupported`。SSR 中若需要验证输出，应读取 `serialize` 或
`serialize_node` 的结果，而不是把 property 状态当作 attribute 查询结果。

属性名或 property 名为空时返回 `DomError::AttributeNameEmpty`。调用方应在进入
DOM 层之前完成业务层的名称选择，不要依赖 backend 接受空名称。

## Style property

`DomContext::set_style_property(element, name, value)` 直接操作 element 的
`style` 对象；`Some(value)` 设置属性，`None` 删除属性。browser 仅支持具有
`HtmlElement` 或 `SvgElement` style 对象的元素；其他 element 会返回
`Unsupported`。

SSR 会读取当前 `style` attribute，把声明拆成 map，更新或删除目标声明，再写回
带分号的 style 字符串。SSR 的 map 是 `BTreeMap`，所以输出顺序稳定；非法或不含
冒号的现有声明不会被保留。

```rust
context.set_style_property(&element, "--accent", Some("blue"))?;
context.set_style_property(&element, "--accent", None)?;
```

## 安全与错误边界

`AttributeValue` 没有 raw-HTML 变体。serializer 对 text 和 attribute 值分别做
HTML 转义，因此不要把已经转义的字符串再次当作结构化 markup 传入；若确实要
注入 HTML，必须在更高层明确承担对应的安全和 hydration 风险，不能通过本 API
绕过检查。

所有请求都带有 `DomElement`，backend 会验证它属于当前 backend。跨 context、
错误节点类别、backend JavaScript 异常和不支持的 style 能力都会结构化为
`DomError`，而不是静默忽略。

对应实现和测试：`src/model/attribute.rs`、
`src/adapters/browser/attribute.rs`、`src/adapters/ssr/attribute.rs`、
`tests/ssr/attributes.rs`。上层响应式 attribute builder 的调用方式见
[`silex_view` 文档](@/developer/crates/silex_view/_index.md)。
