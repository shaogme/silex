+++
title = "silex_view"
description = "Silex 的 backend-neutral View、mount、响应式属性和动态渲染层。"
template = "section.html"
sort_by = "weight"
+++

# `silex_view`

`silex_view` 将声明式 View、物理 DOM 写入、响应式更新和 owner 生命周期组合成
一套高层 mount API。它位于 `silex_dom` 之上：View 只接收显式注入的
`DomContext`，不携带 `web_sys` 或其他 browser concrete type；具体节点操作仍由
`silex_dom` 的 browser 或 SSR backend 执行。

```text
应用 builder / 组件
        │  View + MountBuilderContext
        ▼
   silex_view
   elements · attributes · events · flow · lifecycle
        │  DomContext + owner-bound requests
        ▼
   silex_dom
        │
   BrowserDom 或 SsrDom
```

## 稳定入口与核心类型

| 入口 | 用途 |
| --- | --- |
| `app::MountedApp` | 持有 runtime、DOM host 和一次可重复的 mount session；负责提交、回滚和 dispose。 |
| `app::MountBuilderContext` | `MountedApp::mount` 回调收到的应用级 context；创建 owner-bound DOM action，或把 View 挂到本次 mount 的 staging parent。 |
| `mount::View` | 可重复执行的 View 工厂契约，核心方法是 `mount(&MountContext)`。 |
| `mount::MountContext` | View 内部的 DOM target、逻辑 ancestry、owner、transaction 和错误 handler。 |
| `mount::MountInstance` | 一次 mount 返回的节点快照；它不拥有 owner，清理由 owner 完成。 |
| `elements::Element` / `TypedElement` / `AnyView` | 未类型化元素、带 `Tag` marker 的元素和类型擦除 View。 |
| `attributes::AttributeBuilder` | 链式写入 attribute、property、class、style、事件和 `NodeRef`。 |
| `events::DomEvent` / `EventDescriptor` | backend-neutral 事件 payload 与事件描述符。 |
| `flow::*` | 动态 View、稳定分支、indexed list、keyed list 和响应式 View。 |
| `lifecycle::MountOwnerToken` | 注册 effect、cleanup、owner state、NodeRef 和宿主资源。 |
| crate 根宏 | `chain!` 组合 View；`group!` 组合属性操作；`define_tag!` 生成 tag marker/builder；`view_match!` 将分支结果转为 `AnyView`。 |
| `mount::{Prop, PropInto, PropFixed, PropMissing, ViewCons, ViewNil}` | generated builder 的 prop 转换，以及无类型擦除的 View 组合。 |

应用代码通常从 `silex_view::prelude` 导入；需要实现自定义 View 或使用底层
mount context 时，再从 `mount`、`elements`、`attributes`、`events` 或
`lifecycle` 选择窄入口。

## 最小 View mount 流程

下面是 `MountedApp::mount` 回调中的真实 API 片段。它省略了 browser/SSR backend
的创建和 host 选择，因此不是独立可编译程序；完整的仓库示例是
`docs/examples/silex_dom/basic.rs`，由
`crates/silex_view/tests/docs_examples.rs` 编译验证。

```rust
fn build(context: &MountBuilderContext<'_>) -> SilexResult<()> {
    let error_handler = context.access().error_handler(|_| {})?;
    let view = Element::with_child("button", "Hello from silex_view").id("example");
    context.mount_unit(view, error_handler.view())
}
```

`mount_unit` 仍然返回 `SilexResult<()>`；它只是丢弃本次 View 的
`MountInstance`，并不跳过 owner 注册。挂载过程中产生的节点、effect、listener
和 NodeRef binding 由同一 owner 管理，成功提交后在 `MountedApp::dispose` 或下一次
mount 前清理。

## Feature flags 与边界

| Feature | 作用 |
| --- | --- |
| `browser` | 默认启用，并转发 `silex_dom/browser`。用于 browser adapter 和 Wasm 场景。 |
| `ssr` | 转发 `silex_dom/ssr`。用于内存 DOM、SSR 序列化和 native mount 测试。 |

两个 feature 可以同时启用。只验证 SSR 时使用
`--no-default-features --features ssr`；只做 browser 类型检查时使用
`--no-default-features --features browser`。元素事件在 SSR 中只产生 hydration
record，不会执行 callback；但 `bind_window_event` 在 SSR 中直接返回
`DomError::Unsupported`，不会产生 window hydration record。

## 生命周期、并发与安全边界

`MountOwnerToken`、`MountContext`、`NodeRef`、响应式 binding 和事件 lease 都绑定
到 mount scope，并依赖 `Rc`、`RefCell` 或单线程 backend。它们不是跨线程共享
状态；不要把它们发送到其他线程或保存为全局 context。

一次 mount 先在 fragment staging boundary 中创建和更新节点，builder 成功后先把
staging fragment 插入 host，再执行 root transaction 的 commit callbacks。失败时
清理 owner、listener、binding 和已插入 host 的 staging 节点；如果清理本身失败，
`MountError` 会标记应用为 poisoned，不能安全重试。builder panic 也会使
`MountedApp` poisoned。

`NodeRef<'scope>` 的生命周期 marker 防止它逃逸 mount scope；运行时的 generation
又保证旧分支清理不会清掉已经替换的新 binding。动态 row 使用 comment anchors
界定连续节点，并把 row content、effect 和 updater 绑定到对应 owner。

## 已知限制

- `MountAncestry::closest_logical_element` 当前始终返回 `DomError::Unsupported`；逻辑 ancestry 可以遍历，但尚未提供 selector matching。
- `StableBranch` 只按 key 判断是否替换；同 key 的 snapshot 改变不会重新执行 branch callback，需要在 branch 内使用响应式 View。
- SSR 元素事件只记录 hydration metadata，不执行 callback；SSR window event 直接返回 `Unsupported`，两者都需要 hydration/应用层按能力重新安装 browser listener。
- 所有 owner、NodeRef、event lease 和 backend context 都是单线程资源，不能作为 `Send + Sync` 状态跨线程共享。
- 没有经过基准测试时，本 crate 文档不对 mount、reconcile 或 DOM 写入给出延迟、吞吐或复杂度数字。

## 专题

- [View、元素与类型擦除](views.md)：`View`、`Element`、`TypedElement`、`AnyView` 和自定义 View。
- [挂载、提交与清理](mounting.md)：`MountedApp`、staging boundary、重试、poison 和 dispose。
- [属性、property 与响应式绑定](attributes.md)：attribute/property 分离、class/style 合并和双向 value 绑定。
- [事件与 backend-neutral payload](events.md)：元素事件、window 事件、事件 handler 和 SSR 记录。
- [owner 生命周期与 NodeRef](lifecycle.md)：effect、cleanup、state、宿主资源和作用域约束。
- [动态 View、分支与列表](flow.md)：动态 renderer、稳定 key、indexed/keyed list 和 row updater。
- [错误模型](errors.md)：mount、rollback、dispose 与 error handler 的结构化边界。
- [测试与验证](testing.md)：native/SSR、Wasm/browser、trybuild 和文档示例。

## 源码与测试索引

- crate facade：`crates/silex_view/src/lib.rs`
- 应用 mount：`src/app/handle.rs`、`src/app/boundary.rs`、`src/app/builder.rs`
- View kernel：`src/kernel/contract.rs`、`src/kernel/context.rs`、`src/kernel/transaction.rs`
- 元素：`src/kernel/elements/`
- 属性：`src/kernel/attributes/`
- 事件：`src/kernel/events/`
- 生命周期：`src/lifecycle/`
- flow 与 row：`src/flow/`
- SSR 集成测试：`crates/silex_view/tests/ssr_mount.rs`、`tests/kernel.rs`
- browser/Wasm 测试：`crates/silex_view/tests/browser.rs`
- 作用域编译失败测试：`crates/silex_view/tests/ui/`
