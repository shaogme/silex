+++
title = "host 挂载、替换与错误"
description = "silex_bootstrap 的 AppHost 状态机、挂载事务、替换顺序和清理错误。"
weight = 20
+++

# host 挂载、替换与错误

`AppHost` 是 `MountedApp` 的应用入口包装。它把 caller-owned `Node`、一个
可选的 active session 和清理诊断 sink 放在同一状态机中，阻止两个应用同时
写入同一个 target，也让页面/JavaScript 适配层可以共享相同的错误契约。

## 构造与 API

```rust
let mut host = AppHost::new(target, CleanupSink::console());
host.mount(runtime, |context| {
    let handler = context.access().error_handler(|error| report(error))?;
    context.mount(view, handler)
})?;

let active = host.is_active()?;
let state = host.state();
host.unmount()?;
```

上面的片段依赖外层的 browser `target`、`runtime`、`view` 和 `report`，用于
说明 API 关系，不是独立的 CI 示例；独立示例见总文档中的
`docs/examples/silex_bootstrap/basic.rs`。

| 方法 | 前置状态 | 成功结果 | 失败后的状态 |
| --- | --- | --- | --- |
| `new(target, cleanup_sink)` | 无 | 创建 `Ready` host；不创建 owner。 | 不适用。 |
| `mount(runtime, builder)` | `Ready` | builder 成功、boundary 提交后进入 `Active`。 | clean rollback 回到 `Ready`；非 clean rollback 或 panic 为 `Poisoned`。 |
| `replace(runtime, builder)` | `Active` | 旧 app dispose 成功后挂载新 app，保持 `Active`。 | 旧 dispose 失败为 `Poisoned`；新 mount clean 失败回到 `Ready`。 |
| `unmount()` | 任意非重入状态 | `Disposed` 或无 active 时 `AlreadyUnmounted`。 | dispose 失败为 `Poisoned`。 |
| `is_active()` | 任意 | 返回 host/session/root owner 是否 active。 | 内部状态不一致时返回 `SilexError`。 |
| `state()` | 任意 | 返回当前 `HostState`，不读取 DOM。 | 不返回错误。 |
| `target()` | 任意 | 返回 caller-owned `Node` 的 clone。 | 不返回错误。 |

`mount`、`replace` 和 `unmount` 都是同步方法。`MountContext` 只在 builder
调用期间有效，不能把它、其中的 owner token 或 handler 保存到 host 外部。

## 状态和操作顺序

```text
Ready ── mount ──► Mounting ── success ──► Active
  ▲                  │                       │
  │                  └─ clean failure        │ replace
  │                                          ▼
  └──────── unmount success ◄── Disposing ◄──┘

rollback/disposing cleanup failure、panic 或 re-entry ──► Poisoned
```

`Mounting` 和 `Disposing` 是内部事务正在运行的状态。正常的单线程调用不会
在两个公开方法之间停留；但 builder、cleanup 或浏览器 callback 可能通过
事件重新进入 host，这时返回 `ReentrantOperation`。`AppHost` 会在自己的
边界中使用 `catch_unwind` 把 unwind panic 转成结构化错误，并把 host 标成
`Poisoned`；具体目标的 panic 策略仍决定 panic 是否可捕获。

## builder 和 ownership

`AppHost::mount` 的 builder 签名是：

```rust
for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>
```

挂载时实际发生以下步骤：

1. `MountedApp` 从传入的 `Runtime` 创建 root owner。
2. `silex_dom` 创建 detached `DocumentFragment` 和 mount boundary。
3. builder 通过 `MountContext` 创建 owner-bound view、effect、事件和 cleanup。
4. builder 成功后，boundary 完成并追加到 target；此时才发布 active session。
5. builder 失败、boundary 失败或 panic 时，先关闭 owner，再移除 staging 中
   已创建节点，并把结果放进 `MountError`。

这意味着 builder 返回 `Err` 时，代码不应自行清理已挂载的 view 或 target
全部 children；应用层只需要保留错误并检查 rollback report。target 中
caller 预先存在的节点不属于这次 boundary，不会被 `AppHost` 删除。

## `replace` 不是原子双活切换

`replace` 先从 host 取出旧 app，进入 `Disposing`，调用旧
`MountedApp::dispose`；只有返回 `Ok(())` 后才将 host 设回 `Ready` 并调用
新的 `mount`。因此：

- 旧清理失败时，新 builder 不会执行，旧 session 不会恢复，host 进入
  `Poisoned`；target 可能已经移除了旧 boundary，但清理报告仍必须记录。
- 新 builder 返回 recoverable error 且 rollback clean 时，旧 app 已被移除，
  host 回到 `Ready`，可以再次调用 `mount`。
- `replace` 在 `Ready` 状态返回 `NotMounted`，不会偷偷把它当成首次 mount。

需要无缝保留旧内容的产品级切换，应在应用层准备新数据/新 view，并把
“什么时候替换”作为单独的状态机设计；不能把 `replace` 当成两套 app 同时
active 的事务。

## 错误分层与重试

`AppHostError` 的变体与处理方式如下：

| 错误 | 含义 | 建议动作 |
| --- | --- | --- |
| `AlreadyMounted` | active host 重复调用 `mount`。 | 使用 `replace` 或先 `unmount`。 |
| `NotMounted` | `replace` 没有可替换的 app。 | 首次挂载使用 `mount`。 |
| `InvalidState { state }` | 内部 session 与状态不一致。 | 记录为框架故障，不继续复用。 |
| `Mount(MountError)` | builder、view、DOM、rollback 或 panic 失败。 | 读取 `primary()`、`rollback()` 和 `availability()`。 |
| `Dispose(DisposeError)` | root 或 boundary 清理失败。 | 读取 `report()`；host 已 poisoned。 |
| `ReentrantOperation` | mount/dispose 期间再次进入 host。 | 推迟外层操作，记录来源。 |
| `Poisoned` | host 已经不能安全创建新 session。 | 丢弃 host，按报告处理残留资源。 |

`MountError::can_retry()` 只有在 rollback report clean 时才为 `true`。不要
因为 `primary()` 是 recoverable 就直接重试：如果 root cleanup 或 boundary
清理失败，host 仍然是 terminal `Poisoned`。

`MountError::primary()` 与 `rollback()` 必须分别记录。`rollback()` 可能包含：

- `CleanupFailure`：`Root`、`ProvisionalOwner` 或 `MountBoundary` 的关闭失败；
- `boundary_errors()`：移除实际 DOM 节点时的 `SilexError`；
- `is_clean()`：两个集合都为空时才为 true。

`AppHostError::mount_error()` 和 `dispose_error()` 是不需要匹配所有变体的
只读访问器；需要把报告转移出去时，可继续使用底层 `into_parts()`。

## Drop 和外部移除

`AppHost` 没有自己的额外 DOM 清理实现；它被 drop 时，`active` 中的
`MountedApp` 负责关闭 root 和 boundary。由于 Drop 不能返回 `Result`，失败
只会经过构造时的 `CleanupSink`。应用入口应优先显式调用 `unmount`，这样
调用方能同步看到 `DisposeError`。

`target()` 返回的是节点句柄 clone，不代表 host 获得了 document 中该节点的
所有权。外部代码可以把 target 从 document 中移除；这不会让
`is_active()` 自动变为 false，但显式 `unmount` 仍会关闭 owner，并只移除
仍然属于本次 boundary 的节点。

## 对应测试

- `tests/app_host.rs`：重复 mount、clean rollback retry、replace 顺序、外部
  移除 target、Drop cleanup 和错误后的 poisoned 状态。
- `tests/error.rs`：`MountError`/`DisposeError` 中 primary 与 cleanup report
  的结构化保留。
- `tests/compile_fail.rs` 与 `tests/ui/`：builder scope escape 的编译期约束。
