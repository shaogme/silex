+++
title = "错误处理与诊断"
description = "silex_reactivity 的运行时错误、用户回调错误、handler 和关闭诊断。"
weight = 30
+++

# 错误处理与诊断

`silex_reactivity` 将“运行时无法继续操作”和“用户回调返回了业务错误”分成两条
错误通道。前者使用 `ReactiveError`，后者保留用户定义的 `E`；关闭作用域时，多个
阶段的失败再聚合到 `CloseError`。这样调用方可以决定哪些错误需要重试、哪些错误
需要停止节点，以及哪些错误属于框架或应用自身的 bug。

## 先区分错误发生阶段

| 阶段 | 典型 API | 返回类型 | 处理重点 |
| --- | --- | --- | --- |
| 节点或作用域操作 | signal 读写、创建节点、`stop` | `ReactiveResult<T>` | 处理句柄失效、动态借用、runtime 不匹配 |
| 计算注册 | `computed`、`effect`、`watch`、`effect_with_previous` | `ComputationInitResult<T, E>` | 分开处理注册失败和第一次执行的用户错误 |
| 可失败计算读取 | `Computed::get`、`with` | `CallbackInvokeResult<T, E>` | 区分运行时错误、用户错误和 handler 错误 |
| 类型化 callback | `Callback::invoke` / `dispatch` | `CallbackInvokeResult` / `Result<(), HandlerError>` | `invoke` 返回用户错误，`dispatch` 将其交给 handler |
| completion 提交 | `CompletionOnce::submit` | `CompletionSubmitResult<E>` | 同时检查 callback 错误和 endpoint close 错误 |
| completion 取消 | `CompletionOnce::cancel`、`CompletionSender::cancel` | `Result<(), CloseError>` | 处理 endpoint 关闭阶段的聚合错误 |
| repeating completion 提交 | `CompletionSender::submit` | `CompletionSubmitResult<E>` | 常规返回主要是 callback 错误；关闭错误由 `cancel` 或 Drop 路径报告 |
| owner 关闭 | `OwnerHandle::close` | `Result<(), CloseError>` | 保留聚合条目，必要时在释放借用后重试 |

不要用一个统一的字符串错误替换这些类型。错误变体携带了恢复所需的状态，尤其
是 `CompletionSubmitError::CallbackAndClose` 不能被简化为“提交失败”。

## `ReactiveError`：运行时结构错误

公开的 `ReactiveError` 变体及常见含义如下：

- `NoSuchNode`：节点或 owner 所属作用域已经结束，或节点句柄的身份/代数不再匹配。handler 的代数不匹配则通过 `HandlerError::reason() == HandlerReason::GenerationMismatch` 报告。
- `WrongKind`：句柄指向的节点种类与操作不匹配；这通常说明调用方保存了错误的
  capability，是应优先定位的实现错误。
- `BorrowConflict`：同一节点、scope 或 scheduler 上已有互斥的动态借用。释放外层
  `with`/`update` 借用后，操作通常可以重试。
- `Reentrant`：计算正在运行时又递归读取同一个运行中的计算，或 scope 正在不允许
  重入的关闭阶段。类型化 callback 的递归调用通常表现为 `BorrowConflict`；应改写
  依赖图或把递归状态移到普通 Rust 数据结构中。
- `RuntimeAlreadyRunning`：同一个 `Runtime` 已有活动的 root owner；root 成功释放
  后才能创建下一个 root。
- `RuntimeMismatch`：tracked 读取或依赖边跨越了不同 scheduler family。跨 runtime
  的只读快照使用 `get_untracked` 或 `with_untracked`。
- `InvariantViolation`：运行时内部不变量被破坏，通常应记录完整上下文并停止继续
  操作，而不是重试同一个调用。
- `Handler(HandlerError)`：用户错误无法交付给注册的 handler，应同时检查嵌套借用、
  handler 是否退休以及 scope 是否已释放。
- `DuplicateTarget`：同一个事务在发布前重复登记了同一个 signal。事务会进入
  poisoned 状态，不应继续提交该事务。
- `NonConvergent { iterations, last_scope, last_node, last_phase }`：effect 队列在
  内部预算内没有收敛；`last_phase` 可指出最后处理的是 `Normal` 还是 `PostFlush`。
  应检查 effect 是否形成反馈写入环；该错误不是网络或临时借用错误。

`ReactiveError::is_bug()` 将 `WrongKind`、`Reentrant` 和 `InvariantViolation` 标记
为更可能是实现缺陷的类别，但这只是诊断提示，不是安全边界。所有错误仍应在应用
边界被记录或映射到上层错误枚举。

## 计算的初始化错误与延迟错误

计算 API 注册节点后会立即运行一次闭包。初始化阶段有两种失败：

```rust
let created = scope.computed(
    move || source.get().map(|value| value * 2),
    handler.view(),
);

match created {
    Ok(computed) => {
        let value = computed.get();
        // 读取时仍需处理 CallbackInvokeError。
        record_read_result(value);
    }
    Err(ComputationInitError::Initial(error)) => {
        // 初始闭包返回 E；刚注册的计算及其 cleanup 已被释放。
        record_user_error(error);
    }
    Err(ComputationInitError::Registration(error)) => {
        // handler、scope、动态借用或其他 runtime 错误。
        record_runtime_error(error);
    }
}
```

上面的片段省略了 `handler`、`record_*` 和外层错误类型，只用于展示分支关系，
不是 `docs/examples/` 中的可编译示例。

初始化成功后，后续重算的 `Err(E)` 不会提交新的值和新的依赖边；运行时会将用户
错误送到计算创建时保留的 handler lease。若 handler 成功接收错误，触发重算的操作
通常可以继续返回 `Ok(())`；若 handler dispatch 本身失败，则错误会以
`ReactiveError::Handler` 传播到当前操作。下一次源变化或显式读取仍可能再次尝试
该计算，因此 handler 不能被当作“永久吞错”开关。

读取 `Computed` 时，返回值不是 `ReactiveResult<T>`，而是
`CallbackInvokeResult<T, E>`：

```rust
match computed.get() {
    Ok(value) => use_value(value),
    Err(CallbackInvokeError::User(error)) => record_user_error(error),
    Err(CallbackInvokeError::Runtime(error)) => record_runtime_error(error),
    Err(CallbackInvokeError::Handler(error)) => record_handler_error(error),
}
```

signal、stored value 和 node ref 的直接操作仍使用 `ReactiveResult`；不要因为
computed 的用户错误类型与 signal 的 runtime 错误类型相似，就把两者混用。

## Handler 的退休和诊断上下文

`OwnerAccess::error_handler` 返回拥有注册记录的 `ErrorHandlerToken`。计算、cleanup
和 completion 通过 `ErrorHandlerInput` 获取短期 lease；`ErrorHandlerRef` 只是不拥有
注册记录的分发视图。应用代码通常保存 token，向 API 传入 token 或 `token.view()`，
并让 token 与 owner 同时结束：

```rust
let handler = scope.error_handler(|error: MyError| {
    record_user_error(error);
})?;

let effect = scope.effect(
    EffectPhase::Normal,
    move || do_work().map_err(MyError::from),
    &handler,
)?;
```

当 token 被显式关闭或最后一个强引用释放时，handler 进入 closing/retired 流程。
已有计算的 lease 可能仍然有效，但新的分发可能返回 `HandlerError`。诊断时使用
`HandlerError::reason()` 和 `HandlerError::context()`；context 包含 phase，并可能
包含 owner、node kind 和 node id。node id 只用于关联同一次运行中的日志，不应被当成
跨关闭或跨 runtime 稳定的业务标识。

`HandlerReason` 的公开分类包括 `BorrowConflict`、`NoSuchNode`、`Inactive`、
`GenerationMismatch`、`ScopeReleased` 和 `Internal`。其中前两类通常应结合当前
借用或节点状态处理，后四类主要用于识别 handler 生命周期、注册代数或内部状态问题。

## Completion 的双重结果

`CompletionOnce::submit` 既会调用用户 callback，也会关闭一次性 endpoint，因此可能
同时得到 callback 和 close 错误。推荐拆解结构化结果：

```rust
match completion.submit(value) {
    Ok(true) => record_accepted(),
    Ok(false) => record_stale_completion(),
    Err(error) => {
        let (callback, close) = error.into_parts();
        if let Some(callback) = callback {
            record_callback_error(callback);
        }
        if let Some(close) = close {
            record_close_error(close);
        }
    }
}
```

`Ok(false)` 表示 endpoint 已取消、owner 已关闭或一次性 endpoint 已提交过；它不是
用户 callback 的失败。`CompletionSender` 可多次提交，但 callback 返回用户错误时
sender 不会自动结束，调用方应根据业务决定重试或显式 `cancel`。callback panic 会
先尝试关闭 endpoint，再继续传播 panic；之后的提交会被拒绝。

框架使用的 detached completion 与普通 completion 共享上述提交和错误模型，只是
callback 节点不挂在创建它的当前计算子树中。runtime 正在 disposal transaction
内时，endpoint close 会进入 pending 队列，由外层统一 drain；重复登记会按
`(owner_id, node_id)` 去重，drain 的 runtime/handler/panic 失败仍会并入关闭聚合，
不会通过递归 disposal 丢失。

## `CloseError`：关闭阶段的聚合诊断

显式 `OwnerHandle::close` 不会只返回第一条失败。使用 `entries()` 读取每个条目的
`ClosePhase`、`CloseSource` 和 `CleanupFailure`，或使用 `diagnostic()` 获取稳定的
cleanup panic 诊断：

```rust
if let Err(error) = owner.close() {
    for entry in error.entries() {
        record_close_entry(entry.phase(), entry.source(), entry.failure());
    }
    record_close_diagnostic(error.diagnostic());
}
```

`CleanupFailure` 有四类：`Runtime(ReactiveError)`、`Transaction(TransactionError)`、
`Handler(HandlerError)` 和 `Panic(CleanupDiagnostic)`。关闭流程会继续尝试其他 child、
节点和 cleanup；如果
失败属于可重试的动态借用冲突，owner 保持可重试状态，释放借用后重新调用
`close`。已经进入 released 状态的 owner 再次关闭是幂等成功。

`OwnerHandle::drop` 无法返回 `CloseError`，因此会把无法向上传递的诊断放入
`Runtime::take_unhandled_close_errors()`。长生命周期宿主应在生命周期边界定期取出
这些错误并记录；不要因为没有显式 `close` 就假定清理一定成功。

## 错误映射建议

在框架边界可以把 crate 错误映射到应用错误，但建议保留以下信息：

- 对 `BorrowConflict`、`RuntimeMismatch` 和 `NoSuchNode`，记录操作阶段以及 owner
  生命周期状态；不要无限重试 `NoSuchNode`。
- 对 `ComputationInitError::Initial(E)`，将用户错误映射到组件初始化失败；对
  `Registration(ReactiveError)`，保留 runtime 错误类别。
- 对 `HandlerError`，记录 `reason` 和 `context`，并检查是否在 cleanup 或 close
  阶段发生。
- 对 `TransactionError`，保留 `phase`、`primary()` 和 `rollback_failures()`；
  rollback 失败可能已经另外进入 runtime 的关闭诊断队列。
- 对 `CloseError`，保存所有 `entries()`；只保留 `diagnostic()` 可能丢失阶段和
  handler 信息。

相关实现和回归测试：`crates/silex_reactivity/src/error.rs`、
`src/root/scope.rs`、`tests/fallible_derived.rs`、`tests/fallible_memo.rs`、
`tests/error_handler.rs`、`tests/completion.rs` 和 `tests/panic_reentry.rs`。
