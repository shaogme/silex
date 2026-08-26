+++
title = "挂载、提交与清理"
description = "说明 silex_view 的 MountedApp、staging boundary、重试、poison 和 dispose。"
weight = 20
+++

# 挂载、提交与清理

`MountedApp` 是应用级 mount handle。它把 `Runtime`、`DomContext` 和 host 绑定在
一起，并把一次 `mount` 过程放入 fragment staging boundary；builder 完成后先把
staging 插入 host，再执行 root transaction 的 commit callbacks。若提交或 callback
失败，边界和 owner 会一起清理，因此失败的 mount 不会把半棵 View 留在真实 DOM 中。

## 创建 `MountedApp`

`MountedApp::new(runtime, dom, host, cleanup_sink)` 不立即验证 host；第一次
`mount` 会验证 host 是否属于给定 `DomContext`。需要在构造时验证时使用
`MountedApp::try_new`，它在 host 为 foreign context 时直接返回错误。

```rust
let app = MountedApp::new(
    Runtime::new(),
    dom.context(),
    host.node().clone(),
    CleanupSink::new(|report| {
        // 在宿主侧记录 Drop 阶段的清理诊断。
        let _ = report;
    }),
);
```

该片段省略了 `dom`、`host` 的 backend 创建和外层返回类型；`CleanupSink` 的回调
适合保存 `DropFailureReport`，不要在回调中再次触发同一个 app 的 dispose。

## `mount` 回调中的两个 context

`MountedApp::mount` 的闭包参数是 `MountBuilderContext<'scope>`，不是 View kernel
的 `MountContext<'scope>`。builder context 提供：

- `access()`：当前 owner scope 的 `OwnerAccess`，用于创建 signal 和 error handler；
- `dom()`、`parent()`：当前 backend context 和 staging parent；
- `owner()`：本次应用 session 的 `MountOwnerToken`；
- `dom_action()`：绑定当前 owner 的低层 DOM action；
- `mount(view, handler)` / `mount_unit(view, handler)`：把 View 挂到 staging parent。

`MountBuilderContext::mount` 创建一个 append target 和 root ancestry，再把 builder
的 transaction 与 error handler 交给 `MountContext`。View 内部收到的
`MountContext` 还可以用 `with_element`、`with_target`、`with_owner` 和
`with_error_handler` 创建子 context；这些 clone 共享同一 transaction 和生命周期
树，不会创建新的 document。

## 提交与 rollback 顺序

一次成功的应用 mount 大致经过以下边界：

1. 创建 root owner、staging fragment 和 mount transaction；
2. builder 在 staging 中创建节点、安装属性、effect、NodeRef 与 listener；
3. builder 返回 `Ok(())` 后，staging 记录当前 owned nodes；
4. staging fragment 插入 host，随后 root transaction 执行 commit callbacks；
5. session 保存到 `MountedApp`，状态变为 active。

builder 返回错误时，transaction 会 rollback，root owner 逆序关闭，host resource
和 NodeRef binding 先清理，再移除 staging 节点。若 staging 已插入 host 但 commit
callback 失败，清理边界会从 host 移除已拥有节点。`MountedApp` 会把主错误与清理
结果包装成 `MountError`。如果 rollback report clean，`MountError::can_retry()`
为 true；否则 `is_poisoned()` 为 true，应用不能再安全重试。

`MountContext::on_commit` 注册的 callback 只在 root transaction commit 时执行；
child transaction commit 只把 callback 合并到 parent，不会提前运行。callback 返回
的错误会交给 context 的 error handler，并作为 transaction commit 的首个错误返回。

## 重复 mount、dispose 与 Drop

`MountedApp::mount` 在已有 session 时，会先 dispose 旧 session，再开始新一轮
mount。因此重新 mount 的常规结果是 host 只保留新 View。显式
`MountedApp::dispose` 会关闭 owner、取消宿主资源、清理响应式 effect，最后移除
boundary 节点；没有活动 session 时重复调用是安全的。

`dispose` 返回 `SilexResult<()>`。清理失败会返回 `DisposeError` 并把 app 标为
poisoned；之后 `dispose` 不会再次尝试同一个 session。`MountedApp` drop 时仍会
尝试清理未结束 session；由于 Drop 不能返回错误，失败会转换为
`DropFailureReport` 交给 `CleanupSink`，sink 自身 panic 会被隔离并记录到 console。

## 状态与错误处理

| 情况 | `is_active()` | `is_poisoned()` | 后续 mount |
| --- | --- | --- | --- |
| 尚未 mount 或已成功 dispose | `Ok(false)` | `false` | 可以执行 |
| mount 成功 | `Ok(true)`（root owner 仍 active 时） | `false` | 会先 dispose 旧 session |
| 可回滚的 mount 失败 | `Ok(false)` | `false` | 可以重试 |
| builder panic、rollback/dispose 清理失败 | `Ok(false)` | `true` | 返回 poisoned mount error |

表中的 `is_active()` 可能返回 `SilexResult`，因为它会验证内部 session 一致性；
不要只用 `is_poisoned()` 推断 DOM 是否为空。mount 错误应检查
`SilexError::kind()` 中的 `ViewError::mount_error()`，并根据
`MountError::can_retry()` 决定是否重试，而不是根据错误字符串判断。

更多错误字段见[错误模型](errors.md)，NodeRef、effect 和 cleanup 的注册顺序见
[owner 生命周期与 NodeRef](lifecycle.md)。
