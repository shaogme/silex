+++
title = "属性、事件与 NodeRef"
description = "silex_dom 的低层 attribute/property/event request，以及 silex_view 的高层 binding 边界。"
weight = 40
+++

# 属性、事件与 `NodeRef`

`silex_dom::attribute` 不构造高层属性 builder。它接收已经由
`silex_view::attribute` 解析好的 `AttributeRequest`、`PropertyRequest` 和
`AttributeValue`，把同一套操作交给 browser 或 SSR backend。

## 两层 API

```text
silex_view::AttributeBuilder
  attr / prop / class / style / on / node_ref
                │
                ▼
silex_dom::attribute::AttributeRequest
  AttributeTarget + AttributeValue
                │
                ▼
        DomContext::set_attribute
```

attribute、property、class 和 style 必须使用正确目标。`Removed`、空值和
字符串值的语义由 `AttributeValue` 表达；class token 更新只删除当前 binding
贡献的 token，不会破坏其它属性来源。

高层应用示例：

```rust
use silex_view::attribute::{AttributeBuilder, GlobalAttributes};
use silex_view::Element;

let view = Element::with_child("button", "save")
    .attr("data-state", "ready")
    .class("primary");
```

## 事件与 SSR omission

`silex_dom::event::EventSpec` 只描述名称、类别、bubbles 和 cancelable；
browser concrete event 类型以及 owner-bound callback 位于 `silex_view::event`。
SSR listener 注册是 inert 的：serialization 永远不生成 `onclick` 或其它
事件 attribute，而 `SsrDom::hydration_records()` 记录目标 backend、稳定节点
identity 和 `EventSpec`。

这使静态 HTML 与 hydration metadata 分离，也避免把闭包或 `web_sys` 值序列化。
事件 record 的 target 必须来自同一 `DomContext`；跨 backend 操作返回结构化
`DomError::CrossContext`。

## `NodeRef`

`silex_dom::node_ref::NodeRef<'scope>` 只保存抽象 `DomNode`，不暴露 browser
对象。`silex_view::GlobalEventAttributes::node_ref` 在 mount 后 set，在 owner
cleanup 时 clear。NodeRef 的生命周期不能超过当前 mount scope；跨 scope 保存
会被 trybuild 拒绝。SSR、browser 和 rollback 都必须验证成功 mount 期间可读，
builder 失败或 dispose 后为空。

## staging 与 commit

低层 request 本身不假设真实 document 已连接。高层 `AttrOp::custom` 在
staging 阶段运行，`AttrOp::on_commit` 只用于需要提交后物理状态的操作。错误
由 `DomError` 转为 `SilexErrorKind::Dom`，由当前 View owner 的 handler 报告。

相关源码：`silex_dom/src/attribute_backend.rs`、`silex_dom/src/event_backend.rs`、
`silex_view/src/attribute.rs`、`silex_dom/src/node_ref.rs`。
