+++
title = "owner 生命周期与宿主资源"
description = "silex_dom 的 MountOwner、MountState、cleanup、事件 callback 和 window 资源边界。"
weight = 50
+++

# owner 生命周期与宿主资源

DOM 节点只是可见结果，真正的清理边界是 `MountOwner`。每个元素、复合
view、动态 row 和 branch 都可以创建局部 owner；局部 owner 共享当前
`OwnerAccess` 的 runtime 能力，但拥有独立的 DOM effects、cleanups、child
树和宿主资源列表。关闭局部 owner 时，不需要等整个应用 root 关闭。

## `MountOwner` 的职责

自定义 `View` 实现通过 `&dyn MountOwner<'scope>` 使用以下能力：

| 方法 | 用途 |
| --- | --- |
| `effect(callback, handler)` | 注册跟随 owner 的 detached reactive effect；首次注册时立即运行。 |
| `on_cleanup(cleanup, handler)` | 注册一次性的局部资源清理；cleanup 失败交给 handler/close report。 |
| `token()` | 取得可保存到当前 view 资源中的 `MountOwnerToken`。 |
| `child()` | 创建普通 DOM 子 owner，关闭时先于父 owner。 |
| `branch_child()` | 创建具有独立 persistent runtime child 的 branch owner。 |

`MountOwnerToken` 还提供 `effect_with_previous`、`owner_state` 和若干内部
host callback/resource 安装能力。公开 `MountState<T>` 是局部 owner-bound
状态容器，适合保存 row、dynamic render state 或 cleanup 需要取出的资源。

## 关闭顺序与错误

局部 owner close 的顺序是：

1. 标记 local state inactive，拒绝新的 effect、cleanup、state 和 host resource；
2. 逆序关闭 child owner；
3. 停止由该 runtime owner 持有的 effects；
4. 逆序运行 local cleanups；
5. runtime disposal 在安全边界 drain pending completion endpoint，并按
   owner/node ID 去重；
6. 聚合 `CloseError`，并通过 `CleanupReporter` 或 error handler 报告。

branch content 的 runtime handles 由 branch persistent child 管理，因此
content owner 关闭时不会重复 stop 已由 runtime 递归清理的 handles。这个
分支是为了避免 double close，不是允许 content 资源脱离 branch 生命周期。

cleanup 注册成功后，owner 会在 core owner 上注册一个总 cleanup；这使得
直接关闭 `OwnerHandle`、父 element cleanup、动态 row replacement 和
`MountedApp::dispose` 都最终走同一条 local close 路径。清理闭包可以返回
`SilexResult<()>`，也可能 panic；两者都进入结构化 close report，而不是
静默丢弃。

## `MountState<T>` 的访问规则

`owner_state(value)` 返回带 scope 的 `MountState<T>`，其 `with`、`update`、
`take`、`replace` 会在 owner active 时工作；owner 关闭后返回
`SilexError::fatal(ReactiveError::NoSuchNode)`。cleanup 阶段内部可以使用
cleanup-only 的取值路径完成最后一次释放，但普通应用不应绕过 active
检查继续修改已关闭状态。

`MountState` 解决的是“把 row 或资源的所有权交给 owner，但在 cleanup 时
仍需取出它”的问题。不要用全局 `Rc<RefCell<T>>` 替代它来绕过 scope：那
会让 stale callback 继续持有已移除 DOM 的风险失去结构化边界。

## 事件与 completion destination

DOM listener、window listener 和 timer callback 不直接调用带 scope 的
Rust closure。它们先通过 owner 创建 `HostCallback`，再由
`CompletionSender` 把 JS payload 投递回当前 runtime owner：

```text
DOM/Window callback
        │ JsValue
        ▼
HostCallback gate ── inactive? drop payload
        │ active
        ▼
CompletionSender ──► owner callback / error handler
```

owner close 会关闭 gate、取消 destination，并释放 JS `Closure` 或 timer
resource；异步 dispatch 在 close 之后不会调用用户 closure。callback 返回
的 `SilexError` 由对应 handler 接收；destination submit 同时出现 callback
error 与 close error 时，维护代码必须保留两类诊断。

completion endpoint 在 runtime disposal 已经进行时不会递归调用
`dispose_nodes`。它只把自身的 `(owner_id, node_id)` 登记到 pending 队列，
由外层 disposal transaction 在 ownership tree 稳定后统一关闭；重复登记只
保留一项。若节点已由当前 disposal 批次移除，drain 会安全跳过该项。

## owner-bound helpers

`helpers` 中以下 API 都要求 `&MountOwnerToken` 和
`ErrorHandlerInput<'scope>`：

- `window_event_listener` / `window_event_listener_untyped`；
- `request_animation_frame`、`request_idle_callback`、`queue_microtask`；
- `set_timeout`、`set_interval` 和 `debounce`。

它们返回 `HostResource<'scope>` 或 owner-bound `FnMut`。返回值可以
被调用方提前调用 `cancel()`；一次性资源完成时应调用 `finish()`，避免再执行
物理取消。创建成功后资源立即登记到 owner registry，registry 保存私有 lease
并在 cleanup 时调用一次内部 `cancel_once`。`HostResource` 没有 `Clone` 或
`Drop` 隐式取消语义，因此资源何时结束不会由公开值的复制或丢弃决定；重复
`cancel()` 是幂等的，物理取消最多执行一次。

有一个平台差异：`queue_microtask` 在浏览器中不能物理取消已经排队的
microtask；owner gate 只保证任务执行到 destination 后不再调用用户代码。
timer、interval、animation frame 和 idle callback 则会在 cleanup 时调用
对应的 clear/cancel API。

## `OwnedTimeout` 与一次性定时器

`view::OwnedTimeout<'scope>` 是 `set_timeout` 的 owner-bound 封装，专门表示一
个只执行一次的浏览器 timeout。`OwnedTimeout::schedule` 接收
`&MountOwnerToken<'scope>`、返回 `SilexResult<()>` 的一次性 task、
`Duration` 和 `ErrorHandlerInput<'scope>`；创建成功后，timeout、JS callback、
completion destination 和 cleanup lease 都属于该 owner。

它提供三个生命周期操作：

- `cancel()` 取消尚未执行的 timeout，并把 close 失败返回为 `SilexError`；重复
  cancel 是安全的；
- `finish()` 把一次性资源标记为已完成，不执行物理 clear；timeout callback
  会通过 once guard 消费用户 task 并关闭 callback gate，领取 callback 的状态机
  仍应调用 `finish()` 标记返回的 resource 已完成；
- `ticket()` 返回 `OwnedTimeoutTicket`，`is_current(ticket)` 同时检查 ticket
  是否匹配和资源是否仍 active，适合防止旧 timer callback 触碰新 request。

`OwnedTimeout` 不是跨 owner 的调度器，也不提供线程安全。owner close 会通过
注册表取消它；owner 已 inactive 时不能用新的 token 调度 timeout。回调执行前
会经过 host callback gate，因此 owner 已关闭时不会再调用用户 task；回调返回的
业务/运行时错误由传入的 error handler 处理。

`silex_persist` 的 `PersistWriteMode::Debounced` 使用这个类型保存待提交的
最新 raw：新 mutation 先取消旧 timeout，owner close、`flush`、`reload`、`remove`
和外部快照也会取消它。维护任何依赖 `OwnedTimeout` 的状态机时，必须同时保证
“旧 ticket 不能提交新状态”“创建/取消失败可报告”“owner cleanup 不重复物理
clear”三个条件。

## detached helpers

`helpers::detached` 是故意不绑定 owner 的另一组 API，适合应用级、页面级
或由外部生命周期管理的 callback。它们返回：

- `WindowListenerHandle`：drop/remove 会移除 listener，`forget` 则放弃
  自动清理；
- `AnimationFrameRequestHandle`、`IdleCallbackHandle`、`TimeoutHandle`、
  `IntervalHandle`：通过 `cancel`/`clear` 操作浏览器 id；
- 无 handle 的 convenience function：调用方不再拥有取消入口，适合明确
  的 page-lifetime 一次性工作。

detached callback 必须是 `'static`，因为它不借用 mount scope。不要在组件
内部用它代替 owner-bound helper，否则组件卸载后 callback 可能继续读取
失效 state 或保持 DOM/闭包存活。

## window/document 与错误选择

`try_window()`、`try_document()` 返回 `Option`，适合测试、SSR-like native
代码或浏览器对象可能尚未建立的初始化阶段。`window()`、`document()` 会
`expect`，只应在已经确认浏览器环境存在的应用入口使用。

事件 helper 也有两种错误策略：

- `event_target_value_result` 将缺少 target、target 类型不支持等情况返回
  `SilexErrorKind::Dom`；
- `event_target_value` 在这些情况返回空字符串；
- `event_target_checked` 在缺少或非 input target 时返回 `false`；
- `event_target<T>` 使用 unchecked cast，调用者必须保证 `T` 与真实 target
  类型匹配；不匹配会导致 wasm/JavaScript 边界上的无效行为。

表单逻辑、数据提交和错误恢复应优先使用带 `Result` 的 helper，不要把
“无法取得 target”当成用户输入为空。

## 对应测试

- `tests/host_resources.rs`：元素/window listener 的物理移除、callback
  drop、owner dispose、重渲染替换、重复取消和 panic 后 gate 状态；其中
  `render_rerun_replaces_old_window_listener` 验证 rerun 的 add/remove 数量，
  `window_listener_cancel_is_idempotent_and_owner_keeps_final_control` 验证
  显式取消与 owner 最终控制不会重复移除资源。
- `tests/owner.rs`：初始/deferred/cleanup error 的分发、动态 row/branch
  关闭和 runtime owner 清理。
- `tests/ui/fail_detached_host_callback.rs`、
  `fail_scoped_host_callback.rs`、`fail_cleanup_sink_scope.rs`：host callback
  与报告 sink 的 lifetime 边界。

修改 owner close 顺序、callback dispatch 或 host resource cancel 时，必须
同时检查“关闭后用户 closure 不再执行”“JS 资源已释放”“cleanup error
仍可报告”这三个条件。
