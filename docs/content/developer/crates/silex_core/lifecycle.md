+++
title = "owner 生命周期与异步边界"
description = "silex_core 的 runtime、owner、cleanup、handler、task 和 completion 约束。"
weight = 20
+++

# owner 生命周期与异步边界

`silex_core` 把所有响应式节点和异步资源绑定到 `OwnerAccess<'owner>`。这个绑定同时由 Rust 生命周期和底层 runtime registry 维护：生命周期防止能力值逃逸，registry 在运行时拒绝 stale handle、动态借用冲突和已关闭节点。

本文的短代码片段用于展示 API 关系；它们省略了外层函数和部分错误传播，不是 `docs/examples/` 中的 CI 编译示例。完整的可运行流程请参考总览页的 `docs/examples/silex_core/basic.rs`。

## 三种 owner 形态

| 形态 | 创建方式 | 关闭方式 | 适用场景 |
| --- | --- | --- | --- |
| root owner | `runtime.owner()` | `OwnerHandle::close()` | 应用或框架持有明确的根生命周期。 |
| 持久 child | `owner.create_child()` 或 `root.create_child()` | 返回的 `OwnerHandle::close()` | 路由分支、页面或组件子树需要被替换时。 |
| transient scope | `runtime.with_transient(...)` 或 `owner.with_transient(...)` | 回调返回后自动关闭 | 同步计算、一次性局部节点和测试。 |

`Runtime::owner` 需要 `&mut self`，并且一个 runtime 同时只允许一个活动 root。`OwnerHandle` 只提供关闭权和借出 access 的能力；真正创建节点时使用 `access()` 或 `with_access()`：

```rust
let mut runtime = Runtime::new();
let root = runtime.owner()?;

root.with_access(|owner| {
    let signal = owner.signal(0_i32)?;
    let read = signal.read_signal();
    let write = signal.write_signal();
    write.set(1)?;
    assert_eq!(read.get()?, 1);
    Ok::<(), SilexError>(())
})?;

root.close()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

这里的 `root.close()` 不能省略。它可能返回底层 `CloseError`，与节点操作使用的 `SilexError` 不是同一种错误类型；调用方要保留关闭错误中的清理阶段和诊断信息。

## 借用与逃逸规则

`OwnerAccess<'owner>` 是 `Clone + Copy` 的借用能力，但节点句柄的 `'owner` 参数把它们绑定到同一个 scope。以下边界由类型系统保证：

- transient 回调可以返回普通数据，但不能返回捕获 child lifetime 的 `ReadSignal`、`Callback`、`TaskHandle` 或 `OwnerAccess`；
- `spawn_scoped` 接受的 future 必须是 `Future<Output = ()> + 'owner`，因此 future 可以借用 owner 数据，但不能比 owner 活得更久；
- error handler token/view、completion endpoint 和 `SilexContext` 都携带 scope lifetime，不能独立保存为全局状态；
- 跨 runtime 的句柄即使 Rust 生命周期没有问题，tracked 读取也会由 `RuntimeMismatch` 拒绝。

如果需要让异步工作继续存在，应创建持久 root/child，并使用 `OwnerHandle::with_access_async` 借出 access：

```rust
root.with_access_async(|owner| {
    Box::pin(async move {
        // owner 和它创建的句柄只在这个 future 内有效。
        let signal = owner.signal(1_u32)?;
        let source = signal.read_signal();
        let set_source = signal.write_signal();
        set_source.set(2)?;
        source.get()
    })
})
.await?;
```

实际调用需要在 `async` 函数中处理 `SilexResult`；`with_access_async` 本身不会替调用方关闭 root。

## cleanup 与关闭顺序

`OwnerAccess::on_cleanup` 注册一个 `FnOnce() -> SilexResult<()>`。cleanup 属于注册时的 owner，并由 owner 关闭路径执行。清理代码应满足以下不变量：

- 不要把异步工作遗留到 cleanup 返回之后；cleanup 返回前应取消任务、释放外部句柄或把结果交给仍活动的上层 owner；
- 子 owner 必须先于父 owner 清理，依赖子树的父 cleanup 不应假设子节点仍然活动；
- cleanup 返回的 `SilexError` 会进入关闭错误聚合，不能用 `unwrap`/`expect` 把失败变成 panic；
- 关闭失败时，调用方应先释放动态借用，再根据 `CloseError` 的可重试语义重试；不要继续使用已经报告为 inactive 的节点。

`StoredValue` 有一个窄的清理例外：最终 owner cleanup 正在释放其 payload 的窗口内，`StoredValue::with`/`update` 仍可访问它；普通 signal、callback、node ref、effect 和节点创建 API 在 owner 标记 inactive 后仍不可用。这个例外只用于同步释放资源，不能把 handle 交给异步代码。

需要把一棵带 parent lifetime 的子树交给 owner root 管理时，使用
`OwnerChild<'owner>` 和 `on_owner_cleanup`：

```rust
let child = owner.create_owned_child()?;
let child_owner = child.access();
// 在 child_owner 中初始化 owner-bound 能力。

if let Err(error) = owner.on_owner_cleanup(
    child,
    |child| child
        .close()
        .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error))),
    error_handler,
) {
    let (registration_error, child) = error.into_parts();
    let _ = child.close();
    return Err(registration_error);
}
```

`on_owner_cleanup` 不会把 cleanup 挂到当前 effect；它永远使用 parent owner root。
注册前的任何失败都会返回 `OwnerCleanupRegistrationError<'owner, T>` 及原始
payload，调用方必须关闭或释放该 payload。`on_cleanup` 仍然表示当前 computation
cleanup。`OwnerChild::close()` 是显式、幂等、可重试的关闭操作，父 owner close
是 Resource、DOM branch 等 owner-bound 资源的最终取消边界。

`Resource` 的复制只复制已经创建的 reactive node capability，不创建 child、不增加
引用计数，也不承担关闭权。丢弃一个或全部 `Resource` 变量不会取消请求；资源创建
成功后，父 owner cleanup 持有 child scope 的最终关闭权。需要比父 owner 更早取消时，
应创建独立 child owner，将资源绑定到该 child，再显式关闭 child。

## Error handler 与 context

需要处理延迟错误的 API 都接受 `ErrorHandlerInput<'owner>`，常见输入是：

- `ErrorHandlerToken<'owner>`：`owner.error_handler(...)` 返回的 RAII 注册 token；
- `ErrorHandler<'owner>` / `ErrorReporter<'owner>`：不拥有注册本身的可复制 view；
- `ErrorHandlerAnchor<'owner>`：框架生命周期需要长期保留的 owning view。

组件可以把 owner 与 reporter 打包为 `SilexContext`：

```rust
let reporter = owner.error_handler(|error| report_to_host(error))?;
let ctx = SilexContext::new(owner, reporter.view());
let child_ctx = ctx.with_error_reporter(other_reporter.view());
```

`SilexContextProvider` 只要求实现者提供这两个能力和替换 reporter 的方法。`rx!` 宏通过这个契约取得 owner 与错误目的地，并返回 `SilexResult`；调用方可以用 `?`、`match` 或其它方式处理宏创建阶段的错误。

## `spawn_scoped` 与取消

`OwnerAccess::spawn_scoped` 在本地执行器上启动 `Future<Output = ()>`，并把取消 cleanup 注册到当前 owner。它返回 `TaskHandle`：

```rust
let task = owner.spawn_scoped(
    async move {
        run_local_work().await;
    },
    reporter,
)?;

if should_cancel {
    task.cancel();
}
assert!(task.is_cancelled());
```

`TaskHandle` 的 `cancel` 是幂等的，并会同步取出并释放尚未完成的 future。丢弃 handle 不会脱离 owner；owner cleanup 仍然负责取消任务。future 内的业务错误不能通过 `Future::Output` 返回，因为输出固定为 `()`；应在 future 内调用 reporter，或把错误转换成可处理的状态。

`src/task.rs` 使用一次 `unsafe transmute` 把 driver 中暂时保存的 scoped future 擦除成 `'static`，再交给 `spawn_local`。安全前提是 owner cleanup 在 scope 捕获的数据释放前同步清空 future；这不是可由调用方绕过的通用 `'static` 能力。任何修改 task driver、cleanup 顺序或 owner close 逻辑的变更都必须复核这一不变量。

## completion endpoint

`OwnerAccess::completion_once` 与 `completion_sender` 把外部 future 的结果交回 owner。`CompletionOnce<T>` 适合单次终点，`CompletionSender<T>` 适合多个任务共享的终点；二者的回调都要求 `UnwindSafe`，提交返回值必须处理：

- callback 自身失败时，错误仍是用户的 `SilexError`；
- owner 已关闭或 endpoint 已关闭时，结果不会调用失效 callback；
- callback 错误与 close 错误可以同时出现，`CompletionSubmitError` 保留两部分，不能只取一个字符串；
- callback panic 后，运行时仍需恢复 endpoint 的终态，业务代码不应依赖 panic 传递错误。

框架内部还可使用 detached completion，把 callback 节点从当前 effect 的 ownership
子树移出，使 effect rerun/stop 不会提前关闭宿主 callback。它仍绑定创建它的 owner，
因此 owner close 或显式 cancel 仍会关闭 endpoint。runtime 已进入 disposal 时，
endpoint close 会登记到 pending 队列，由外层事务统一 drain 和去重；这保证嵌套
cleanup 不会递归执行同一 endpoint 的 disposal，相关失败仍保留在 close report 中。

`Resource` 和 `Mutation` 使用该机制，并额外用 request id 抑制过期结果；详细状态机见[异步状态专题](async.md)。

## 并发与 runtime provenance

同一个 `Runtime` 的不同 owner 可以互相读取 tracked source，前提是源节点和 observer 都仍活动。不同 `Runtime` 之间：

- `get`/`with` 会建立依赖，因此返回 `ReactiveError::RuntimeMismatch`；
- `get_untracked`/`with_untracked` 只读取目标值，不建立边，但仍会检查节点有效性和动态借用；
- `OwnerAccess::validate_runtime` 可在创建 target-side 节点前主动验证 source 的 owner 和 runtime。

不要通过复制句柄、把 runtime 放入 `Rc<RefCell<_>>` 或把数据发送到另一个线程来规避这些边界；这种做法只会把生命周期错误推迟到运行时。

## 对应测试

- root、transient、child 和关闭：`crates/silex_core/tests/root_scope.rs`、`tests/runtime_compatibility.rs`
- stale handle、cleanup 和借用冲突：`crates/silex_core/tests/reactivity_errors.rs`
- handler view 与作用域捕获：`crates/silex_core/tests/error_reporter.rs`
- task、resource、mutation 和 future drop：`crates/silex_core/tests/async_completion.rs`
- lifetime、handler escape、`Send` 和旧 API：`crates/silex_core/tests/compile_fail.rs`、`tests/ui/`
