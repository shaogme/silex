+++
title = "响应式值与派生"
description = "silex_core 的 signal、computed、effect、watch、promotion 和响应式 trait。"
weight = 10
+++

# 响应式值与派生

`silex_core` 的响应式 API 通过 `OwnerAccess` 创建节点，通过 `SilexResult` 暴露底层运行时错误。tracked 读取在当前计算中建立依赖；untracked 读取只取快照。派生节点的闭包每次成功运行后才提交新值和本次读取的依赖，失败运行不会把临时依赖留在图中。

读取和写入主要由 trait 提供：`RxBase` 用于只建立依赖，`RxGet` 提供
`get`/`get_untracked`，`RxRead` 提供 guard 和闭包读取，`RxWrite` 提供
写 guard 与写入操作。示例假定已导入
`silex_core::traits::{RxBase, RxGet, RxRead, RxWrite}`；使用 `prelude` 也可以一次导入这些 trait。

本文的代码片段省略外层函数和错误处理辅助函数，只用于说明真实 API 的调用关系；完整可编译示例在 `docs/examples/silex_core/basic.rs`。

## 节点选型

| 类型 | 读 | 写 | 是否进入依赖图 | 适用场景 |
| --- | --- | --- | --- | --- |
| `ReadSignal<T>` | `get`、`with` | 无 | 是 | 将读能力交给视图或计算。 |
| `WriteSignal<T>` | 无 | `set`、`update`、`notify` | 写入会通知 | 将写能力限制在事件或控制器。 |
| `Signal<T>` | 读写 | 读写 | 是 | 同时需要两种能力的局部状态。 |
| `Computed<T>` | `get`、`with` | 无 | 是 | 需要 `PartialEq` 的缓存派生值。 |
| `Rx<T>` | 统一只读访问 | 无 | 取决于内部 source | 在组件 API 中统一接收 signal、computed 或 stored value。 |
| `StoredValue<T>` | `with` | `update`、`set` | 否 | owner 管理的普通状态、缓存或异步控制器。 |
| `NodeRef<T>` | `get` | `load`、`clear` | 否 | 宿主对象引用，不是响应式值。 |

所有这些句柄都带 scope lifetime，并且在 owner 关闭后失效。`StoredValue` 的清理例外和 `NodeRef` 的 `Option` 语义见[生命周期专题](lifecycle.md)。

## Signal 读写

`Signal` 同时实现 `RxRead` 和 `RxWrite`，可以直接获取具名的读写 guard。
读取 guard 实现 `Deref`，写入 guard 实现 `DerefMut`；guard 存活期间会保持
runtime 的动态借用。

`silex_core::ReadGuard` 和 `silex_core::WriteGuard` 在底层 runtime guard 外再封装
了一层，统一把释放、提交和回滚操作转换为 `SilexResult<()>`。因此调用方只需使用
`finish()?`、`commit()?` 或 `abort()?`，不需要重复写
`.map_err(SilexError::fatal)`；guard 的 Drop 仍保留底层的 best-effort 清理行为。

```rust
let signal = owner.signal(0_i32)?;
let read_guard = signal.read()?;                 // tracked
let current = *read_guard;
read_guard.finish()?;

let snapshot_guard = signal.read_untracked()?;   // 不建立依赖
let snapshot = *snapshot_guard;
snapshot_guard.finish()?;

let mut write_guard = signal.write()?;
*write_guard += 1;
write_guard.commit()?;
```

`read()` 会在当前 observer 存在时建立依赖，`read_untracked()` 只校验句柄并
读取当前值，不建立依赖。`finish()` 会显式结束读 guard；如果直接丢弃 guard，
其 Drop 实现仍会执行 best-effort 清理。`write()` 获取独占写 guard，修改完成
后应显式调用 `commit()`，让写入和队列刷新返回可处理的错误；未显式结束时，
Drop 会执行 best-effort commit。

`with`/`with_untracked` 适合引用只在闭包内有效的短访问；guard 适合需要在一段
连续逻辑中直接访问 payload 的场景。持有读 guard 时获取同一 signal 的写 guard
会返回借用冲突，而不是绕过 owner 校验。

`Signal` 的 `get`、`get_untracked`、`set` 和 `update` 仍可用于 clone-based 或
闭包式的便利访问；它们不会改变 guard 的生命周期和动态借用规则。

通过 `Cell`、`RefCell` 或其他内部可变容器绕过 `set`/`update` 时，运行时看不到 payload 的变化；修改完成后必须调用 `WriteSignal::notify`。`notify` 不负责比较新旧值，也不替代正确的响应式写入。

## 多 signal transaction

当库存、余额和订单计数必须共同成功时，使用 `OwnerAccess::transaction`。事务
中的 `snapshot` 会读取不建立依赖的 clone，`update` 和 `set` 只修改暂存值；
闭包返回 `Ok` 后才会统一发布，任一用户错误、运行时错误或 `?` 提前返回都会
丢弃全部暂存写入。

```rust
let result = owner.transaction(|transaction| {
    let item_name = transaction.snapshot(item)?;
    let requested = transaction.snapshot(quantity)?;
    let price = requested.saturating_mul(10);

    let remaining = transaction.update(stock, |available| {
        if item_name.trim().is_empty() {
            return Err(SilexError::recoverable(SilexErrorKind::Framework(
                "商品名称不能为空".to_string(),
            )));
        }
        if *available < requested {
            return Err(SilexError::recoverable(SilexErrorKind::Framework(
                "库存不足".to_string(),
            )));
        }
        *available -= requested;
        Ok(*available)
    })?;

    let balance_left = transaction.update(balance, |available| {
        if *available < price {
            return Err(SilexError::recoverable(SilexErrorKind::Framework(
                "余额不足".to_string(),
            )));
        }
        *available -= price;
        Ok(*available)
    })?;

    let orders = transaction.update(order_count, |count| {
        *count = count.saturating_add(1);
        Ok(*count)
    })?;

    Ok((item_name, requested, remaining, balance_left, orders))
});

match result {
    Ok((item_name, requested, remaining, balance_left, orders)) => {
        status.set(format!(
            "订单已提交：{item_name} × {requested}，库存 {remaining}，余额 {balance_left}，订单数 {orders}"
        ))?;
    }
    Err(error) => status.set(format!("提交失败：{error}"))?,
}
```

`status.set` 位于事务成功之后，因此它是 UI 状态副作用，不会成为库存提交的
一部分。不要把网络请求、日志或 DOM 操作放进 transaction closure；这些操作
无法由 runtime 回滚。事务只接受 owner 作用域内的 signal，不能逃出该作用域，
也不能保存到 future 后跨越 `await`。普通 `WriteGuard` 继续保持原有的
best-effort Drop/commit 语义；它不是 transaction 的暂存写入，也不能通过
`abort` 恢复已经发布的 live payload。

## Computed、effect 与 watch

### Computed

`OwnerAccess::computed` 要求输出实现 `PartialEq`，首次创建时立即运行闭包，只有新输出不相等时才向下游传播。`computed_always` 不比较输出，每次成功求值都通知下游：

```rust
let doubled = owner.computed(
    move || Ok::<_, SilexError>(source.get()? * 2),
    error_handler,
)?;

let formatted = owner.computed_always(
    move || Ok::<_, SilexError>(format!("value={}", doubled.get()?)),
    error_handler,
)?;
```

创建时的用户错误会作为 `SilexError` 返回；创建成功后，重新计算或显式读取仍可能失败。`Computed::map` 和 `Rx::map` 创建 always-notifying 派生值，并把读取错误交给传入的 handler。

### Effect

`OwnerAccess::effect` 注册 `FnMut() -> SilexResult<()>`，创建时先运行一次。闭包中普通 `get`/`with` 读取的 source 成为依赖，依赖变化时 effect 再运行。`EffectHandle::stop` 返回 `true` 表示本次停止了活动 effect，返回 `false` 表示它已经停止或 scope 已失效。

所有 effect-like API 都必须显式选择 `EffectPhase::Normal` 或
`EffectPhase::PostFlush`。初始执行仍然同步发生；phase 只决定依赖变化后的重跑
顺序。`Normal` 工作会先完全收敛，`PostFlush` 回调中的 signal 写入会在下一个
PostFlush 回调前重新进入 Normal 队列。PostFlush 是 runtime 内的同步阶段，不等同
于浏览器的 microtask 或 animation frame。

`effect_with_previous` 的闭包接收 `Option<&T>`，只有上一次成功返回的值才会作为下一次的 previous；失败运行不会覆盖 previous。`effect_detached` 是隐藏的框架入口，普通应用应使用 `effect`。

### Watch

`watch`/`watch_with_options` 接收一个 `ReactiveSource`，`watch_getter`/`watch_getter_with_options` 接收显式 getter。getter 结果必须实现 `PartialEq`，回调签名为 `FnMut(&T, Option<&T>) -> SilexResult<()>`：

```rust
let watcher = owner.watch_getter_with_options(
    EffectPhase::Normal,
    move || source.get(),
    move |current, previous| {
        record_change(current, previous);
        Ok(())
    },
    error_handler,
    WatchOptions::default().immediate().once(),
)?;
```

- getter 初始时总会运行以建立依赖；`immediate()` 才会在第一次求值时调用回调；
- `once()` 在第一次实际调用回调后停止，因此 `immediate().once()` 会在初始化回调后立即停止；
- 回调在 untracked 上下文执行，回调内部读取的 source 不会变成 watcher 依赖；
- getter 的 source 变化后，只有新结果与上一次结果不相等时才调用回调；
- 返回的 `EffectHandle` 仍可显式 `stop`。

## 统一值与输入提升

`Rx<'scope, T>` 是 crate 根的统一只读值，内部保留三类来源：`ReadSignal`、computed 和 stored value。`Signal<'scope, T>` 是同时持有读写句柄的可写 signal，不是 `Rx` 的 facade；通过 `Signal::into_rx()` 可在只读边界取得 `Rx`。`is_constant` 只对 StoredValue-backed `Rx` 返回 `true`；它表示当前来源不是依赖图中的 signal，不表示值永远不会被 `StoredValue::set` 修改。

`ReactiveSource<'scope>` 和 `PromotionPlan<'scope, T>` 用于把输入延迟到 target owner 再物化：

- 已存在的 scoped source 直接保留原句柄，不重复创建节点；
- primitive、`String`、`&str` 和 `Constant<T>` 作为 constant，在目标 owner 中创建 owner-owned 值；
- tuple source（最多六个元素）、`SignalSlice`、`Resource` 和 `Mutation` 会生成 derived computed，并追踪其成员；
- `OwnerAccess::promote` 在创建 target-side 节点前接收 `ErrorReporter`，并检查 target owner 是否与 source 属于同一个 runtime。

实现自定义 source 时，`into_promotion_plan` 阶段不能注册节点；只有 `PromotionPlan::derived` 的 materializer 才能使用传入的 `OwnerAccess` 创建节点。materializer 不能创建新的 `Runtime`、detached owner 或线程局部 runtime。

## Trait 适配层

`silex_core::traits` 把不同节点统一为可组合输入：

| Trait | 作用 |
| --- | --- |
| `RxValue` | 暴露关联的 `Value` 类型。 |
| `RxBase` | 只建立依赖而不借用或 clone payload；适用于非 `Clone` source。 |
| `RxRead` / `RxGet` | `RxRead` 提供 `ReadGuard` 关联类型、`read`、`read_untracked` 和闭包访问；`RxGet` 提供 clone-based `get`/`get_untracked`。 |
| `RxWrite` | 提供 `WriteGuard` 关联类型、`write`，以及 `set`、`update`、`notify` 和 setter/updater 闭包。 |
| `RuntimeScoped` | 暴露 source 保存的 `OwnerAccess`，供 runtime provenance 校验。 |
| `RxFrom` / `RxDefault` | 在显式 owner 中从值或 `Default` 创建 owner-owned wrapper。 |
| `ReactiveInput` | 将既有 scoped source 或支持的常量转换到目标 wrapper；不会隐式创建 runtime。 |
| `RxOptionExt` | 对 `Option<T>` source 提供 `map_or`、`unwrap_or`、`and_then`、`is_some_and` 等派生。 |
| `ForLoopSource` | 将 `Vec<T>`、`Option<Vec<T>>` 或 `SilexResult<Vec<T>>` 统一暴露为 slice。 |

tuple 的 `RxRead` 会逐个短暂借用成员并创建 owned tuple，因此聚合 `get`/`with` 要求成员 `Clone`；tuple 不实现 `RxWrite`，因为多个独立 source 没有统一的事务写入语义。

## projection、map 与逻辑运算

`SignalSlice` 通过 `signal.slice(|value| &value.field)` 进行安全 projection；它的
`read()` 返回 `MappedReadGuard`，同时持有 source guard，因此字段引用只在 source
借用有效期间暴露。作为 `ReactiveSource` 使用时会 clone 字段建立 derived
computed。tuple、`Resource` 和 `Mutation` 的读取结果则是 `OwnedReadGuard` 快照，
不会把底层 payload 借用暴露给调用方；`Rx` 会用 `RxReadGuard` 保留具体 source
变体的 guard。

`logic::*` 中的 trait 都会在显式 owner 中创建派生节点：

- `Map::map` / `map_fn` 创建 always-notifying 的转换；
- `ComputedSource::computed` 创建 equality-gated 的 computed；
- `ReactivePartialEq` 提供 `equals`/`not_equals`；
- `ReactivePartialOrd` 提供四种大小比较；
- `Rx<T>` 提供 `add`、`sub`、`mul`、`div`、`rem`、位运算、`neg` 和 `not` 等方法，要求对应的引用运算 trait。

这些方法接受 primitive 或其他 `ReactiveSource` 作为输入；常量会在目标 owner 物化，scoped source 则保留依赖 provenance。每个派生操作都需要错误 handler，因为 source 读取和 runtime 操作可能失败。

## 批量读取与宏

`batch_read!(a, b => |left: T, right: U| ...)` 嵌套调用每个 source 的 tracked `with`，适合需要在一个闭包中读取多个值而又不复制整个 API 的场景。`batch_read_untracked!` 使用 `with_untracked`，不会为这些 source 建立依赖，但宏体内部显式执行的 tracked 读取仍然有效。

`rx!` 需要 `SilexContext` 或满足 `SilexContextProvider` 的 context：

```rust
let value = rx!(ctx; source.field)?;
```

宏表达式返回 `SilexResult<Rx>` 或 `SilexResult<Callback>`；生成代码内部通过 `?` 传播 scope、promotion 和初始计算错误，不再要求调用函数本身必须能够接收宏内部的提前返回。调用方可以使用 `?`，也可以用 `match` 显式处理结果。`$(source.field)` 形式用于把字段本身作为 reactive source（例如 store 宏生成的字段）；普通 `$source` 则按照已有 tracked value access 读取。宏的内部展开入口 `__internal_rx` 和递归 batch 宏不是应用层稳定 API。

## 依赖与批处理语义

- `track` 和 tracked 读取会在当前 runtime observer 存在时建立边；untracked 读取不会建立边，但仍检查句柄和 dynamic borrow；
- 每次 computed/effect/watch getter 成功运行后，依赖集合按本次实际读取替换，条件分支切换会移除旧 source；
- `OwnerAccess::batch` 延迟队列刷新，把多个写入合并为一次逻辑刷新，但不改变 tracked/untracked 规则；
- 普通 signal 写入可能同步刷新受影响的 effect，因此写入 API 返回的 `SilexResult` 也必须处理；
- feedback loop 可能触发底层 scheduler 的 `NonConvergent`，这不是应通过重复写入来恢复的暂时错误。

## 对应测试

- tracked/untracked 与 batch：`crates/silex_core/tests/batch_read.rs`
- tuple source、clone snapshot 和 tuple `get`：`crates/silex_core/tests/tuple_traits.rs`
- watch 的 promotion、`immediate`、`once` 和 batch：`crates/silex_core/tests/watch.rs`
- 借用冲突、stale node、`NodeRef` 和内部可变性：`crates/silex_core/tests/reactivity_errors.rs`
- scoped guard、owned snapshot 和 projection 借用：`crates/silex_core/tests/signal_guards.rs`
- transaction 的快照、原子提交和回滚：`crates/silex_core/tests/transaction.rs`
- runtime provenance：`crates/silex_core/tests/runtime_compatibility.rs`
