+++
title = "silex_view"
description = "Silex 的 View、Element、属性、事件、owner 与 mount kernel。"
template = "section.html"
sort_by = "weight"
+++

# `silex_view`

`silex_view` 是高层 DOM 组织层。它拥有 `View`、`Element`、typed tags、
属性 builder、事件 handler、owner、动态分支、列表、`MountedApp` 和错误状态；
物理节点与 backend 能力通过 `silex_dom::runtime::DomContext` 注入。

## 公共边界

```text
Element / View / AttributeBuilder / EventHandler
                       │
                       ▼
              MountContext + owner
                       │
                       ▼
             silex_dom::runtime::DomContext
                       │
                 browser / SSR
```

公共签名不携带 `web_sys` 或 `wasm_bindgen` 类型。browser 使用
`silex_dom::adapters::browser::BrowserDom::from_window()`，SSR 使用
`silex_dom::adapters::ssr::SsrDom::new()`；同一 View mount 契约
可以在两者上运行。

## 入口

| API | 说明 |
| --- | --- |
| `View<'scope>` / `MountInstance` | 可重复 mount 的高层契约和本次节点快照。 |
| `Element` / `TypedElement` / `Tag` | 构造 HTML/SVG 元素和能力约束。 |
| `AttributeBuilder` / `AttrOp` | attribute/property/class/style/event/node_ref。 |
| `MountBuilderContext` / `MountedApp` | 应用级 host、staging、commit、rollback 和 dispose。 |
| `MountContext` | 单个 View 的 target、ancestry、transaction、owner 和 handler。 |
| `MountOwnerToken` / `MountState` | effect、cleanup、动态状态和资源注册。 |
| `AnyView` / dynamic / list / row | 类型擦除、稳定 branch、keyed identity 和 row updater。 |
| `SsrDom` / `DomContext` | 通过 `silex_dom::adapters::ssr` 与 `silex_dom::runtime` 显式导入。 |

## 最小 mount

```rust
app.mount(|context| {
    let handler = context.access().error_handler(|_| {})?;
    context.mount_unit(
        Element::with_child("main", "hello").class("shell"),
        handler,
    )
})?;
```

`MountedApp::new` 的 host 必须是同一 `DomContext` 创建的 `DomNode`。builder
失败时，provisional owner、DOM、NodeRef 和 listener 一起回滚；cleanup report
为空才允许 retry。

应用 builder 内的 `MountBuilderContext::mount` 与 View kernel 的入口不同。
在自定义 `View` 或已有 `MountContext` 中，应把 View 作为借用值交给当前
context：

```rust
let view = Element::with_child("section", "content");
let instance = context.mount(&view)?;
context.mount_unit(&view)?;
```

`MountContext::mount` 和 `MountContext::mount_unit` 都接受 `&V`，因此调用方
可以复用已有 View，且不需要为 dispatch 创建额外 clone；`View::mount` 仍是
自定义 View 实现使用的底层 trait hook。

## SSR、事件与 NodeRef

SSR serialization 确定性转义文本/attribute/style，property 不进入 markup。
事件不会生成 `onclick` 等 attribute，而在 `SsrDom::hydration_records()` 中
保留目标 identity 和 `EventSpec`。`GlobalEventAttributes::node_ref` 在
mount 期间保存抽象节点，rollback/dispose 后清空。

### NodeRef 能力入口

`silex_dom::lifecycle::node_ref::NodeRef<'scope>` 只保存 backend-neutral 的 opaque
`DomNode`。`Some` 只表示当前存在逻辑 binding，不代表节点属于调用者选择的
backend、仍连接在 document 中或一定可以执行某项能力。需要 DOM 能力时必须使用
显式的 context：

```rust
let action = context.dom_action();
action.focus(&node_ref)?;
let element = action.with_context(|dom| node_ref.resolve_element(dom))?;
```

`MountDomAction` 是 owner-bound 的 mount/event callback 入口。它内部检查 owner
是否仍然 active，再把操作交给真实的 `DomContext`；不要把组件的
`SilexContext`/`RouterContext` 当成 `DomContext`，也不要把 `web_sys` 类型放进
组件或 facade 的公共参数。`focus` 成功只表示 backend 接受了该操作；未绑定、已
清理、跨 context、错误 node kind、detached 或 backend 不支持时会返回错误，而不是
把失败显示为成功。SSR 对 browser-only focus 返回 `Unsupported`，不会执行真实
browser 操作。

## Feature

| feature | 作用 |
| --- | --- |
| `browser` | 默认启用 `silex_dom/browser`，用于真实 browser adapter。 |
| `ssr` | 启用 `silex_dom/ssr`，不启用 web runtime。 |

SSR 边界命令：

```text
RUSTFLAGS='-D warnings' cargo test --locked -p silex_view \
  --no-default-features --features ssr
```

trybuild 使用 `tests/compile_fail.rs`，文档示例使用
`tests/docs_examples.rs`；browser 的 check、no-run、Firefox runner 和 nightly
build-std 细节见 [`silex_dom` 测试说明](@/developer/crates/silex_dom/testing.md)。
