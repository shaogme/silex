+++
title = "错误模型"
description = "silex_dom 的 DOM 错误、backend 校验、能力缺失和清理诊断。"
weight = 60
+++

# 错误模型

`silex_dom::diagnostics::error` 重新导出 `silex_core::error::dom` 中的
`DomError` 和 `DomResult<T>`。DOM 操作不把失败转换成字符串或 panic，而是保留
context、节点类别、树关系、backend 和宿主能力等结构化信息。

## DomError 分类

| 错误 | 常见原因 |
| --- | --- |
| `CrossContext { expected, actual }` | 把不同 `DomContext`/backend 的 handle 混用。 |
| `InvalidHandle` | opaque handle 的 backend 或 kind 元数据失效。 |
| `Detached` / `NoParent` | 需要连接节点的能力（例如 browser `focus`）或移除操作发现节点没有 parent。 |
| `WrongNodeKind` | 需要 element/text/document，却传入了其他 node kind。 |
| `CannotContain` / `Cycle` | parent 不能有 child，或树操作会形成环。 |
| `CannotRemoveDocument` | 尝试移除 document root。 |
| `ParentMismatch` / `ReferenceNotChild` | range、reference 或 child 不属于声明的 parent。 |
| `AttributeNameEmpty` | attribute、property 或 style 名为空。 |
| `NodeRefBorrowed` / `NotBound` / `Cleared` | NodeRef 重入、未绑定或已清理。 |
| `BindingGenerationExhausted` | NodeRef generation 无法继续递增。 |
| `Unsupported` | 当前 backend 没有该能力，例如 SSR focus。 |
| `Backend { operation, message }` | JavaScript、序列化或 backend 内部操作失败。 |

调用方应按 variant 处理需要恢复的情况。例如 SSR 中不要把
`Unsupported(focus)` 当作 browser 失败重试；`CrossContext` 需要修正 context
归属；`ReferenceNotChild` 则需要重新读取当前树，而不是重复提交同一 request。

## 验证顺序与不变量

`DomContext` 的方法把 request 交给 backend，backend 再验证 handle 和树关系。
实现新的 backend 或 tree operation 时，至少保持以下顺序：

1. 验证 node 的 backend identity 和 concrete handle 是否有效；
2. 验证 node kind、parent 能力和 reference 归属；
3. 验证 cycle、range 顺序和不能移除的 root；
4. 只有检查通过后才执行 mutation 或调用外部 JavaScript。

SSR 测试覆盖了跨 context append、detached remove、wrong parent、wrong kind、
fragment、range 和 cycle 等错误。browser backend 还会验证底层 node 所属
`Document`，而不是仅比较 Rust wrapper 的指针；但普通 browser tree mutation
主要委托给原生 DOM，原生异常会被包装为 `Backend`，不能假定两种 backend 的
具体错误 variant 完全相同。

## Host resource 错误

`HostResource::cancel()` 返回 `DomResult<()>`，但它的状态门先从 Active 变为
Cancelled，再执行取消闭包。因此取消闭包失败时，资源仍不可再次取消，调用方
必须记录该 error；再次 `cancel()` 返回 `Ok(())` 只是表示资源已经 inert，不代表
第一次取消动作成功。

`finish()` 不执行取消闭包，只丢弃它。对 listener 使用 `finish()` 可能留下真实
browser listener，因此只有确认宿主资源已自然结束时才能使用 `finish()`；正常
dispose 应优先 `cancel()`。`HostResourceState::Inert` 虽然是公开枚举值，但当前
`HostResource` 的构造和状态转换路径不会产生该状态。

## NodeRef 与清理诊断

NodeRef 的错误说明当前逻辑绑定，而不是自动证明真实 DOM 状态：

- `NotBound` 表示从未有可操作 binding；
- `Cleared { generation }` 表示某代 binding 已由 `clear` 清理；
- `CrossContext` 和 `WrongNodeKind` 只有在 context 解析时才会出现；
- `NodeRefBorrowed` 说明内部 `RefCell` 借用冲突，需要修正重入，而不是重试循环。

mount/rollback 的清理失败由 `CleanupReport` 或 `DropFailureReport` 聚合。报告有
cleanup failures 和 boundary errors 两个通道；不要只记录 `Display` 字符串，也
不要在 Drop 路径用 `unwrap`/`expect` 将清理失败升级为 panic。

## 事件错误边界

`DomEventBridge::dispatch` 返回 `DomResult<()>`，但 browser JS callback 当前会
丢弃该返回值，因为事件回调不在 `listen()` 的同步 `Result` 调用栈中。bridge 或
上层 handler 必须主动把错误交给 reporter、console 或应用错误边界。SSR 不调用
bridge：监听创建时只验证 request、记录 `EventRecord`，取消时从记录集合中移除
对应 id。

## 调试建议

遇到 DOM 失败时，建议按以下顺序记录：

1. `context.backend_id()` 与 node 的 `backend_id()` 是否相同；
2. node 的 `kind()`、`parent()` 和 `identity()` 是否符合当前树；
3. request 中的 element、reference 和 range 是否来自同一 context；
4. backend 是否明确支持该能力，还是返回了 `Unsupported`；
5. 如果错误发生在 dispose/drop，分别读取 cleanup failures 和 boundary errors。

对应实现和测试：`crates/silex_core/src/error/dom.rs`、
`src/diagnostics/error.rs`、`src/runtime/host.rs`、`tests/node_ref.rs`、
`tests/ssr/tree.rs`、`tests/ssr/attributes.rs` 和 `tests/ssr/events.rs`。
