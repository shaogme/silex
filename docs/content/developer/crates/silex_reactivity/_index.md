+++
title = "silex_reactivity"
description = "显式、单线程、带词法作用域约束的细粒度响应式运行时。"
template = "section.html"
sort_by = "weight"
+++

# `silex_reactivity`

`silex_reactivity` 是 Silex 的底层响应式引擎。它把 signal、computed、effect、watch、callback 和清理任务注册到显式的 owner 作用域中，并通过运行时图谱维护依赖和调度。

它解决的是两个相互关联的问题：一方面，状态读取需要能够精确地订阅真正使用到的源；另一方面，节点、回调和异步结果不能在所属作用域结束后继续访问已经释放的状态。公开句柄因此同时携带节点身份和创建作用域的 Rust 生命周期，运行时还会在操作时检查 owner、节点代数和 scheduler 身份。

## 先理解三层边界

可以把一次响应式操作看成下面三层：

```text
Runtime
└── owner / transient owner
    └── OwnerAccess<'owner>
        └── signal、computed、effect、watch、callback、cleanup
```

- `Runtime` 是显式的、单线程的执行边界，不提供隐式线程局部 runtime。
- owner 是生命周期和清理树的边界。`OwnerHandle` 持有关闭权限，`OwnerAccess` 是借用的节点创建和操作视图。
- 节点读取和写入通过 scheduler 进入响应式图。普通读取可以建立依赖，untracked 读取只取得当前值而不建立边。

`Runtime` 使用 `Rc`、`Cell` 等单线程类型，不应当作为 `Send + Sync` 的全局状态容器。一个 `Runtime` 同时只允许一个活动的 root owner；没有 root owner 时，可以使用 `Runtime::with_transient` 运行一次自动关闭的临时作用域。

## 节点选型

| 类型 | 用途 | 是否参与依赖图 |
| --- | --- | --- |
| `ReadSignal` / `WriteSignal` | 将可变源拆成读能力和写能力 | signal 的读会追踪，写会通知 |
| `Signal` | 同时持有读写能力的成对句柄 | 同上；可用 `read()`、`write()` 拆分 |
| `Computed` | 缓存一个由其他节点派生的值 | 是；默认按 `PartialEq` 判断输出变化 |
| `EffectHandle` | 执行副作用并根据读取重新运行 | 是；创建时先运行一次 |
| `watch_getter` | 将 getter 和变化回调分离 | 只有 getter 读取追踪 |
| `Callback` | 作用域拥有的类型化可调用节点 | 不会自动把参数变成依赖 |
| `StoredValue` | 存放不需要响应式通知的作用域数据 | 否 |
| `NodeRef` | 存放可选的宿主对象引用 | 否 |

`computed` 要求输出实现 `PartialEq`，只有输出改变时才通知下游；`computed_always` 不比较输出，每次成功求值都通知。`watch` 的 getter 初始时总会求值，回调是否在初始阶段执行由 `WatchOptions::immediate` 决定；watch 回调本身使用 untracked 上下文。

## 调度阶段

所有 effect-like API 都要求显式传入 `EffectPhase`：
`EffectPhase::Normal` 用于普通响应式更新，`EffectPhase::PostFlush` 用于必须观察
本轮普通 DOM 更新之后状态的副作用。初始回调仍在注册期间同步执行；phase 只影响
依赖变化后的调度。

runtime 会先清空 Normal 队列，再执行 PostFlush 队列。PostFlush 回调写入 signal
时，新增的 Normal 工作会在下一个 PostFlush 回调前收敛。该阶段是 runtime 的同步
顺序保证，不依赖浏览器 microtask、timeout 或 animation frame；`computed` 仍然
是同步拓扑计算，不接收 effect phase。

## 最小可运行流程

下面示例创建一个临时作用域，建立 signal、computed 和 effect，再写入 signal 触发 effect。示例中的错误类型和错误分层都来自实际公开 API；不要把这些 `Result` 在业务代码中用 `unwrap` 隐藏掉。

{% set source = load_data(path="examples/silex_reactivity/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

示例源文件由 `crates/silex_reactivity/tests/docs_examples.rs` 直接编译并执行。它专门把计算初始化错误、计算读取错误和 transient scope 关闭错误分开处理，再在示例边界转换到 `Box<dyn Error>`。实际应用可以把这些错误映射到自己的错误枚举。

## 错误分层

响应式代码中最容易误判的是“创建成功”与“以后每次运行都成功”不是同一件事：

- 直接的节点读写返回 `ReactiveResult<T>`，例如节点失效、动态借用冲突或跨 runtime 追踪读取失败。
- `computed`、`effect` 和 `watch` 的创建返回 `ComputationInitResult<T, E>`。`Registration` 表示注册或运行时失败，`Initial` 表示初始执行返回了用户错误；初始失败的计算节点会被释放。
- `Computed`、`Callback` 和 completion 的回调执行结果使用 `CallbackInvokeError<E>` 或 `CompletionSubmitError<E>`，可以分别区分运行时错误、用户错误、handler 错误和 close 错误。
- owner 的显式关闭返回 `CloseError`。它可能包含多个清理阶段的失败；只要关闭失败是可重试的，owner 会保持活动状态，调用方应在释放动态借用后重试。

错误处理器也是作用域对象。通过 `OwnerAccess::error_handler` 注册的 token、view 和 handler lease 都不能脱离对应的 owner 生命周期；处理器分发失败应记录其 `HandlerReason` 和 `ErrorContext`，不要依赖内部节点编号拼接诊断。

## 适用边界

- 需要短生命周期的局部图时使用 `Runtime::with_transient`；回调结束后作用域自动关闭，句柄不能通过 HRTB 生命周期检查逃逸。
- 需要由框架或应用明确替换、关闭的子树时，创建 root 或持久子 owner，再通过 `OwnerHandle::access`/`with_access` 取得 `OwnerAccess`。
- 需要把外部任务结果回传到作用域时，使用 `completion_once` 或 `completion_sender`，并检查 `submit` 返回的 `bool`。owner 已关闭时，提交会被拒绝而不会调用失效回调。
- 需要跨 runtime 取得快照时，只使用 untracked 读取；跨 runtime 的普通追踪读取会返回 `ReactiveError::RuntimeMismatch`。
- `silex_reactivity` 是底层引擎。Silex 应用通常通过更高层 facade 或框架生命周期接入它，而不是自行管理内部 node id、指针或 scheduler。

## 专题

- [响应式图与计算](@/developer/crates/silex_reactivity/signals.md)：signal、computed、effect、watch、callback、非响应式节点和调度规则。
- [作用域与生命周期](@/developer/crates/silex_reactivity/lifecycle.md)：owner、transient scope、持久子作用域、cleanup、handler 和 completion。
- [错误处理与诊断](@/developer/crates/silex_reactivity/errors.md)：初始化错误、延迟回调错误、关闭聚合和恢复策略。
- [测试与调试](@/developer/crates/silex_reactivity/testing.md)：集成测试、编译期契约、`test-support` 快照和基准边界。

## 源码与测试索引

- 公开入口：`crates/silex_reactivity/src/lib.rs`
- owner 与公开节点：`crates/silex_reactivity/src/owner.rs`、`src/owner/node.rs`
- 运行时执行、依赖和队列：`crates/silex_reactivity/src/runtime/`
- 生命周期错误与关闭聚合：`crates/silex_reactivity/src/root/scope.rs`、`src/error.rs`
- 文档示例：`docs/examples/silex_reactivity/basic.rs`
- 作用域与清理测试：`crates/silex_reactivity/tests/runtime_scope.rs`、`tests/owned_scope.rs`
- 图谱与追踪测试：`crates/silex_reactivity/tests/graph.rs`、`tests/automatic_tracking.rs`
- completion 测试：`crates/silex_reactivity/tests/completion.rs`
- 编译期契约：`crates/silex_reactivity/tests/compile_fail.rs` 和 `tests/ui/`
- 基准入口：`crates/silex_reactivity/benches/reactivity.rs`

验证本页或公开 API 变更时，至少运行 `cargo check -p silex_reactivity`、`cargo test -p silex_reactivity` 和 `zola check`。涉及文档示例时，优先确认 `docs_examples` 测试仍然通过。
