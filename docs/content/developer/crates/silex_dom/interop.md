+++
title = "backend 与互操作"
description = "silex_dom 的 browser/SSR backend、类型擦除和自定义适配边界。"
weight = 50
+++

# backend 与互操作

`silex_dom` 的公共 API 不直接暴露浏览器对象。`DomBackend` 是 object-safe 的
低层能力集合，`DomContext` 通过 `Rc<dyn DomBackend>` 注入它；browser 和 SSR
backend 共享相同的 model、request 和错误边界。

## BrowserDom

启用 `browser` feature 后，`BrowserDom::from_window()` 在 `wasm32` 下从当前
window 获取 document；也可以使用 `BrowserDom::new(document)` 注入指定的
`web_sys::Document`。`context()` 返回可 clone 的 `DomContext`，
`from_web_sys_node(node)` 则把一个属于该 document 的原始节点转换为 opaque
`DomNode`。`from_window()` 仅在 `wasm32` 目标提供；`new` 和
`from_web_sys_node` 则是 browser feature 下的公开 API。

```rust
#[cfg(target_arch = "wasm32")]
use silex_dom::{
    adapters::browser::BrowserDom,
    diagnostics::error::DomResult,
    runtime::DomContext,
};

#[cfg(target_arch = "wasm32")]
fn browser_context() -> DomResult<DomContext> {
    Ok(BrowserDom::from_window()?.context())
}
```

上面的片段只展示 browser-only 入口，不是独立 CI 示例。`from_window()` 可能因为
window 或 document 不存在而返回 backend error；浏览器测试应显式检查这些失败。

browser handle 内部保存 `web_sys::Node`，并通过当前 `BrowserDom` 的 `WeakMap`
为同一 JS node 分配 identity。每次操作都会验证它属于创建该 `BrowserDom` 时的
document。即使两个
`BrowserDom` 包装同一个 `Document`，它们的 `BackendId` 仍不同，句柄也不能混用。

browser listener 通过 `Closure<dyn FnMut(Event)>` 接入 JS。listener 的取消闭包
同时持有 callback，保证 callback 在 `remove_event_listener` 前仍存活；
`HostResource` 释放后才允许 callback 一起 drop。内部还使用
`unchecked_ref()` 把 Closure 交给 web-sys 的 callback 参数，这个边界依赖
web-sys 的签名与 callback 类型保持一致，调用方不应复制或绕过该适配层。

## SsrDom

启用 `ssr` feature 后，`SsrDom::new()` 创建确定性的 in-memory document。它提供：

- `context()`：执行与 browser 相同的节点和属性 API；
- `serialize(options)` / `serialize_node(node, options)`：输出转义后的 HTML；
- `event_records()` / `hydration_records()`：取得已注册 listener 的 hydration 元数据。

SSR backend 只保存逻辑树和 property 状态。它没有真实 `Window`、document body
或 layout，因此 `document_body()` 返回 `None`，`document_hidden()` 返回
`Ok(None)`；`focus`、active element、`contains`、attribute read 和 window
listener 等没有 SSR 实现的能力按 contract 返回 `Unsupported`。

常用可选能力的当前差异如下：

| 能力 | Browser | SSR |
| --- | --- | --- |
| `document_body()` | 返回当前 document body（可能是 `None`） | `Ok(None)` |
| `focus()` | 检查连接状态后调用 HTML element focus | `Unsupported` |
| `active_element()` | 查询 document 的 active element | `Unsupported` |
| `contains()` | 使用原生 `Element::contains` | `Unsupported` |
| `document_hidden()` | 返回 `Some(bool)` | `Ok(None)` |
| `get_attribute()` | 读取真实 attribute | `Unsupported` |
| `listen_window()` | 安装 window listener | `Unsupported` |

## DomBackend 契约

自定义 backend 需要实现 `DomBackend` 的 document、节点创建、树操作、属性、
property 和 element listener 等方法。trait 默认提供部分可选能力：
`document_body`/`document_hidden` 返回 `None`，style、attribute read、focus、
active element、contains 和 window listener 默认返回 `Unsupported`。

所有跨 trait 边界的参数都是 opaque handle 或 request struct。实现必须：

- 先检查 `BackendId`，拒绝来自其他 context 的 node；
- 在 downcast 具体 handle 前检查 node kind 和 handle 是否仍有效；
- 对 cycle、错误 parent、detached node、空名称和 unsupported capability 返回
  `DomError`；对于查询操作，应明确区分合法的 detached 结果（例如没有 parent）
  与真正的错误；
- 保持 `DomRange` 的闭区间验证、`HostResource` 的一次性取消和 event request
  的空名称校验语义。

外部实现需要返回 listener 的 `HostResource<'static>`，但其构造函数
`HostResource::with_cancel` 当前是 `pub(crate)`。因此公开 trait 虽然允许自定义
backend 类型实现大部分能力，第三方 crate 目前没有公开的 host resource 构造入口；
这是一项 API 可扩展性限制，不应在文档示例中伪造构造函数。

## 类型擦除与安全边界

`DomNode` 用 `Rc<dyn Any>` 隐藏具体 backend payload，同时保存 backend id、kind 和
identity。这个设计让 `DomContext` 与 `silex_view` 的公共签名同构，但安全不变量
不在 `Any` 本身：真正的保护来自每次 backend 操作前的 identity、kind 和底层对象
有效性检查。自定义 backend 不应从 `DomNode::raw` 复制或假设自己的 handle 类型。

crate 的公共 API 没有要求应用书写 `unsafe`；所有 JavaScript 类型转换都集中在
browser adapter。应用层应继续只使用 `DomNode`/`DomElement`/`DomEvent`，不要把
`web_sys` 引用存进长期存在的 View、NodeRef 或跨 backend request。

## Feature 与构建选择

| 目标 | 建议命令 |
| --- | --- |
| SSR/native | `cargo test -p silex_dom --no-default-features --features ssr` |
| 默认 browser API | `cargo check -p silex_dom` |
| 只检查 browser feature | `cargo check -p silex_dom --no-default-features --features browser` |
| Wasm 目标 | `cargo check -p silex_dom --target wasm32-unknown-unknown --no-default-features --features browser` |

对应实现：`src/runtime/backend.rs`、`src/runtime/context.rs`、
`src/adapters/browser/backend.rs`、`src/adapters/ssr/backend.rs`。
