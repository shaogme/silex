+++
title = "错误模型"
description = "说明 silex_view 的 mount、rollback、dispose、poison 和错误处理契约。"
weight = 70
+++

# 错误模型

`silex_view` 的公开操作返回 `silex_core::SilexResult`，View 生命周期错误通过
`SilexErrorKind::View(Box<ViewError>)` 结构化保存。不要只根据 `Display` 文本判断是否
能够重试；应读取 `ViewError` 内部的 `MountError`、`RollbackError` 或
`DisposeError`。

## 错误层次

| 类型 | 内容 | 常见入口 |
| --- | --- | --- |
| `MountError` | mount 主错误、rollback `CleanupReport` 和 `MountAvailability` | `MountedApp::mount`、View mount 失败 |
| `RollbackError` | 主错误与回滚报告 | 需要将主失败和回滚失败一起向上转发时 |
| `DisposeError` | 完整 cleanup report | `MountedApp::dispose` |
| `ViewError` | 上述三类错误或 `Invariant` | `SilexErrorKind::View` 中的统一 wrapper |
| `SilexError` | 跨 crate 的 severity、kind 和 handler 传递容器 | 所有公开 `SilexResult` |

`MountError::primary()` 是触发失败的主错误；`rollback()` 是 staging、owner、
host resource 或 boundary 清理的报告。`rollback_error()` 可把两者组成一个
`RollbackError`。`DisposeError::report()` 和 `into_parts()` 可读取清理失败明细。

## Retryable 与 poisoned

`MountError::new(primary, report)` 只有在 `report.is_clean()` 时才产生
`MountAvailability::Retryable`；只要 rollback 有 cleanup failure 或 boundary
error，就会变为 `Poisoned`。对应的判断方法是 `can_retry()` 和 `is_poisoned()`。

`MountedApp` 还会在以下情况直接 poison：

- builder 或 mount callback panic；
- 在已经 `Mounting` 或 `Disposing` 状态中重入 `mount`；
- 旧 session dispose 失败；
- 显式 dispose 产生非 clean report。

可重试的业务/DOM 错误会移除 provisional 节点、清理 owner，并允许下一次
`mount`。poisoned handle 不应继续复用；`MountedApp::mount` 会返回 poisoned
error。`dispose` 在 handle 已 poisoned 时直接返回 `Ok(())`，这是幂等 no-op，
不会再次尝试清理；调用方应先通过 `is_poisoned()` 识别该状态。

```rust
let result = mounted.mount(|context| {
    let handler = context.access().error_handler(|_| {})?;
    context.mount_unit(Element::with_child("p", "content"), handler.view())
});

if let Err(error) = result {
    if let SilexErrorKind::View(view) = error.kind()
        && let Some(mount) = view.mount_error()
        && mount.can_retry()
    {
        // 只有 rollback clean 时才允许安排下一次 mount。
    }
}
```

该片段展示 View 错误分支，省略了 `mounted` 的 backend 创建；没有使用 `unwrap`
隐藏失败。实际代码还应记录 `mount.primary()` 与 `mount.rollback()`，而不是只记录
格式化后的字符串。

host 校验发生在进入挂载事务之前：`MountedApp::try_new` 和
`MountedApp::mount` 都会调用 `DomContext::validate_node`。因此 foreign host 等
错误直接以 `SilexErrorKind::Dom` 返回，不会被包装成 `ViewError`。新 mount 的
View/transaction 失败，以及替换旧 session 时的清理失败，才使用上述 `MountError`
wrapper：

```rust
match mounted.mount(|context| {
    let handler = context.access().error_handler(|_| {})?;
    context.mount_unit(Element::with_child("p", "content"), handler.view())
}) {
    Ok(()) => {}
    Err(error) => match error.kind() {
        SilexErrorKind::Dom(dom_error) => {
            // host/backend 校验等 DOM 错误。
            let _ = dom_error;
        }
        SilexErrorKind::View(view) => {
            let _ = view.mount_error();
        }
        _ => {}
    },
}
```

## owner 与 handler 错误

effect、event callback 和 cleanup 都带有 `MountErrorHandler`。callback 返回错误
时，runtime 会尝试通过 handler 分发；handler 自身失败或 panic 会被转换为
`ReactiveError::Handler`/`CloseError`，并在可返回的 mount/dispose 边界进入 report。
事件 bridge 的同步 listen 调用不能把 callback dispatch error 返回给原始 DOM
事件安装者，所以事件错误必须依赖 owner 绑定的 handler 和诊断路径。

`MountDomAction`、`MountState` 和 stale `RowUpdater` 带有 owner lifecycle gate；
owner inactive 后调用它们通常产生 `ReactiveError::NoSuchNode`。`NodeRef` 本身
不持有 owner gate：未绑定或已清理时，`get()` 和 `resolve_element()` 通常返回
`Ok(None)`，`focus()` 则按 logical state 返回 `DomError::NotBound`、
`DomError::Cleared` 或 backend DOM error。相反，`MountDomAction` 调用
`NodeRef::focus` 前会先检查 owner，因而可能先得到 `ReactiveError::NoSuchNode`。
NodeRef 直接访问与 `MountDomAction` 的 owner gate 不是同一类错误；这些错误都说明
scope/生命周期已经失效，不能通过重复调用同一个 capability
恢复，应重新 mount 或使用当前 branch/row 提供的 capability。

## 与 `silex_dom` 错误的关系

View 不重新定义 DOM backend 错误。`DomError::CrossContext`、`NoParent`、
`WrongNodeKind`、`Unsupported` 等会转换为 `SilexError` 并成为 mount primary，或
在 cleanup 中成为 report 项。实现自定义 View 时应保留这些结构化错误，不要
`format!` 后重新构造一个丢失 variant 的 framework 字符串。

需要区分 browser/SSR 能力时，应按 backend contract 处理：例如 SSR focus 的
`Unsupported` 不是应该无限重试的 mount 失败；foreign host 则应在
`MountedApp::try_new` 或 mount 开始处修正 context 归属。

## 诊断顺序

发生失败时推荐按以下顺序检查：

1. 读取 `SilexError::kind()`，确认是否是 `ViewError`；
2. 对 mount 读取 `primary`、`rollback`、`availability`；
3. 对 dispose 读取完整 `CleanupReport`，区分 owner close 和 boundary remove；
4. 如果是动态 row，再检查 key duplicate、row invariant 和 rollback 子错误；
5. 如果是事件/NodeRef，再检查 owner 是否 active、NodeRef generation 和 backend context。

实现与测试入口：`silex_core/src/error/view.rs`、`src/app/handle.rs`、
`src/app/boundary.rs`、`src/lifecycle/owner.rs`，以及 `tests/ssr_mount.rs`、
`tests/kernel.rs`、`tests/browser.rs` 中的 retry、poison、rollback 和 cleanup 测试。
