+++
title = "Resource、Mutation 与异步状态"
description = "silex_core 的异步资源、异步变更、scoped task 和请求替换语义。"
weight = 25
+++

# Resource、Mutation 与异步状态

`Resource` 和 `Mutation` 把 owner 绑定的本地 future 转换成响应式状态。它们不提供跨线程执行器，也不把 future 直接暴露给组件；调用方观察 `ReadSignal`，通过方法触发 fetch/mutate，并让 owner cleanup 负责取消未完成工作。

## Resource 状态机

`Resource<'owner, T, E>` 的 `state` 类型是 `ReadSignal<ResourceState<T, E>>`：

| 状态 | 含义 |
| --- | --- |
| `Idle` | 尚未开始加载。初始化 effect 通常会立即触发第一次加载，因此只在构造边界短暂可见。 |
| `Loading` | 当前没有旧数据，正在等待第一次结果。 |
| `Ready(T)` | 当前请求成功并有数据。 |
| `Reloading(T)` | 请求重新开始，但继续保留上一次成功数据。 |
| `Error(E)` | 当前请求失败；没有旧数据可供 `as_option` 返回。 |

`ResourceState::as_option` 对 `Ready` 和 `Reloading` 返回数据，`into_value` 只接受这两种状态，否则返回 fatal framework error；`is_loading` 对 `Loading` 和 `Reloading` 返回 true。

## 创建与触发 fetch

```rust
let resource = Resource::builder(owner)
    .source(source)
    .fetch(|key| async move { fetch_data(key).await })
    .suspense(suspense)
    .build(error_handler)?;

resource.refetch()?;
let loading = resource.loading()?;
let value = resource.value()?;
resource.update(|data| data.refresh_local_cache())?;
resource.set(local_value)?;
```

`source` 必须同时实现 `RxRead<Value = S>`、`ReactiveSource`、`Clone`，且 source 读取会成为内部 effect 的依赖。source 初始求值和每次变化都会启动 fetch；`refetch` 通过内部 trigger 重新执行 effect，即使 source 值没有变化。

`ResourceFetcher` 是 fetcher 抽象：可以直接传入 `Fn(S) -> Future<Output = Result<T, E>>`，也可以实现 trait 以复用更复杂的 fetcher。`T` 必须可 clone，`E` 必须 `Clone + Debug`，因为状态既要放入 signal，也要通过 `SilexError` handler 边界诊断。

## 请求替换与取消

每个请求分配递增 request id。结果回到 owner 后，只有 id 仍等于当前 id 才能提交 `Ready(T)` 或 `Error(E)`；过期结果被丢弃，不会覆盖新请求状态。

请求 future 还挂在资源 child 的 effect run/owner cleanup 下：source 变化时，旧 run
的 cleanup 会结束旧请求；owner 关闭时，所有未完成 future 都会被同步释放。
`Resource` 本身是 `Copy + Clone` 能力句柄，复制或丢弃句柄不会改变 future 生命周期；
创建资源的 owner close 才是资源的最终取消边界。需要提前取消时，应把资源绑定到
独立 child owner 并关闭该 owner。`id` 校验是状态一致性保护，cleanup 是资源释放
机制，两者不能互相替代。

Resource builder 使用通用的 `OwnerChild` 事务：child 初始化成功后，父 owner 通过
`on_owner_cleanup` 持有 child close payload；初始化或 registration 失败时，错误的
`into_parts()` 会恢复 child，调用方立即执行 rollback。该 registration 不依赖当前
Resource effect 的 cleanup，也不通过 `untrack` 改变 cleanup 归属。

如果传入 `SuspenseContext`，每个未结算请求会 `increment`，成功、失败、替换 cleanup 或 owner close 都会保证对应的 `decrement`。计数是饱和递减，不会因为重复 settle 变成负数；`ResourceCompletion` 的 `settled` 标记防止 callback 与 cleanup 重复减少计数。

## Resource 的读接口

- `value` 与 `get_data` 返回 `SilexResult<Option<T>>`，在 `Ready`/`Reloading` 中 clone 当前数据；
- `loading` 返回当前是否处于加载中；
- `update` 只修改 `Ready`/`Reloading` 中已有的数据，`Idle`/`Loading`/`Error` 下不执行闭包；
- `map(owner, f, handler)` 把 `Option<&T>` 转为新的 always-notifying `Rx`；
- 作为 `ReactiveSource` 使用时，Resource 暴露的是 `Option<T>`，并在目标 owner 中创建 derived computed。

## Mutation 状态机

`Mutation<'owner, Arg, T, E>` 的 `state` 类型是 `ReadSignal<MutationState<T, E>>`：

| 状态 | 含义 |
| --- | --- |
| `Idle` | 没有正在处理的变更。 |
| `Pending` | 最近一次 mutate 已发布，future 尚未完成。 |
| `Success(T)` | 最近一次有效请求成功。 |
| `Error(E)` | 最近一次有效请求返回业务错误，或 prepare 阶段失败。 |

`MutationState::value` 只对 `Success` 返回数据，`is_loading` 只对 `Pending` 返回 true。

创建普通 mutation：

```rust
let mutation = Mutation::new(
    owner,
    |argument| async move { save_data(argument).await },
    error_handler,
)?;

mutation.mutate(argument)?;
mutation.mutate_with(source)?;
let loading = mutation.loading()?;
let value = mutation.value()?;
let error = mutation.error()?;
```

`new` 会先发布 `Pending`，再调用 action 创建 future。需要在发布 `Pending` 之前完成同步准备时使用 `new_with_prepare`：

```rust
let mutation = Mutation::new_with_prepare(
    owner,
    |argument| prepare_save(argument),
    error_handler,
)?;
```

prepare 返回 `Err(E)` 时不会启动 future，而是直接发布 `Error(E)`，并递增 request id 使之前请求的迟到 completion 失效。无论是 action 返回业务错误，还是 prepare 返回错误，都保留在 `MutationState::Error`；运行时错误和 completion/close 错误则走 `SilexError` handler。

## 过期 mutation 结果

Mutation 与 Resource 一样使用 request id。连续调用 `mutate` 时，只有最近一次 id 的 completion 能够更新状态；旧 future 即使稍后完成也只会被提交 endpoint 丢弃。这个规则避免响应顺序反转覆盖新状态，但不保证服务端操作本身被撤销；需要取消本地 future 或服务端请求时，必须让 action/fetcher 提供对应能力。

`Mutation` 对 owner 关闭是惰性的：如果 owner 已 inactive，后续 `mutate` 返回 `Ok(())` 而不启动任务；已经启动的 future 由 owner cleanup 取消。调用方不能把 `Mutation` 句柄带到 owner 之外异步使用。

## TaskHandle 与 completion 错误

资源和变更最终通过 `OwnerAccess::spawn_scoped` 运行 future，再通过 `CompletionSender` 将结果送回 owner。若 `submit` 同时产生 callback 错误和 close 错误，`report_completion_error` 会分别交给 handler；不能只记录 callback 那一部分。

普通任务的 `TaskHandle::cancel` 会立即释放尚未完成的 future，重复调用没有副作用。任务 handle 被丢弃时不会自动 detach；owner close 仍是最终取消边界。

## 失败路径清单

- fetcher/action 的 `E` 是业务状态，应从 `ResourceState::Error` 或 `MutationState::Error` 读取；
- signal、computed、owner、completion 和 cleanup 的失败是 `SilexError`，应传播或交给正确的 `ErrorReporter`；
- owner 在 future 完成前关闭时，completion 不应调用已释放的 callback；
- 旧结果被丢弃不代表 future 已释放，必须检查 owner/effect cleanup 和 future 的 `Drop` 行为；
- 不要在 effect 内无条件写回它自己读取的 source，否则可能触发 `NonConvergent`。

## 对应测试

异步语义集中在 `crates/silex_core/tests/async_completion.rs`，其中覆盖：

- Resource 的 `Loading`、`Reloading`、suspense 计数和 source 替换；
- Resource/Mutation future 在 scope dispose 后的 drop；
- Mutation prepare 错误使前一 completion 失效；
- 持久 owner 中异步更新仍使用同一 scope capability；
- `TaskHandle` 的 cancel、重复 cancel 和 future 释放；
- completion callback 错误、close 错误和 panic 恢复边界。

该测试文件只在 `wasm32` 下启用 `wasm-bindgen-test`；native 测试仍覆盖请求 id 的纯函数状态转换。
