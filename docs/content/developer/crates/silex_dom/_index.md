+++
title = "silex_dom"
description = "Silex 的 backend-neutral DOM、事件和宿主资源边界。"
template = "section.html"
sort_by = "weight"
+++

# `silex_dom`

`silex_dom` 是 Silex 的低层 DOM 适配层。它把节点、元素、文档、属性、事件
和宿主资源抽象成不携带 `web_sys` 类型的 Rust API，并把具体操作委托给
browser 或 SSR backend。上层的 `silex_view` 负责 View、mount、响应式更新和
组件 owner；`silex_dom` 只负责这些上层操作所依赖的物理 DOM 边界。

## 在 Silex 架构中的位置

```text
silex_view / router / css
        │  DomContext + backend-neutral requests
        ▼
     silex_dom
  model · runtime · lifecycle
        │
        ├── BrowserDom → web_sys / JavaScript DOM
        └── SsrDom     → in-memory tree / HTML serializer
```

这里的核心边界是显式注入的 `DomContext`。context 保存一个
`Rc<dyn DomBackend>`，调用方不需要知道 backend 的具体节点类型；每个节点句柄
都带有 `BackendId`，因此来自不同 context 的节点不能混用。

## 稳定入口与核心类型

| 入口 | 用途 |
| --- | --- |
| `DomContext` | 创建、查询、移动和删除 backend-neutral 节点；设置属性、属性值和事件。 |
| `DomBackend` | 自定义 DOM backend 所需实现的 object-safe 能力集合。 |
| `BrowserDom` | 从 `web_sys::Document` 或浏览器 `Window` 创建 browser context。 |
| `SsrDom` | 创建内存 DOM、序列化 HTML，并保存 hydration event records；需要 `ssr` feature。 |
| `DomNode` / `DomElement` / `DomDocument` | 分别表示任意节点、元素和文档的 opaque handle。 |
| `ElementSpec` / `Namespace` / `NodeKind` | 描述元素名称、命名空间、void 标记和节点类别。 |
| `DomRange` | 表示同一父节点下的闭区间，可整体移除或移动。 |
| `NodeRef<'scope>` | 保存上层 mount 逻辑绑定的当前节点，并用 generation 防止旧清理误删新绑定。 |
| `HostResource<'scope>` | 代表 listener 等宿主资源；`cancel` 幂等，`Drop` 会尝试取消。 |
| `DomError` / `DomResult<T>` | DOM、backend、context、NodeRef 和不支持能力的结构化错误。 |

公共模块按职责划分为 `model`、`runtime`、`adapters`、`lifecycle` 和
`diagnostics`。crate 根只声明这些模块，不提供全局 DOM context 或隐式当前
document。

## 最小 SSR 流程

下面的示例来自 `docs/examples/silex_dom/ssr.rs`，由
`crates/silex_dom/tests/docs_examples.rs` 编译并执行。它使用 `?` 传播每个
DOM 操作的错误，没有用 `unwrap` 或 `expect` 隐藏失败路径。

{% set source = load_data(path="examples/silex_dom/ssr.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

构建该示例时使用 `--no-default-features --features ssr`。browser 应用则使用
`BrowserDom::from_window()` 或 `BrowserDom::new(document)`，并把得到的
`context()` 注入上层 View。

## 生命周期与并发边界

`DomContext`、`DomNode`、`NodeRef` 和 host 资源都使用 `Rc`、`Cell` 或
`RefCell`；它们是单线程能力，不应当发送到其他线程，也不是 `Send + Sync` 的
共享状态。context 的 clone 只是共享同一个 backend，不会创建新的 document。

节点的生命周期由 backend 的树和上层 mount owner 共同决定。`DomNode` 的 clone
只复制 opaque handle；它不会把节点插入树，也不会改变节点是否连接在 document
中。调用 `DomContext` 的操作时，backend 会验证 context identity、handle 和
`NodeKind`；需要父节点的操作还会验证当前树关系。跨 context、错误类别、移除
没有 parent 的节点或对 detached 元素执行 browser `focus` 时，会返回相应的
`DomError`，但 `parent`/`children` 等查询并不会因为 detached 而统一失败。

事件 listener 返回 `HostResource`。它先关闭 active gate，再执行一次取消动作；
取消动作失败不会重新打开资源，后续 `cancel` 仍是 inert。上层 mount 代码应在
dispose 或 rollback 边界显式保存并取消这些资源。

## Feature flags

| Feature | 作用 |
| --- | --- |
| `browser` | 默认启用；编译 `BrowserDom`、`web_sys` 和 `js_sys` 适配器。 |
| `ssr` | 启用内存 SSR backend、serializer 和 SSR event records。 |

两个 feature 彼此独立且可以同时启用。只做 SSR 或 native 测试时，推荐使用
`--no-default-features --features ssr`，避免把 browser adapter 当作运行时依赖。
`silex_view` 和 `silex_css` 也分别通过自己的 `browser`/`ssr` feature 转发对应
的 `silex_dom` feature；使用这些上层 crate 时应选择同一套目标 feature。

## 专题

- [节点树与渲染](@/developer/crates/silex_dom/rendering.md)：节点创建、插入、范围移动、命名空间和 SSR 序列化。
- [属性与 property](@/developer/crates/silex_dom/attributes.md)：安全值模型、class token、style 和 SSR 输出。
- [事件与宿主资源](@/developer/crates/silex_dom/events.md)：事件描述、bridge、browser 控件和 listener lease。
- [生命周期与 NodeRef](@/developer/crates/silex_dom/lifecycle.md)：generation binding、清理 sink 和上层 owner 边界。
- [backend 与互操作](@/developer/crates/silex_dom/interop.md)：`web_sys` 类型擦除、自定义 backend 和 browser/SSR 差异。
- [错误模型](@/developer/crates/silex_dom/errors.md)：context 校验、能力缺失、回滚和 drop 诊断。
- [测试与调试](@/developer/crates/silex_dom/testing.md)：SSR/native 测试、Wasm 检查和文档示例。
- [View 与 mount 的上层边界](@/developer/crates/silex_dom/views.md)：为什么 View 和 mount API 属于 `silex_view`。

## 源码、示例与测试索引

- 公共模块：`crates/silex_dom/src/lib.rs`
- backend 与 context：`src/runtime/backend.rs`、`src/runtime/context.rs`
- 节点模型：`src/model/node/`、`src/model/attribute.rs`、`src/model/event/`
- browser adapter：`src/adapters/browser/`
- SSR adapter：`src/adapters/ssr/`
- NodeRef 与清理：`src/lifecycle/node_ref.rs`、`src/lifecycle/cleanup.rs`
- 可执行示例：`docs/examples/silex_dom/ssr.rs`
- 节点与 NodeRef 集成测试：`crates/silex_dom/tests/node_ref.rs`
- SSR 树、属性和事件测试：`crates/silex_dom/tests/ssr/`
- 上层 View 文档示例：`docs/examples/silex_dom/basic.rs`，由
  `crates/silex_view/tests/docs_examples.rs` 复用测试

## 已知限制与维护注意

- `silex_dom` 没有全局 context，也没有 mount、dispose、响应式 effect 或 View
  类型；这些职责由 `silex_view` 和 `silex_core` 承担。
- `DomNode` 通过 `Rc<dyn Any>` 保存 backend 实例。类型擦除让公共 API 保持同构，
  但 backend 实现必须在 downcast 前验证 `BackendId` 和节点类别；自定义 backend
  不应绕过 `DomBackend::check_node`。
- SSR listener 只记录 `EventRecord`，不会执行 callback；window listener 在 SSR
  中返回 `Unsupported`。hydration 层必须自行使用 record 重新安装 browser listener。
- browser event callback 当前无法向同步 `listen` 调用方返回 bridge dispatch 错误；
  bridge 的上层实现需要自行把错误交给 `silex_core` handler 或宿主诊断。
- `PropertyValue` 永远不序列化为 HTML attribute；需要进入 SSR markup 的值必须使用
  `AttributeRequest`。`AttributeValue` 也没有 raw-HTML 变体，不能借它绕过转义。
- 没有基准数据时，本 crate 文档不对 DOM 操作写入延迟、吞吐或复杂度数字。

验证本页和 `silex_dom` API 时，至少运行 [测试说明](testing.md) 中的
`cargo fmt`、`cargo check`、SSR 测试和 `zola --root docs check`。
