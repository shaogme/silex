+++
title = "作用域与生命周期"
description = "owner、临时作用域、子作用域、清理、错误处理和 completion 的生命周期语义。"
weight = 20
+++

# 作用域与生命周期

作用域是 `silex_reactivity` 管理节点、类型化 payload、error handler 和 cleanup 顺序的基本单位。公开节点句柄携带创建它们的作用域生命周期；运行时关闭则通过 owner registry、代数化身份和动态借用检查拒绝 stale handle。

这里的“生命周期”有两层含义：Rust 生命周期限制句柄能否从闭包中逃逸，运行时生命周期则决定句柄在 owner close、子 scope 退出或节点 stop 后是否仍然有效。两层检查都通过后，操作才会真正访问 payload。

本文的短代码片段用于展示 API 关系，省略了外层函数、具体错误枚举和部分业务函数；需要可编译示例时，请使用总览页引用的 `docs/examples/silex_reactivity/basic.rs`。

## Runtime、root owner 与 access

`Runtime` 只保存显式的 scheduler 边界和 root 活动状态。常用的两种入口如下：

```rust
let mut runtime = Runtime::new();

runtime.with_transient(|scope| {
    let (read, write) = scope.signal(0_i32)?;
    write.set(1)?;
    read.get()
})??;

let root = runtime.owner()?;
root.with_access(|scope| {
    let (_read, write) = scope.signal(0_i32)?;
    write.set(1)
})?;
root.close()?;
```

`Runtime::with_transient` 使用高阶 trait bound：回调获得的 `OwnerAccess<'scope>` 只能在本次回调中使用。回调正常返回后，运行时自动以 child-first 顺序关闭 transient owner；回调 panic 会在清理完成后继续向调用方传播。

`Runtime::owner` 返回拥有显式 close 权限的 root `OwnerHandle`。同一个 `Runtime` 只有一个活动 root；root 关闭或 drop 后，runtime 才能创建下一个 root。`OwnerHandle::access`、`with_access` 和 `with_access_async` 借出 `OwnerAccess`，但不把 close 权限交给借用视图：

- `OwnerHandle` 可以跨多个调用保存，负责 `close` 和 `is_active`。
- `OwnerAccess<'owner>` 是 `Copy` 的借用能力，负责创建节点、注册 cleanup、建立 handler 和执行 runtime 操作。
- `with_access_async` 只允许 future 携带与 owner access 相同的借用生命周期；它不是把 scope capability 转换成 `'static`，也不改变运行时的单线程约束。

`with_access_async` 只约束 Rust 借用，不会自动延长 owner 的运行时寿命。调用方仍
应在替换页面或分支时先停止任务、取消 completion，之后再关闭 owner；如果 future
在 owner 已进入关闭流程后继续使用句柄，操作会返回 `NoSuchNode` 或其他运行时错误。
需要把数据从异步任务送回作用域时，优先使用本页后面的 completion endpoint，而不是
把 `OwnerAccess` 或节点句柄转换为 `'static`。

## 持久子 owner 与 transient 子 scope

需要替换一个长期存在的分支时，从 `OwnerHandle` 调用 `create_child`，或从 `OwnerAccess` 调用 `create_child`，得到新的 `OwnerHandle`：

```rust
let root = runtime.owner()?;
let child = root.create_child()?;

child.with_access(|scope| {
    let (_read, write) = scope.signal(0_i32)?;
    write.set(1)
})?;

child.close()?;
root.close()?;
```

持久子 owner 使用和父 owner 相同的 scheduler，但拥有自己的 owner registry 条目和关闭权限。父 owner 关闭时会先递归关闭子 owner；如果调用方先关闭子 owner，父 registry 会保留足够的代数和拓扑信息，使父 close 仍然是安全的幂等操作。

`OwnerAccess::with_transient` 适合只在父计算或父回调的单次执行中创建局部节点：子 scope 退出时，其节点、回调和 cleanup 一起释放。不要把这种局部节点交给异步任务或保存到父 effect 的下一次运行中；编译器通常会通过生命周期直接拒绝，运行时也会在无法由编译器表达的边界返回 `NoSuchNode`。

## 节点的归属和停止

在一个计算正在运行时，通过普通 `OwnerAccess` 创建的 signal、callback、computed 或 effect 会记录为该计算的子节点。下一次重新运行计算前，运行时先释放旧的子节点，再执行旧 cleanup，然后才执行新的计算闭包。这样，分支切换和重复注册不会把上一轮的局部资源遗留在图中。

`OwnerAccess::effect_detached` 是框架专用的隐藏入口，它把 effect 直接挂在 owner 下，不挂在当前计算节点下。框架需要让一个 effect 跨父计算重跑时才使用它；普通应用代码应使用 `effect`，并显式保存 `EffectHandle` 后在分支替换时调用 `stop`。

`EffectHandle::stop` 会取消该 effect 的排队任务、释放其子节点和 cleanup，并返回是否真的停止了一个活动节点。它不会重新激活 effect；owner close 时仍会清理 owner 剩余节点。

## Cleanup 注册与执行顺序

通过 `OwnerAccess::on_cleanup` 注册一次性 `FnOnce() -> Result<(), E>`：

```rust
let handler = scope.error_handler(|error: ReactiveError| {
    log_error(error);
})?;

scope.on_cleanup(
    || {
        release_resource()?;
        Ok::<(), ReactiveError>(())
    },
    handler,
)?;
```

注册位置取决于调用时是否有当前计算：

- 在 effect、computed 或 watch getter/callback 的计算上下文中，cleanup 属于当前计算；下一次该计算运行或该节点 stop 时执行，并由下一次成功运行重新注册新的 cleanup。
- 没有当前计算时，cleanup 属于 owner；owner 最终关闭时执行。
- cleanup 内的普通 signal 读取是 untracked，不会把 cleanup 意外加入响应式依赖。

计算重新运行时，顺序是“子节点 cleanup → 当前计算已有 cleanup → 新计算闭包”；owner 关闭时，顺序是“子 owner → owner 节点/计算 → owner cleanup”。同一层的 cleanup 需要按注册的生命周期顺序理解，不能依赖内部 node id 或容器索引。

最终 owner cleanup 开始时，scope 已经被标记为 inactive，普通 signal、computed、callback 和 node ref 操作都会失败。唯一的访问例外是 `StoredValue::with` 和 `StoredValue::update`：它们可以在 pending cleanup 释放 stored payload 前同步访问一次。该例外不适用于 effect rerun、异步回调或 cleanup 返回之后的代码。

## 显式关闭、Drop 与 panic

`OwnerHandle::close`、completion 的 `cancel` 以及 owner close 产生的 `CloseError` 都使用结构化失败，而不是只返回第一条字符串：

- `CloseError::entries()` 暴露每条失败的 `ClosePhase`、`CloseSource` 和 `CleanupFailure`。
- `CleanupFailure` 可以是 runtime 错误、handler 错误或带稳定诊断的 cleanup panic。
- 关闭流程会尽量继续后续 child、effect 和 cleanup 阶段，因此调用方应保留并记录完整的 `CloseError`。
- 如果动态借用冲突使某个阶段可重试，owner 不会标记为已释放；释放借用后可以重试 `close`。已经释放的 owner 再次 close 是成功的幂等操作。

owner 的 `Drop` 不能返回错误，因此会尽力关闭并把无法交给调用方的 close 诊断放入 runtime 队列：

```rust
drop(root);
let close_errors = runtime.take_unhandled_close_errors()?;
for error in close_errors {
    record_close_error(error);
}
```

cleanup panic 不应被当成普通业务错误吞掉。显式 close 会把它转换为 `CloseError`，同时尽量继续其他清理；transient scope 会把它包装为 `TransientScopeError::Close`。计算或 callback 的普通 panic 则继续传播，但运行时会先恢复 observer、借用和节点终态，避免下一次独立操作继承损坏的动态状态。

同一 owner 内的 root cleanup 按注册顺序执行；嵌套 owner 和计算节点则遵循子节点
先于父节点的词法拓扑。不要把 cleanup 注册顺序误当成“失败即停止”：一个 cleanup
失败后，运行时仍会尝试同一关闭阶段的剩余工作，并将失败集中到 `CloseError` 或
handler dispatch 路径。需要严格验证顺序时，应使用事件记录测试，而不是依赖内部
node id。

## Error handler 的所有权

`OwnerAccess::error_handler` 返回 `ErrorHandlerToken<'scope, E>`。计算、watch、callback、cleanup 和 completion 可以按值、按引用或通过 token 的 `view()` 使用它：

```rust
let errors = scope.error_handler(|error: ReactiveError| {
    record_error(error);
})?;

scope.effect(
    move || source.get().map(|_| ()),
    &errors,
)?;
```

token 是注册记录的 RAII 强引用；显式调用 `close` 或 drop 最后一个 token 后，新的、没有活动 lease 的分发会得到 inactive/retired 诊断。计算创建时会为自己的回调保留 handler lease，所以 token 释放后，已有计算可能在 lease 仍有效期间继续分发；lease 结束后 handler 才会退休。不能在计算注册完成后立刻把仍在运行的 handler 当作已经安全销毁的外部资源。

框架如果需要在调用者放弃 token 后继续持有处理器，可以使用 `ErrorHandlerRef::anchor` 得到 `ErrorHandlerAnchor`；这是生命周期适配入口，不应替代普通应用的 token 管理。handler callback 只能在所属 scope 和 scheduler 有效时运行，也不能跨线程发送。

## Callback 与 completion

### 作用域 callback

`OwnerAccess::callback` 返回带 `T`、`E` 类型的 `Callback`。`invoke` 保留运行时错误和用户错误的区分；`dispatch` 则把用户错误交给传入的 handler。callback 仍然是 owner 节点，owner 关闭后任何外部保存的 copy 都只能得到失效错误。

### CompletionOnce

`completion_once` 用于一个异步任务只回传一次结果：

```rust
let completion = scope.completion_once(unwind_safe(|value: i32| {
    println!("completion: {value}");
    Ok::<(), ReactiveError>(())
}))?;

let accepted = completion.submit(42)?;
```

`CompletionOnce::submit` 第一次提交会先验证 owner 和 callback，再调用用户回调，最后关闭并释放 callback 节点；即使用户回调返回错误，也不会允许第二次提交。返回 `Ok(true)` 表示本次提交被接受，owner 已关闭、endpoint 已 cancel 或已经提交过则返回 `Ok(false)`。callback 错误和 close 错误可能同时发生，应使用 `CompletionSubmitError::into_parts` 拆开处理。

completion callback 可能捕获 `Rc<RefCell<_>>` 等不满足 unwind-safety 的状态。只有确认该状态可以承受 panic unwind 时，才使用 `unwind_safe` 适配器；它只是显式的安全断言，不是线程安全或业务回滚保证。

### CompletionSender

`completion_sender` 用于多次回传：

- `clone` 得到的 sender 共享同一个终态和 callback；用户错误不会自动关闭 sender，下一次 submit 可以重试。
- 显式 `cancel` 会关闭 endpoint 并释放 callback，重复 cancel 是幂等的。
- 最后一个 active clone drop 时会取消 endpoint；如果长期 owner 被替换但 sender clone 仍被异步任务持有，必须在替换流程中显式 cancel。
- callback panic 会先关闭 callback 节点，再恢复 panic；后续提交返回 `Ok(false)`。

completion 的 `T` 必须满足 `'static`，因为提交者可能独立于创建 callback 的栈帧；错误 `E` 仍受 owner 生命周期约束。两者都不改变 runtime 单线程限制，sender 不能被当作跨线程 channel。

## 异步任务的推荐关闭流程

异步任务持有 completion 时，建议按下面顺序组织框架生命周期：

```text
创建 owner
  ↓
在 owner 内注册 completion
  ↓
启动任务并保存 completion clone
  ↓
owner 被替换/关闭 ──→ cancel completion
  ↓                         ↓
任务稍后 submit ───────→ 检查 Ok(false) 或结构化错误
```

`submit` 返回 false 不是网络失败，也不是用户 callback 错误，而是目标作用域已经不再接受结果。调用方应该停止更新旧 UI 或旧状态，并让任务本身完成资源回收；不要为了让提交成功而延长旧 owner 的生命周期。

## Runtime 身份检查

`OwnerAccess::same_runtime` 只比较 scheduler family，不比较两个 owner 是否相同。
同一 `Runtime` 的 root、持久子 owner 和 transient 子 scope 可以建立跨 owner 的
追踪边；两个不同 `Runtime` 即使节点类型和 Rust 类型完全相同，也不能建立 tracked
依赖。需要跨 runtime 读取只读快照时使用 `get_untracked`、`with_untracked`，并接受
它不会订阅源节点这一语义。

这项限制也适用于线程边界：`Runtime`、owner access、节点句柄、handler 和
completion 都服务于单线程 scheduler，不应放入跨线程 channel 或全局共享状态。若
确实需要跨线程传递数据，应在线程边界传递拥有数据，再在目标 runtime 中重新注册
signal 或 completion。

## 生命周期测试索引

- transient/root close、cleanup 顺序和 panic：`crates/silex_reactivity/tests/runtime_scope.rs`、`tests/root_scope.rs`
- 持久子 owner、父子关闭和 owner registry：`crates/silex_reactivity/tests/owned_scope.rs`
- completion 的一次性、重复提交、cancel 和 panic：`crates/silex_reactivity/tests/completion.rs`
- handler token、lease、anchor 和退休：`crates/silex_reactivity/tests/error_handler.rs`
- 生命周期编译约束：`crates/silex_reactivity/tests/ui/fail_child_handle_escape.rs`、`fail_callback_escape.rs`、`fail_handler_escape.rs`

维护这部分代码或文档时，至少回答：句柄是否仍在 owner 生命周期内、owner close 后是否还有异步提交、显式 close 错误是否被记录、cleanup 是否意外建立依赖，以及新的 unsafe/类型擦除边界是否有对应的失败测试。
