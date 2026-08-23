+++
title = "响应式图与计算"
description = "silex_reactivity 的 signal、computed、effect、watch 与依赖追踪。"
weight = 10
+++

# 响应式图与计算

`silex_reactivity` 把 signal、computed、effect、watch、callback 等对象注册为 owner 作用域内的节点。计算闭包执行期间，追踪读取会记录依赖；源节点变化后，scheduler 将受影响的计算放入队列。计算成功后才会提交新的值和依赖边，计算失败则保留旧图谱并进入错误处理路径。

下面的 API 片段省略了外层函数、错误传播和 handler 注册等样板代码，只用于说明调用关系。示例中的 `handler::<E>(scope)` 是文档占位写法，表示已经通过 `scope.error_handler(...)` 注册的 `ErrorHandlerToken`；实际代码需要保留注册结果，并处理计算创建返回的 `ComputationInitResult`。需要可编译的完整流程时，请参考总览页引用的 `docs/examples/silex_reactivity/basic.rs`。

## Signal：可变源

通过 `OwnerAccess::signal` 创建的 `Signal` 同时持有同一个节点的读写能力，
可以通过 `read_signal()` 和 `write_signal()` 拆分：

```rust
let signal = scope.signal(0_i32)?;
let read = signal.read_signal();
let write = signal.write_signal();

let current = *read.read()?;
let snapshot = *read.read_untracked()?;
let copied = read.with(|value| *value)?;
let copied_untracked = read.with_untracked(|value| *value)?;

{
    let mut guard = write.write()?;
    *guard = 1;
    guard.commit()?;
}
write.set(1)?;
write.update(|value| *value += 1)?;
let changed = write.set_if_changed(3)?;
write.notify()?;
```

`get` 和 `with` 会在当前 scheduler 的 observer 上建立依赖；`get_untracked` 和 `with_untracked` 只读取值，不建立订阅。`get` 需要 `T: Clone`，而 `with` 允许在借用期间只提取需要的结果。

`read()` 返回的 `ReadGuard` 可以显式调用 `finish()` 提前释放借用并立即报告待处理
的 scheduler 错误；不调用时，guard 在 drop 时也会释放借用。`write()` 返回的
`WriteGuard` 默认在 drop 时提交修改，也可以显式调用 `commit()` 发布，或调用
`abort()` 释放借用而不发布 signal 通知。`abort()` 不会回滚已经通过 `DerefMut`
写入的 payload；如果需要回滚，调用方必须在普通 Rust 数据结构中自行保留并恢复旧值。

`set` 和 `update` 总会把本次写入视为变化并通知订阅者。`set_if_changed` 需要 `T: PartialEq`，相等时返回 `Ok(false)` 且不通知，不相等时返回 `Ok(true)`。如果通过 `Cell`、`RefCell` 等内部可变容器绕过了 signal 的写方法，需要在内部修改完成后调用 `notify`，否则响应式图不知道值已经改变：

```rust
let signal = scope.signal(std::cell::Cell::new(0_i32))?;
let read = signal.read_signal();
let write = signal.write_signal();
read.with(|value| value.set(1))?;
write.notify()?;
```

`OwnerAccess::signal` 返回 `Signal`，适合需要把读写能力作为一个值传递的场景；它提供对应的 `get`、`get_untracked`、`set`、`update`、`set_if_changed`，也可以用 `read_signal()` 和 `write_signal()` 拆分。需要把句柄显式拆成 pair 时，可以调用 `into_pair()`；从已有读写句柄组合时使用 `Signal::from_pair()`，它会拒绝属于不同 signal 节点的 pair。

## Computed：缓存的派生值

`OwnerAccess::computed` 创建一个需要 `PartialEq` 输出的 computed。它首次创建时会立即执行，读取的 signal 成为依赖；后续上游变化会使它重新求值：

```rust
let total = scope.computed(
    move || {
        let left = left.get()?;
        let right = right.get()?;
        Ok::<_, ReactiveError>(left + right)
    },
    handler::<ReactiveError>(scope),
)?;

let value = total.get()?;
let borrowed = total.with(|value| *value)?;
```

当新输出与旧输出相等时，`computed` 不会因为该输出变化而额外通知下游；如果输出不可比较，或每次成功求值都必须传播，则使用 `computed_always`。两者都提供 `get`、`with`、`get_untracked` 和 `with_untracked`。

计算的错误分两个阶段：

- 创建时的用户错误返回 `ComputationInitError::Initial(E)`，临时计算节点及其初始清理会被释放；注册或运行时失败则是 `ComputationInitError::Registration(ReactiveError)`。
- 创建成功后的重新计算如果返回 `E`，错误会交给注册的 error handler；显式读取通常通过 `CallbackInvokeError::User(E)` 返回。失败的求值不会提交新的值或新的依赖边，后续读取或源变化可以再次尝试。

因此不能只判断 `computed(...)` 返回 `Ok` 就认为所有未来读取都不会失败；读取也必须处理 `CallbackInvokeError`。

## Effect：副作用

`OwnerAccess::effect` 注册一个 `FnMut() -> Result<(), E>`。创建时先执行一次以建立初始依赖，随后在这些依赖发生变化时重新执行：

```rust
let effect = scope.effect(
    EffectPhase::Normal,
    move || {
        let value = source.get()?;
        record(value);
        Ok::<(), ReactiveError>(())
    },
    handler::<ReactiveError>(scope),
)?;

let stopped = effect.stop()?;
```

`EffectHandle::stop` 返回一个 `bool`：活动 effect 被停止时为 `true`，已经停止或所属作用域已经失效时为 `false`。停止会清理该 effect 的子节点和 cleanup，且不会重新激活它。effect 的初始用户错误是 `ComputationInitError::Initial(E)`；后续运行错误会交给 handler。

如果每次运行还需要拿到上一次成功返回的值，使用 `effect_with_previous`，其闭包签名是 `FnMut(Option<&T>) -> Result<T, E>`；第一次运行收到 `None`。

`effect_detached` 是标记为 `#[doc(hidden)]` 的框架生命周期入口。普通应用代码应使用 `effect`，因为普通 effect 在创建期间如果处于另一个计算中，会成为当前计算的子节点；detached effect 则明确挂在 owner 根下，由框架负责管理。

### `effect_with_previous`：保留上一次成功值

需要在每次 effect 运行中比较或释放上一次成功结果时，使用
`OwnerAccess::effect_with_previous`。闭包接收 `Option<&T>`，第一次运行是
`None`，之后只会看到上一次成功提交的值：

```rust
let effect = scope.effect_with_previous(
    EffectPhase::Normal,
    |previous: Option<&String>| {
        let next = source.get()?.to_string();
        if let Some(previous) = previous {
            println!("{previous} -> {next}");
        }
        Ok::<_, ReactiveError>(next)
    },
    handler::<ReactiveError>(scope)?,
)?;
```

`T` 不是借给下一次运行的临时引用；运行时只在回调执行期间借出上一次值，并在
`Ok(T)` 后提交新的值。回调返回错误或 panic 时，新的值不会提交，下一次运行仍
会从上一次成功值开始。与普通 effect 一样，读取的 signal 会成为依赖，返回的
`EffectHandle` 可以用 `stop` 结束它。

## Watch：只在 getter 结果变化时回调

`watch_getter` 把依赖读取和副作用回调分离。getter 返回 `T`，要求 `T: PartialEq`；只有 getter 的结果变化时，回调才会收到当前值和上一次值：

```rust
let watcher = scope.watch_getter_with_options(
    EffectPhase::Normal,
    move || source.get(),
    move |current, previous| {
        println!("{previous:?} -> {current}");
        Ok::<(), ReactiveError>(())
    },
    handler::<ReactiveError>(scope),
    WatchOptions::default(),
)?;
```

`WatchOptions` 有两个独立开关：

- `immediate()` 让初始化阶段也调用一次回调；没有该选项时，getter 仍会初始化并建立依赖，但回调不会在第一次求值时执行。
- `once()` 让 watcher 在第一次实际调用回调后停止。`immediate().once()` 会在初始化回调后立即停止；单独使用 `once()` 则会等待第一次变化。

watcher 返回 `EffectHandle`，可以显式调用 `stop`。getter 的依赖在每次成功运行后替换，回调在 untracked 上下文中执行，所以回调内部读取的 signal 不会成为 watcher 的依赖。getter 或回调返回的用户错误分别走初始化错误、后续 handler/error result 路径，不能把回调中的读取当作 getter 结果变化。

`watch` 是 `watch_getter_with_options` 的同义入口，适合已经拥有统一
`WatchOptions` 的框架代码；普通应用也可以使用更能表达意图的
`watch_getter` 或 `watch_getter_with_options`。watcher 的 getter 结果必须实现
`PartialEq`，而回调参数是 `(&T, Option<&T>)`：初始回调的旧值为 `None`，变化回调
的旧值是上一次成功 getter 结果。

## Callback 与非响应式节点

### `Callback`

`OwnerAccess::callback` 创建一个由 owner 持有的类型化回调：

```rust
let callback = scope.callback(|value: i32| {
    println!("{value}");
    Ok::<(), ReactiveError>(())
})?;

callback.invoke(1)?;
```

`Callback::invoke` 返回 `CallbackInvokeResult`，其中 `CallbackInvokeError::User(E)` 是用户回调返回的错误，`Runtime` 是节点或动态借用错误；`CallbackInvokeError::Handler` 是其他可失败回调路径共享的错误变体。需要把用户错误交给指定 handler 时使用 `Callback::dispatch`，它统一返回 `HandlerError`。

回调节点只在 owner 活动期间有效。递归调用同一个正在运行的回调会得到 `ReactiveError::BorrowConflict`；回调 panic 会在恢复回调节点后继续向调用方传播，不应依赖 panic 作为业务错误通道。

### `StoredValue`

`OwnerAccess::stored` 创建一个不参与依赖图的作用域值。`with` 和 `update` 不会触发 effect，也不会通过读取建立订阅，适合放置缓存、框架状态或 cleanup 需要释放的资源。

普通作用域关闭后，`StoredValue` 和其他节点一样不可用。但在最终 owner cleanup 正在运行的短窗口内，`StoredValue::with`/`update` 仍可访问它；此时 scope 已经被标记为 inactive，只有 stored value 享有这个清理例外，异步代码或 effect rerun 不能利用它。

### `NodeRef`

`OwnerAccess::node_ref` 管理 `Option<T>` 宿主引用，提供 `get`、`set` 和 `clear`。它不追踪读取，也不会因为设置引用而把值传播为响应式计算；它的作用是让框架把宿主对象和 owner 生命周期绑定起来。

## 追踪、动态依赖与批处理

### `untrack`

`OwnerAccess::untrack` 在回调期间暂时关闭当前 runtime 的依赖收集：

```rust
scope.untrack(|| {
    let _ = source.get()?;
    Ok::<(), ReactiveError>(())
})??;
```

它适合只想读取一次快照、但不想让当前 effect 因该 signal 重跑的场景。`untrack` 仍然保留作用域和节点有效性检查；它不是把失效句柄变成有效句柄的办法，也不会把不同 runtime 的节点合并到同一张图中。

### 动态依赖

计算每次成功运行后，依赖集合会按照本次实际读取替换。例如条件分支只读取当前分支的 signal，切换条件后旧分支会被移除，新分支会被加入。依赖事务在闭包返回错误或发生运行时失败时回滚，因此失败运行中临时读取的节点不会留下半成品订阅。

这条规则也适用于嵌套 scope：子 scope 中建立的节点在子 scope 结束时释放，不能因为父 effect 曾经读取过它们就继续留在队列中。

### `batch`

普通 signal 写入在 scheduler 空闲时会刷新受影响的队列。需要把多次写入表达为一次逻辑更新时使用 `OwnerAccess::batch`：

```rust
scope.batch(|| {
    set_left.set(2)?;
    set_right.set(3)?;
    Ok::<(), ReactiveError>(())
})??;
```

batch 只延迟队列刷新，不改变写入值，也不会让依赖追踪变成 untracked。闭包结束时运行时会尝试刷新队列；如果闭包 panic，panic 会继续传播，但 batch 深度和 scheduler 状态仍会恢复。

### 多 signal 的底层事务

框架内部可以通过 `OwnerAccess::reactive_transaction` 创建绑定当前 owner 的
`ReactiveTransaction`。事务会复制并暂存每个 signal 的值，在 `commit()` 前验证目标、
owner 和 runtime，然后一次性应用和发布；`abort()` 或直接 drop 会丢弃尚未发布的值。
带用户闭包的 `update` 用 `TransactionOperationError<E>` 区分用户错误和
`TransactionError`，同一个 signal 重复登记会返回 `DuplicateTarget` 并使事务进入
`Poisoned` 状态。该创建入口标记为 `doc(hidden)`，普通应用通常使用 `batch`；事务的
公开类型主要服务于框架级批量更新和一致性测试。

## 运行时隔离与调度错误

同一个 `Runtime` 的不同 owner 可以建立跨 scope 的追踪边，前提是依赖节点和 observer 都仍然有效。不同 `Runtime` scheduler family 之间：

- 普通 `get`/`with` 追踪读取会先返回 `ReactiveError::RuntimeMismatch`，不会先执行脏计算。
- `get_untracked`/`with_untracked` 可以读取 foreign runtime 的值，但不会建立订阅；读取仍然受目标节点生命周期和动态借用校验保护。

运行时还会报告 `Reentrant`、`BorrowConflict`、`NoSuchNode` 和 `NonConvergent` 等错误。`Reentrant` 通常表示在计算仍持有运行租约时递归读取同一个计算；`NonConvergent` 表示 effect 队列在内部迭代预算内没有收敛。业务代码应传播这些 `ReactiveError`，框架边界再决定记录、停止或重建哪棵作用域树。

写入 signal 会在 scheduler 空闲时刷新队列，因此一次 `set` 可能同步运行受影响的
effect。若 effect 在运行中继续写入依赖链，队列会继续处理新任务；设计相互写入的
effect 时必须确保最终稳定。`NonConvergent` 是运行时拒绝无限刷新的一种保护，出现
后应修复反馈环或停止相关 effect，而不是通过重复调用写入来“等待”它自行恢复。

## 对应测试

- signal、callback、stored value 和 node ref：`crates/silex_reactivity/tests/nodes.rs`
- computed、依赖链、动态依赖和 batch：`crates/silex_reactivity/tests/graph.rs`
- 自动追踪与 untracked：`crates/silex_reactivity/tests/automatic_tracking.rs`
- computed/watch 的用户错误：`crates/silex_reactivity/tests/fallible_derived.rs`、`tests/fallible_memo.rs`、`tests/watch.rs`
- panic、re-entry 和批处理恢复：`crates/silex_reactivity/tests/panic_reentry.rs`
- 不同 runtime 的兼容性：`crates/silex_reactivity/tests/runtime_compatibility.rs`
- 事务与 guard：`crates/silex_reactivity/src/transaction.rs`、`crates/silex_reactivity/tests/signal_guards.rs`
