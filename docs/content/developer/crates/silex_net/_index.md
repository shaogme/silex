+++
title = "silex_net"
description = "Silex 面向浏览器 Fetch、WebSocket、EventSource 和响应式请求状态的网络 facade。"
template = "section.html"
sort_by = "weight"
+++

# `silex_net`

`silex_net` 把浏览器网络 API 接入 Silex 的 owner、响应式状态和错误模型。
HTTP 请求可以直接 `send`，也可以转换为 `silex_core::Resource` 或
`Mutation`；WebSocket 和 EventSource 则把浏览器 host registration、消息状态、
回调与连接清理绑定到同一个 `OwnerAccess`。因此它位于应用请求层和
`silex_core` 生命周期 facade 之间，底层 host 资源由 `wasm-bindgen`/`web-sys`
负责。

## 在 Silex 架构中的位置

```text
组件 / 页面 / 交互事件
          │  OwnerAccess<'scope>
          ▼
      silex_net
  HttpClientBuilder ── Resource / Mutation
  WebSocketConnection / EventStreamConnection
          │
          ├── silex_core::Resource、Mutation、Completion
          ├── NetError / ErrorHandler
          └── browser Fetch / WebSocket / EventSource
```

crate 不提供全局 client、线程池或跨线程连接。公开句柄带有 owner 的 Rust
lifetime；owner 关闭后，异步 completion、浏览器回调和重连任务都会失效或被
清理。应用应把请求和连接放在组件对应的持久 owner 或 transient scope 中，
不能把它们提升为 `'static`。

## 稳定入口与核心类型

| 入口 | 用途 |
| --- | --- |
| `HttpClient` / `HttpClientBuilder` | 创建带 URL、query、header、body、retry 和 codec 的 HTTP 请求。 |
| `ValueResolver` / `IntoNetValue` | 将静态值、响应式值或动态 closure 接入请求字段。 |
| `Transport` / `BrowserTransport` | 抽象请求发送；默认实现使用浏览器 Fetch。 |
| `ResponseCodec` / `TextCodec` | 将响应文本转换为应用类型；`json` feature 提供 `NetJsonCodec`。 |
| `RequestSpec` / `HttpResponse` | 传输层使用的完整请求和响应模型。 |
| `Resource` / `Mutation` | 分别表达由 source 驱动的读取和显式触发的变更。 |
| `WebSocket` / `WebSocketConnection` | 管理双向文本消息、连接状态、发送和可选重连。 |
| `EventStream` / `EventStreamConnection` | 管理 EventSource 消息缓冲、命名事件和显式重连。 |
| `NetError` / `NetErrorKind` | 区分可恢复错误、致命配置错误、HTTP 状态和解码错误。 |

应用通常还需要从 `silex_core` 导入 `Runtime`、`OwnerAccess` 和
`ErrorHandlerToken`。builder 和 connection 的 error handler 可以是拥有者内的
token，也可以是借用的 `&ErrorHandlerToken`；后者只要仍在当前 scope 内即可。

## Feature flags

crate 默认不启用可选 feature，默认响应 codec 是 `TextCodec`。

| Feature | 公开内容 |
| --- | --- |
| `json` | `NetJsonCodec`、JSON request body、WebSocket/EventStream 的 typed message API；依赖 `serde` 与 `serde_json`。 |
| `persist` | `HttpCache`、`CacheCodec`、`CacheConfig`、`CacheEviction` 与 `CachePolicy` 的持久缓存接入；依赖 `silex_persist`。 |
| `json` + `persist` | `NetJsonCodec` 同时实现响应 codec 和持久缓存 codec，JSON 响应可参与 HTTP cache。 |

`persist` 不会让所有请求自动写入 `localStorage`。请求还必须满足匿名、无敏感
信息的持久化缓存安全条件，且 transport 必须显式声明支持；详见
[持久缓存与安全边界](cache.md)。

## 最小可验证流程

下面的源文件是 `docs/examples/silex_net/basic.rs`，页面没有复制另一份代码。
它在 native 测试中创建 owner-bound builder、解析请求字段，并验证 request model
和 retry policy。实际发送在浏览器中 await `builder.send()`，或使用
`into_resource`/`as_mutation` 将 future 交给 core 的 owner-bound 异步状态。

{% set source = load_data(path="examples/silex_net/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

对应测试是 `crates/silex_net/tests/docs_examples.rs`。示例不调用浏览器 host，
所以可以在 native 环境完成编译和同步断言；Fetch、WebSocket 和 EventSource 的
host 行为由 browser 测试覆盖。

## 生命周期与并发边界

一次请求或连接的关键边界如下：

```text
OwnerAccess<'scope>
├── ValueResolver / builder / connection handle
├── core signal、Resource、Mutation、completion endpoint
└── browser registration / scoped future / retry task
        │ owner close
        ▼
   cancel、invalidate、remove host callbacks、drop registration
```

- `ValueResolver` 对响应式输入保留 `'scope`。resource 的 tracked resolver 会让
  URL、query、header 和 body 成为请求 source；普通 `send()` 使用 untracked 读取，
  因此一次 `send` 本身不会因为读取请求字段而建立响应式订阅。
- 每个 HTTP resource/mutation 和每次连接建立都有 operation id。新操作或 owner
  cleanup 会使旧 id 失效；迟到的 HTTP、WebSocket 或 EventSource 事件不能覆盖新
  状态。
- `TransportFuture` 是 owner 侧 future，不保证 `Send`。内置实现使用浏览器
  Fetch；scope 清理时由 future drop 触发 `AbortController.abort()`，超时也会
  通过同一 controller 中止请求。
- host registration 自己持有 JS callback 的 `Closure`。`Drop` 会先关闭 gate、
  移除 callback，再关闭 socket/source；不要在 owner 外保存连接句柄或回调。
- `NetError` 返回值和 error handler 是两条边界：公开操作的 `Result` 必须处理，
  延迟 completion 或 handler/cleanup 的错误则报告到 `ErrorHandlerInput`。

## 专题

- [HTTP builder、请求状态与传输](http.md)：请求字段解析、codec、Resource/Mutation、retry 和自定义 transport。
- [WebSocket 与 EventStream 连接](connections.md)：状态机、消息读取、回调、重连和 host cleanup。
- [持久缓存与安全边界](cache.md)：`persist` feature、cache policy、key、淘汰和敏感请求筛选。
- [测试与调试](testing.md)：native、browser、UI 编译期契约和文档示例验证。

## 源码、示例与测试索引

- facade 与 feature re-export：`crates/silex_net/src/lib.rs`
- 请求模型与 retry：`crates/silex_net/src/state.rs`
- builder、resolver 与 cache：`crates/silex_net/src/builder.rs`、`src/builder/`
- Fetch 与 transport trait：`crates/silex_net/src/backend/fetch.rs`
- WebSocket：`crates/silex_net/src/backend/websocket.rs`
- EventSource：`crates/silex_net/src/backend/event_stream.rs`
- operation 失效与关闭：`crates/silex_net/src/operation.rs`
- 文档示例：`docs/examples/silex_net/basic.rs`
- 文档示例测试：`crates/silex_net/tests/docs_examples.rs`
- native resolver/connection 契约：`crates/silex_net/tests/resolver.rs`、`tests/connections.rs`
- browser host 与异步状态：`crates/silex_net/tests/browser.rs`
- lifetime、feature 和静态 transport 契约：`crates/silex_net/tests/compile_fail.rs`、`tests/ui/`

## 已知限制与维护注意

- crate 当前是浏览器导向的 wasm facade。native 测试可以验证 builder、request
  model、resolver 和 operation 逻辑，但不能把 `BrowserTransport` 当作 native HTTP
  client；浏览器 host 行为必须在 wasm/browser 测试中验证。
- 默认 builder 使用 `TextCodec` 将 `HttpResponse.raw_body` 转为字符串；内置
  `HttpBackend`/`BrowserTransport` 仍返回包含 URL、状态和 body 的完整
  `HttpResponse`。非 2xx 会在 client 层转换为 `NetErrorKind::HttpStatus`，不会交给
  response codec。
- 默认 retry policy 是 `RetryPolicy::new(1, Duration::ZERO)`，即初始请求之外最多
  再尝试一次；只有 timeout、transport unavailable 和 408、429、500–599 可重试。
  `Aborted`、decode、serialize 和配置错误不会被 retry。
- `RequestSpec::cache_key()` 是 SHA-256 标识，不是加密；`HttpCache` 的底层
  `localStorage` 仍是同源脚本可读的明文存储，不能用于 token、密码或个人敏感数据。
- `WebSocket` 的 retry 是 crate 管理的 owner-bound 重连；EventSource 保留浏览器
  自带的重新连接行为，crate 只提供显式 `reconnect()`，不能把两者的 retry 语义
  混写。

验证公开 API 或本页时，至少运行目标 crate 的 `cargo check`、文档示例测试和
`zola --root docs check`；修改 browser host 或 wasm lifetime 时再运行对应 browser
编译和 UI compile-fail 测试。
