+++
title = "生命周期与 NodeRef"
description = "silex_dom 的 NodeRef generation、host 资源清理和 drop 诊断。"
weight = 40
+++

# 生命周期与 `NodeRef`

`silex_dom` 不拥有上层组件的 mount 生命周期，但提供两个让上层实现安全清理的
原语：带作用域的 `NodeRef<'scope>`，以及可取消的 `HostResource<'scope>`。
`NodeRef` 只保存 opaque `DomNode`；它不会自动选择 context、检查节点是否连接，
也不会替上层 owner 管理节点。

本文的片段省略外层函数，仅展示真实 API。上层 mount、rollback 和 dispose 的完整
事务由 `silex_view` 实现，见[View 与 mount 的上层边界](views.md)。

## NodeRef 的状态

`NodeRef` 的逻辑状态是：

| 状态 | 含义 |
| --- | --- |
| `Unbound` | 尚未绑定节点。 |
| `Bound { generation }` | 当前有一个首次绑定的节点。 |
| `Replaced { generation }` | 当前绑定替换了上一代节点。 |
| `Cleared { generation }` | 当前绑定已被清理，记录最后一代 generation。 |

`NodeRef::new()` 创建空引用；`get()` 返回当前 node 的 clone，`set(node)` 直接
设置当前绑定，`clear()` 清除当前绑定。所有内部借用通过 `try_borrow`/
`try_borrow_mut` 完成；如果在已有借用期间重入，会返回
`DomError::NodeRefBorrowed`。

```rust
let reference = NodeRef::new();
reference.set(node.clone())?;
let current = reference.get()?;
reference.clear()?;
assert!(current.is_some());
```

`set` 和 `bind_for_mount` 都不会验证 node 属于哪一个 context。这样 NodeRef 可以
在 mount glue 中先保存 backend-neutral handle；只有 `resolve_element(context)`、
`focus(context)` 或其他实际 DOM 操作才会通过给定 context 验证 backend identity
和 node kind。

## Generation-aware cleanup

mount 代码应优先使用 `bind_for_mount(node)`，因为它返回与当前 generation 绑定的
`NodeRefBinding`：

```rust
let binding = reference.bind_for_mount(node)?;
// 新 mount 可能随后绑定另一代 node。
let outcome = binding.clear_if_current()?;
```

`clear_if_current()` 只有在 token generation 仍是当前 generation 时才清除；旧
mount 的 cleanup 在新 mount 已替换 binding 后返回 `ClearOutcome::AlreadyReplaced`，
不会误删新节点。重复清理当前已清除的代数返回 `AlreadyCleared`。

这是比单纯保存 `NodeRef` 再调用 `clear()` 更重要的边界：动态 branch 或重复 mount
可能让旧 cleanup 晚于新绑定执行。`NodeRefBinding` 也实现 `Clone`，但 clone 共享
同一 generation，不会获得新的清理代数。

## 解析与 focus

`resolve_element(context)` 在未绑定时返回 `Ok(None)`，绑定 text、comment 或
fragment 时返回 `WrongNodeKind`，绑定 foreign backend 时返回 `CrossContext`。
`focus(context)` 对未绑定返回 `NotBound`，对已 clear 的绑定返回带 generation 的
`Cleared`；最终是否支持 focus 仍由 backend 决定，SSR 会返回 `Unsupported`。

`Some(DomNode)` 不代表节点仍连接在 document，也不代表调用方传入的 context 能够
操作它。调用方必须处理 `DomResult`，不能把 NodeRef 当作永久有效的浏览器引用。

## CleanupSink 与 drop 诊断

`CleanupSink` 接收 `DropFailureReport`。报告把清理阶段的
`CleanupFailureDiagnostic` 和 boundary `SilexError` 分开保存：

- `cleanup_failures()` 返回带 `CleanupOrigin` 和 `CleanupDiagnostic` 的清理失败；
- `boundary_errors()` 返回 mount 或宿主边界的 `SilexError`；
- `is_clean()` 只有两类集合都为空时才为 true；
- `CleanupSink::console()` 将报告写入跨目标 console，`new` 可注入测试或宿主回调。

Drop 路径不能把错误返回给调用者，也不应让清理 panic 穿透；上层可以在显式
dispose/rollback 时先处理 `CleanupReport`，再把 Drop 阶段遗漏的诊断交给 sink。
`HostResource` 的 `Drop` 只会尝试取消并丢弃取消错误，不会自动把错误发送到
`CleanupSink`；需要保留 drop 阶段错误时，必须由拥有者在可返回错误的边界显式
取消，或在自己的 drop/诊断路径中构造 `DropFailureReport`。`CleanupSink` 的
callback 是 `'static` 且通过 `Rc` 保存，因此仍是单线程资源。

## 与 HostResource 的关系

listener、window listener 等宿主资源由 `HostResource` 持有；上层 owner 应保存它，
在正常 dispose 或 rollback 中显式调用 `cancel()` 并处理 `DomResult`。资源 drop
会尝试取消，但 drop 不能返回取消错误；错误必须由 sink 或上层日志路径收集。

`finish()` 只把资源标记为 Finished 并丢弃取消闭包，不会执行取消动作。对 browser
listener 直接调用 `finish()` 不会移除真实 listener；只有 backend 自己确认资源
自然结束时才适合使用它。

## 所有权与上层边界

`NodeRef<'scope>` 的 marker 防止引用超出创建它的 scope；`NodeRefBinding<'scope>`
同样受 scope 约束。生命周期检查由 Rust 类型系统与运行时 context 校验共同完成：
前者防止能力逃逸，后者拒绝 detached、foreign 或错误类别的 handle。

不要把 `DomNode`、`NodeRef` 或 `HostResource` 放进跨线程共享状态；它们依赖
`Rc`/`RefCell` 和显式单线程 backend。需要让它们随组件销毁时结束，应把清理权交给
`silex_view` 的 mount owner，而不是创建一个全局 clone。

对应实现和测试：`src/lifecycle/node_ref.rs`、`src/lifecycle/cleanup.rs`、
`src/runtime/host.rs`、`tests/node_ref.rs` 以及 `silex_view/tests/ui/` 中的
NodeRef scope 编译期契约。
