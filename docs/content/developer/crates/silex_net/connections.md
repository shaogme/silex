+++
title = "WebSocket 与 EventStream 连接"
description = "silex_net 的 WebSocket、EventSource 状态机、消息读取、重连和 owner 清理契约。"
weight = 30
+++

# WebSocket 与 EventStream 连接

`WebSocket` 和 `EventStream` 都是 owner-owned connection。builder 在 `build()`
时创建响应式状态、completion endpoint 和 host callback；返回的 connection 是
带 `'scope` 的能力句柄。句柄本身可以复制，但复制不会复制 socket/source，也不
会延长 owner 的生命周期。

## 共用状态机

两种 connection 都使用 `ConnectionState`：

| 状态 | 含义 |
| --- | --- |
| `Disconnected` | lazy builder 尚未连接，或当前没有 host registration。 |
| `Connecting` | 正在创建/等待当前 socket 或 EventSource 打开。 |
| `Connected` | host 已打开，可以读取消息；WebSocket 此时才允许发送。 |
| `Closing` | WebSocket 已请求关闭，等待 host close 事件；EventStream 的 close 会直接进入 `Closed`。 |
| `Closed` | 当前 registration 已关闭。 |
| `Error` | 最近一次 host、构造或消息处理失败。 |

`state()` 返回 `ReadSignal<ConnectionState>`，`is_connected`、`is_connecting` 和
`is_closed` 返回由该 signal 派生的 `Rx<bool>`；`state_str()`（WebSocket）返回
`Rx<&str>`。关闭或错误不会自动让句柄失效，但 owner close 后继续读写会通过 core
错误边界返回错误。

## WebSocket

`WebSocket::open(scope, url, handler)` 等价于默认 `auto_connect(true)` 的
`connect(...).build()`；`WebSocket::lazy` 将初始状态设为 `Disconnected`，需要
调用 `reconnect()` 才创建 socket。builder 还支持：

- `protocol` 添加 WebSocket subprotocol；
- `on_open`、`on_error`、`on_close` 注册 owner-bound callback；
- `reconnect` 或 `reconnect_policy` 配置 error/close 后的 owner-bound retry。

消息接口如下：

| API | 语义 |
| --- | --- |
| `raw_message()` | 最近一个文本消息；新连接不会自动清除旧值。 |
| `message::<T>()` | `json` feature 下把最近文本 decode 成 `Option<T>`。 |
| `send` / `send_text` | 发送文本；仅 `Connected` 且 host readyState 为 OPEN 时成功。 |
| `send_json` | `json` feature 下序列化后发送；序列化失败为 `SerializeError`。 |
| `error()` | 最近一个 typed `NetError`，成功 open 后会清除。 |

`send` 在 `Connecting`、`Closing` 或其它非 OPEN readyState 返回
`ConnectionNotReady { state }`；在 `Closed` 返回 `ConnectionClosed`。调用方应根据
这些 recoverable error 更新 UI 或等待 `on_open`，不要在连接尚未打开时静默丢弃
发送操作。

### WebSocket retry

WebSocket 的 retry window 在一次初始 `reconnect()` 或自动连接时开始。error 或
非手动 close 最多消费 policy 配置的重试次数；每次 retry 由 owner-scoped task
等待 `RetryPolicy` 的 delay，然后创建新的 socket。收到 open 后清除 error 并重置
retry window；手动 `close()` 不会安排 retry，下一次显式 `reconnect()` 会开始新的
window。新的 operation id 会过滤旧 socket 迟到的事件。

`close()` 在有 registration 时先将状态设为 `Closing` 并调用 host close；host
close event 到达后才进入 `Closed`。没有 registration 时直接进入 `Closed`。
`toggle()` 在 active 状态调用 close，否则调用 reconnect；对 active connection
调用 `reconnect()` 是 no-op。

## EventStream / EventSource

`EventStream::open` 默认立即创建 EventSource；`EventStream::lazy` 或
`builder(...).auto_connect(false)` 初始为 `Disconnected`。builder 的
`event(name)` 把消息监听挂到命名事件，未设置时使用普通 `message` 事件；
`max_messages(n)` 让内部 buffer 只保留最新 n 条，超出的旧消息从头部移除。

读取接口如下：

- `raw_messages()` 返回 `ReadSignal<Vec<EventMessage>>`，其中每条消息保留
  `event: Option<String>` 与 `data`；
- 启用 `json` 后，`messages::<T>()` 返回全部 buffer、`last_message::<T>()`
  返回最后一条，`latest_messages::<T>(limit)` 按最新到最旧返回最多 limit 条；
- `clear_messages()` 清空 buffer，但不关闭 EventSource；
- `error()` 返回最近的 connection error，成功 open 后会清除。

EventSource 保留浏览器自己的错误后 reconnect 行为；crate 不额外创建第二个 retry
队列。`on_error` 只报告本次 host error，应用需要明确重新开始时调用
`reconnect()`。与 WebSocket 一样，`reconnect()` 只在 `Closed`、`Disconnected`
或 `Error` 状态创建新的 registration，active 状态调用是 no-op。

## 回调与借用

回调可以借用当前 owner scope 内的数据，并在 callback 中使用 connection 的复制句柄，
因为 callback 只以 `'scope` 存在。典型模式是先用 `Rc<Cell<Option<...>>>` 保存
build 返回的句柄，再在 `on_open`/`on_close` 中取出；不过保存槽本身也必须由 owner
管理，且 callback 不应把句柄发送到其它线程。

host callback 不直接调用带 scope 的用户逻辑。它把事件送入 owner completion，
由 completion 检查 operation id 后更新 signal，再延迟执行 `on_open`、`on_error`
和 `on_close`。这样可以过滤旧 registration 事件，也能把 callback、handler 和
close error 分开报告。

## Cleanup 不变量

连接清理的顺序是重要契约：

```text
owner close / connection close
          │
          ├── gate = false，阻止迟到 JS 事件
          ├── cancel retry task / invalidate operation
          ├── 移除 onopen、onmessage、onerror、onclose
          ├── close WebSocket / EventSource
          └── cancel completion sender
```

`HostRegistration::Drop` 负责释放 JS `Closure` 和 host registration；inner 的
`Drop` 再负责 driver、retry task 和 completion。清理必须幂等：owner close 后再
调用 connection 的控制方法不应重新注册 host。修改 callback 或 completion 桥接
时，应同时验证“迟到事件不再调用用户 closure”和“关闭错误不会掩盖 callback 错误”。

## 相关错误

- 构造 URL、创建 host 失败通常返回 recoverable `JsError` 或 `TransportUnavailable`，
  builder 会先调用 `on_error`，再返回 `Err`；不会返回一个无 registration 的伪连接。
- connection 操作产生的 core signal、completion 或 handler 失败会映射为 fatal
  `NetErrorKind::Core`，应交给 error handler 和调用方共同处理。
- WebSocket/EventStream 的 JSON decode 是派生 `Rx` 的计算过程，decode 失败会
  进入 `NetErrorKind::DecodeError`，不会把坏消息转换成默认值。
