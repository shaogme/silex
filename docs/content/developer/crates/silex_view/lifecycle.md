+++
title = "owner 生命周期与 NodeRef"
description = "说明 silex_view 的 owner、effect、cleanup、state、NodeRef 和作用域边界。"
weight = 50
+++

# owner 生命周期与 NodeRef

`silex_view` 把一个 View mount tree 的响应式 effect、事件 lease、DOM cleanup、
NodeRef binding 和本地状态交给 `MountOwnerToken<'scope>`。owner token 是可 clone
的 capability，不是新的独立生命周期；clone 仍指向同一个 local owner state。

## owner tree 与关闭顺序

`MountOwnerToken::child()` 创建 parent owner 的 child state；动态 branch 使用
`branch_child()` 创建同时拥有独立 runtime access 的 branch content owner。owner
关闭时先关闭 children，再逆序停止 effect，最后逆序执行 cleanup。元素 mount glue
利用这个顺序保证：NodeRef binding 和 event gate/lease 在物理 DOM 移除前处理，
自定义 parent cleanup 可以观察到已经清理的 child 状态。branch content 的
`MountOwnerToken` 标记为 `BranchContent`，其 `close()` 会清空本地 effect handle；
对应的 `OwnerChild` 由 `RowBlock::dispose()` 另行关闭 runtime。不要把 branch
content owner 的 `close()` 当成独立 runtime child 的最终关闭操作。

cleanup 返回 `SilexResult<()>`，但 owner close 需要继续执行后续 cleanup；错误会
先交给注册时的 error handler。handler 成功消费时，该 cleanup 错误不会进入
`CloseTransaction`；只有 handler 返回错误、handler panic，或 cleanup callback
本身 panic 时，owner close 才会把 `CloseError` 汇总到 close report，并在应用边界
进入 `CleanupReport`。因此 cleanup callback panic 也不会中止整个清理序列。

## effect、cleanup 与 owner state

`MountOwnerToken::effect(phase, callback, error_handler)` 注册一个可停止的
runtime effect；`effect_with_previous` 还会把上一次 callback 产生的值传给下一次
执行。普通 DOM reactive binding、动态 View 和 list 都通过这些 API 绑定 signal。

`on_cleanup(cleanup, error_handler)` 注册 close callback；callback 只执行一次，
且 owner close 幂等。`owner_state(value)` 返回共享的 `MountState<T>`，可通过
`with`、`update`、`take`、`replace` 和 `is_active` 访问。owner 关闭后，这些访问会
返回 `ReactiveError::NoSuchNode`；`take_for_cleanup` 是 cleanup 实现专用的无错误
取值入口。

```rust
let resource = /* 已由 DOM backend 创建的 HostResource<'static> */;
let handler = context.error_handler();
context.owner().on_cleanup(
    Box::new(move || resource.cancel().map_err(Into::into)),
    handler,
)?;
```

这是展示 owner API 的非独立片段，`resource` 的创建上下文已省略；实际对
`HostResource` 更推荐直接调用 `owner.track_host_resource(resource, handler)`。
owner 关闭后不要在 cleanup 中调用 `MountState::take()`、已经关闭 owner 的
`MountDomAction` 或其他受 owner gate 保护的 capability；若 cleanup 必须取出
owner state，应使用内部 cleanup 实现使用的 `take_for_cleanup()`，并自行承担其
无错误取值语义。

## `NodeRef` 与 generation

`MountOwnerToken::node_ref()` 创建带 `'scope` marker 的 `NodeRef`。在元素属性链中
使用 `.node_ref(reference.clone())` 会：

1. 在元素实际创建后绑定 opaque `DomNode`；
2. 注册返回的 `NodeRefBinding` cleanup；
3. owner close 时调用 `clear_if_current()`。

`NodeRef::get()` 返回当前 node 的 clone；`resolve_element(dom)` 通过调用方提供的
`DomContext` 转成 `DomElement`；`focus(dom)` 在 browser 中请求 focus，在 SSR
或 detached node 上返回结构化错误。NodeRef 不携带 backend context，因此 resolve
和 focus 时仍必须传入与当前 node 相同的 context。

每次 `bind_for_mount` 都递增 generation。动态分支替换旧节点后，旧 binding 的
cleanup 只会返回 `ClearOutcome::AlreadyReplaced`，不会清掉新 binding。这是
NodeRef 可安全复用在 dynamic View 和 list row 中的关键不变量。

```rust
let reference = context.owner().node_ref();
let view = Element::new("input").node_ref(reference.clone());
context.mount_unit(view, context.error_handler())?;
let element = reference.resolve_element(context.dom())?;
```

`NodeRef` 的 `get`、`resolve_element` 和 `focus` 都可能失败；不要在 mount 外层把
它当作永远存在的 DOM 引用。reference 逃逸创建它的 `'scope` 会被 Rust 类型系统
拒绝，仓库的 `tests/ui/fail_node_ref_scope_escape.rs` 专门验证该契约。

## owner-bound DOM action

`MountContext::dom_action()` 和 `MountBuilderContext::dom_action()` 创建
`MountDomAction<'scope>`。`with_context` 只在 owner active 时执行传入的
`FnOnce(&DomContext) -> DomResult<R>`；`focus(node_ref)` 是它对 NodeRef 的便捷
入口。相比直接 clone `DomContext`，dom action 额外提供 lifecycle gate，适合在
event callback 或 mount-time callback 中执行 owner-bound DOM 工作。

```rust
let action = context.dom_action();
let reference_for_event = reference.clone();
let view = Element::new("button").on_click(move |_| {
    action.with_context(|dom| reference_for_event.focus(dom))?;
    Ok(())
});
```

callback 捕获的 action 不能转换成 `'static`；owner dispose 后调用它会返回
`ReactiveError::NoSuchNode`。browser focus 还会检查 node 是否仍连接；SSR focus
则由 backend 返回 `Unsupported`。

## 宿主资源与 cleanup reporter

事件 listener 等 `HostResource` 应通过 `track_host_resource` 交给 owner；owner
close 会调用 `cancel`。低层 resource 的 `Drop` 只能尽力取消，不能把错误返回给
调用者，因此应用边界需要在显式 dispose/rollback 时优先执行 owner cleanup。

`MountedApp` 为 provisional owner 配置 cleanup reporter。owner close failure 会
进入 mount/rollback 的 `CleanupReport`；如果 app 在 Drop 阶段才发现未清理资源，
则由 `CleanupSink` 接收 `DropFailureReport`。不要把这些诊断当成可恢复的业务
返回值，也不要在 cleanup 中 panic 掩盖原始 mount 错误。

实现与测试入口：`src/lifecycle/token.rs`、`owner.rs`、`state.rs`、`context.rs`，
`tests/ssr_mount.rs` 的 cleanup 顺序/NodeRef 测试，以及 `tests/browser.rs` 的
focus、event dispose 和 scope 行为测试。
