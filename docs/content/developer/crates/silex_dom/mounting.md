+++
title = "挂载事务与回滚"
description = "silex_dom 的 MountedApp、staging boundary、提交、回滚和清理诊断。"
weight = 20
+++

# 挂载事务与回滚

`MountedApp` 把一次应用挂载分成“准备、构建、提交、清理”四个边界。
视图先写入脱离文档的 `DocumentFragment`，builder 成功后才把 boundary
一次性追加到 host；builder、视图或 DOM 操作失败时，已创建的 owner 和
节点会在返回 `MountError` 前回滚。这是应用级 API 与直接调用 `View::mount`
最重要的区别。

## `MountedApp` 的状态机

```text
Ready ── mount ──► Mounting ── success ──► Mounted
  ▲                  │                         │
  │                  └─ retryable error ◄──────┘ mount/dispose
  │                                            │
  └────────────── dispose success ◄──── Disposing

Mounting/Disposing re-entry、panic 或清理失败 ──► Poisoned
```

`MountedApp::new(runtime, host, cleanup_sink)` 只保存调用方提供的 runtime、
host 和清理 sink，不会在构造时创建 root owner 或 boundary。真正的 root
owner 在 `mount` 开始时由 `Runtime::owner()` 创建；这允许一个已 dispose
的 handle 再次挂载。

稳定入口如下：

| API | 语义 |
| --- | --- |
| `mount(builder)` | 先清理旧 session，再创建新 session；成功后进入 `Mounted`。 |
| `is_active()` | 只有已提交 session 且 root owner active 时返回 `true`。 |
| `is_poisoned()` | 清理失败、不可恢复错误、panic 或重入后返回 `true`。 |
| `host()` | 返回构造时传入的 host，即使当前没有 active session。 |
| `dispose()` | 关闭 root owner，再移除本次 boundary 节点；无 session 时幂等成功。 |

当 handle 已经 poisoned 时，`dispose()` 不会再次执行未知的清理序列并返回
一个新的错误；它保持 terminal 状态，`mount()` 会返回 poisoned 的
`MountError`。真正的清理诊断应在第一次 `MountError::rollback()` 或
`DisposeError::report()` 中读取。

## `MountContext` 与 builder

builder 的签名是：

```rust
for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>
```

`MountContext` 的公开能力是：

- `access()`：取得当前 root 的 `OwnerAccess<'scope>`，用于创建 signal、
  error handler 或其它 `silex_core` 节点；
- `owner()`：取得当前事务的 `MountOwnerToken<'scope>`，供高级 view adapter
  注册 DOM owner cleanup；
- `parent()`：取得 detached staging parent，适合需要直接插入 Node 的适配器；
- `mount(view, handler)`：挂载一个 view，并把它的返回节点纳入本次 boundary；
- `mount_instance(...)`：挂载并保留 `MountInstance` 的节点快照；
- `mount_with_attributes` / `mount_instance_with_attributes`：向一个顶层
  view 转发 `PendingAttribute`。

示例中的 error handler 必须来自同一个 `OwnerAccess`：

```rust
app.mount(|context| {
    let handler = context.access().error_handler(|error| record(error))?;
    context.mount(view, handler)
})?;
```

这里的 `record` 代表应用自己的错误收集逻辑；该片段省略了 host 创建和
`CleanupSink`，用于说明 handler 的来源，不是独立的 CI 示例。不要把
`MountContext`、`MountOwnerToken` 或 handler 保存到 builder 之外；HRTB
签名会阻止这种 scope escape。

## 提交与回滚顺序

一次正常 mount 的顺序是：

1. `Runtime::owner()` 创建 root owner。
2. `MountBoundary::new` 创建 `DocumentFragment` 和开始 anchor。
3. `root.with_access` 借出 `MountContext`，builder 将 view 写入 staging。
4. builder 返回 `Ok(())` 后追加结束 anchor，记录 boundary 拥有的节点。
5. `MountBoundary::commit` 把整个 fragment 追加到 host，session 才发布为
   `Mounted`。

失败路径的顺序相反但保持两个错误来源分离：

- builder 的 `SilexError` 是 `MountError::primary()`；
- root owner close 产生的 `CleanupFailure` 位于 `rollback().cleanup_failures()`；
- boundary 节点移除失败位于 `rollback().boundary_errors()`；
- rollback 报告为空时，`availability()` 是 `Retryable`，可以用同一个 handle
  重新 mount；报告不干净时，availability 是 `Poisoned`。

因此不要只格式化 `MountError` 的 Display 文本。需要决定是否重试时，使用
`can_retry()`/`availability()`；需要诊断清理问题时，遍历结构化 report。

## remount、dispose 和 Drop

如果当前是 `Mounted`，再次 `mount` 会先关闭旧 root、移除旧 boundary，
再开始下一次 mount。旧 session 清理失败会使 handle poisoned，新 builder
不会执行。`dispose` 成功后回到 `Ready`，可重复调用；已从 host 外部移除
host 节点时，boundary 只移除仍然属于它的节点，因此显式 dispose 仍可幂等
完成。

`MountedApp` 被 drop 时也会关闭 root 并移除 boundary，但 Drop 不能返回
`Result`。若发生清理失败，crate 会把 `DropFailureReport` 转为诊断并交给
构造时的 `CleanupSink`；sink 自身 panic 也会被隔离。应用若需要同步知道
清理是否成功，应显式调用 `dispose()`，不要依赖 Drop。

## `CleanupSink` 与错误所有权

`CleanupSink` 是一个可复制的、`'static` 的报告入口，专门接收 Drop 阶段
无法返回的 `DropFailureReport`。它不能捕获借用当前 mount scope 的闭包；
这条约束防止 sink 比 owner 活得更久。`CleanupReport` 则属于一次具体
mount/rollback/dispose，包含：

- `CleanupFailure`：root、provisional owner 或 mount boundary 的关闭错误；
- `boundary_errors()`：DOM 节点移除失败；
- `is_clean()`：两类错误都为空时为 true。

`MountError` 还提供 `rollback_error()`，把 primary error 和 rollback report
组合成一个可传递的 `RollbackError`。记录错误时保留这两个层次，以便区分
“业务 builder 拒绝挂载”和“拒绝之后的清理又失败”。

## 直接挂载与应用挂载的选择

高级框架或测试可以直接构造 `MountOwnerToken::new(access)`，再调用
`view.mount(&owner, parent, attrs, handler)`。这种方式适合嵌入已有 owner
树的 adapter，但调用方必须自己保证 owner close 和物理节点清理。

应用入口优先使用 `MountedApp`/`MountContext`：它提供 staging boundary、
host caller-owned 节点保护、rollback report 和可重试状态，避免把应用级
事务逻辑散落在每个 view 实现中。

## 对应测试

- `tests/mounted_app.rs`：staging/commit、caller-owned 节点、remount、外部
  host 移除、retry、dispose 和 rollback 顺序。
- `tests/mounted_contract.rs`：`MountError`、`DisposeError`、
  `CleanupReport`、`CleanupSink` 的错误所有权。
- `tests/owner.rs`：复合 view 失败时 provisional owner 和 DOM 的回滚。
- `tests/ui/fail_mounted_app_scope_escape.rs`、
  `fail_mounted_app_dispose_use.rs`：builder scope 和 disposed handle 的
  编译期边界。
