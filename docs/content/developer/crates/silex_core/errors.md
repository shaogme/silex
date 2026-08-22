+++
title = "错误处理与 feature 边界"
description = "silex_core 的 SilexError、handler、关闭错误和可选领域错误。"
weight = 30
+++

# 错误处理与 feature 边界

`silex_core` 将错误的“类别”和“处理策略”分开保存。`SilexErrorKind` 说明错误来自响应式运行时、框架、JavaScript 或可选领域；`SilexError` 外层的 `Recoverable`/`Fatal` 说明调用方是否可以继续当前生命周期。绝大多数公开操作返回 `SilexResult<T> = Result<T, SilexError>`。

## SilexError 的两层结构

```text
SilexError
├── Recoverable(SilexErrorKind)
└── Fatal(SilexErrorKind)
    ├── Reactivity(ReactiveError)
    ├── Close(CloseError)
    ├── Framework(String)
    ├── Javascript(String)
    └── feature-gated domain / mount / bootstrap kinds
```

使用 `SilexError::recoverable(kind)` 或 `SilexError::fatal(kind)` 构造错误，使用 `severity`、`is_recoverable`、`is_fatal`、`kind` 和 `into_kind` 分析它：

```rust
match operation() {
    Ok(value) => use_value(value),
    Err(error) if error.is_recoverable() => report_and_continue(error),
    Err(error) => {
        record_fatal(error.kind());
        stop_current_scope();
    }
}
```

不要只把错误格式化成字符串再决定恢复策略；`SilexErrorKind::as_str()` 为宿主适配器提供稳定类别名，`source()` 还保留底层错误链。

## 运行时错误与用户错误

高层 facade 对底层运行时结构错误统一映射为 fatal `SilexError`，例如：

| 场景 | 结果 |
| --- | --- |
| stale node、owner 已关闭 | `Fatal(Reactivity(NoSuchNode))` |
| tracked 读取跨 runtime | `Fatal(Reactivity(RuntimeMismatch))` |
| 动态借用重叠 | `Fatal(Reactivity(BorrowConflict))` |
| 计算递归或图不收敛 | `Fatal(Reactivity(Reentrant/NonConvergent))` |
| owner 注册、节点创建或 handler 失效 | `Fatal(Reactivity(...))` |
| 用户闭包返回的 `SilexError` | 保留闭包返回的 recoverable/fatal 级别 |

这意味着计算 API 的 `Ok(Computed)` 只表示节点已注册且初始运行成功；未来重算或显式读取仍可能返回错误。计算、effect、watch、cleanup 和 completion 都要保留自己的错误路径。

## Error handler 的作用域

`OwnerAccess::error_handler` 注册 `Fn(SilexError) + 'owner`，返回 `ErrorHandlerToken<'owner>`。计算后续重算、异步 completion、cleanup 和 task 中无法直接由调用方同步返回的错误，都需要一个 `ErrorHandlerInput<'owner>`：

```rust
let token = owner.error_handler(|error| record(error))?;
let reporter = token.view();

let _effect = owner.effect(
    EffectPhase::Normal,
    move || {
        update_view()?
    },
    reporter,
)?;
```

公开可用的 handler 输入包括 token、`ErrorHandler`/`ErrorReporter` view 和 `ErrorHandlerAnchor`。它们都受 owner lifetime 约束；token/view 不能保存到 owner 关闭之后，也不能跨 runtime 复用。`ErrorHandlerInput` 本身标记为隐藏 trait，应用通常只需要传入上述类型。

`Callback::invoke` 会将底层 `CallbackInvokeError<SilexError>` 映射为 `SilexResult<()>`：runtime 错误变为 fatal Reactivity，用户返回的 `SilexError` 原样保留，handler 分发错误再包装为 fatal Reactivity。`Callback::call` 只是 legacy method spelling，返回类型和错误语义与 `invoke` 相同。

## Owner close 是另一条错误通道

节点操作使用 `SilexResult`，但 `OwnerHandle::close` 的签名是 `Result<(), CloseError>`。关闭可能聚合多个 cleanup、handler、completion 或 owner 阶段错误，因此不要在 close 边界把它提前压成一条 `SilexError`：

```rust
match root.close() {
    Ok(()) => {}
    Err(close_error) => record_close_diagnostics(close_error),
}
```

`Runtime::with_transient` 会把 transient scope 的 runtime/close 失败映射为 `SilexError`；如果 callback 本身返回 `SilexResult<T>`，调用方常见的是嵌套 `Result`，应显式传播两层，而不是忽略自动 close 的错误。`Runtime::take_unhandled_close_errors` 用于取出 Drop 或 panic recovery 路径无法同步返回的 close diagnostics。

`error-dom` feature 下，`CleanupReport`、`MountError`、`DisposeError`、`RollbackError` 和 `DropFailureReport` 保留了 cleanup failure 与 boundary error 的分组。`MountError::can_retry`/`is_poisoned` 的值来自 rollback report：干净 rollback 可重试，有遗留失败则标记 poisoned。框架应记录完整 report，不要只保留 primary error。

## JavaScript 与领域错误

`SilexErrorKind` 的基础变体是 `Dom(String)`、`Framework(String)`、`Javascript(String)`、`Reactivity(ReactiveError)` 和 `Close(CloseError)`。`JsValue` 转换为 `Javascript` kind：优先使用字符串值，否则使用 debug 表示。

可选错误类型遵循同一模式：每个 `XxxError` 都有 `Recoverable`/`Fatal`、`kind`、`severity` 和 `into_kind`，转换为 `SilexError` 时保留原 severity。已验证的 feature 与导出如下：

| Feature | 类型 | 典型类别 |
| --- | --- | --- |
| `error-persistence` | `PersistenceError` | backend unavailable、read/write/decode/encode、reactivity、core。 |
| `error-i18n` | `I18nError` | locale/catalog/message/loader、reactivity、core。 |
| `error-router` | `PathError`、`PathParamError`、`RoutePatternError` | percent encoding、UTF-8、参数和重复 pattern。 |
| `error-net` | `NetError` | timeout、transport、HTTP status、decode/serialize、connection state。 |
| `error-intl` | `IntlError` | invalid value、JavaScript、unsupported formatter。 |
| `error-dom` | mount/dispose/cleanup 类型 | 挂载、回滚、清理聚合和 drop 诊断。 |
| `error-bootstrap` | `AppHostError`、`BootstrapError` | host state、mount/unmount、target 和 listener；隐含启用 `error-dom`。 |

网络错误的 `is_retryable` 只对 timeout、transport unavailable 和特定 HTTP status 返回 true；这不是所有 `Recoverable` 错误都可重试的保证。领域错误要在其所属模块边界处理，core 只负责统一承载。

## 错误处理反模式

- 在文档或业务边界使用 `unwrap`/`expect` 隐藏节点、handler、fetch、cleanup 或 close 错误；
- 把 `Recoverable` 强制转换成 fatal 后再无条件销毁 owner；先判断业务是否能继续；
- 看到 `CompletionSubmitError` 只记录 callback 部分，丢失同时发生的 close error；
- 在 handler 中重新调用可能触发同一动态借用的 API，导致 `BorrowConflict` 或递归分发；
- 只匹配 `SilexErrorKind::Framework`，忽略 feature-gated domain 和 `ReactiveError`；
- 通过 `get_untracked` 规避 `RuntimeMismatch` 后，却误以为 target effect 已经订阅了 foreign source。

## 对应测试

- error severity、kind、source chain 和稳定类别名：`crates/silex_core/src/error.rs` 的单元测试；
- reporter 分发与 scope 捕获：`crates/silex_core/tests/error_reporter.rs`；
- borrow、stale node、runtime mismatch 和 handler 失效：`crates/silex_core/tests/reactivity_errors.rs`、`tests/runtime_compatibility.rs`；
- completion、close 与 callback 失败：`crates/silex_core/tests/async_completion.rs`；
- feature-gated 错误的完整编译组合：`cargo check -p silex_core --all-features`。
