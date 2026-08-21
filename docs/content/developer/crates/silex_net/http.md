+++
title = "HTTP builder、请求状态与传输"
description = "silex_net 的请求构造、响应式 source、Resource/Mutation、codec、重试和自定义 transport。"
weight = 20
+++

# HTTP builder、请求状态与传输

HTTP API 的核心是 `HttpClientBuilder<'scope, T, C, H>`：`T` 是解码后的结果，
`C` 是 `ResponseCodec<T>`，`H` 是 `ErrorHandlerInput<'scope>`。builder 保存
owner-bound 的 `ValueResolver`，在真正发送、创建 resource 或触发 mutation 时
解析出 `RequestSpec`。这种延迟解析让同一个 builder 可以依赖响应式 query、header
或 path parameter，同时又让 lifetime 防止它逃离 owner。

## 创建 builder

`HttpClient` 提供五个默认使用 `TextCodec` 的快捷入口：`get`、`post`、`put`、
`patch` 和 `delete`。通用入口是 `builder(scope, method, url, handler)`；需要
自定义响应类型时使用 `builder_with_codec`。

```rust
// 这是 API 形状示意；handler、scope 和 `query` 由调用方的 owner 提供。
let builder = HttpClient::get(scope, "/api/items/{id}", handler)
    .path_param("id", item_id)
    .query("filter", query)
    .header("Accept", "text/plain")
    .timeout(Duration::from_secs(5));
```

builder 的常用配置如下：

| 方法 | 行为 |
| --- | --- |
| `header` / `headers_pairs` / `header_opt` | header 名大小写不敏感地替换已有同名项；动态 value 在 resolve 时读取。 |
| `query` / `query_pairs` / `query_opt` | 追加 query 项；值经过 URI component 编码，并插入 fragment 之前。 |
| `path_param` | 将 URL 中的 `{key}` 替换为编码后的 value；未匹配的占位符不会被额外处理。 |
| `timeout` / `timeout_ms` | 给 `RequestSpec` 设置 timeout；Fetch 用 `AbortController` 实现。 |
| `credentials` | 设置 `omit`、`same-origin` 或 `include`，默认是 `SameOrigin`。 |
| `text_body` / `form_body` / `body` | 分别创建动态文本、表单或静态 `RequestBody`；form 会默认设置 `Content-Type`。 |
| `bearer_auth` / `basic_auth` | 根据静态或响应式输入生成 `Authorization` header。 |
| `intercept` | 在发送前修改完整 `RequestSpec`；适合统一加 header 或调整请求。 |
| `transport` | 替换默认 `HttpBackend`；值被放进 `Rc<dyn Transport>`，transport 必须是 `'static`。 |
| `on_response` / `on_error` | 分别观察成功响应和每次传输、HTTP 状态或 decode 失败。 |
| `on_retry` | 在下一次尝试前收到 request、attempt、delay 和 error。 |

请求字段由 `IntoNetValue<'scope>` 接收。它支持 `&str`、`String`、数字、`bool`、
浮点数、`ValueResolver` 以及 `Rx`、`ReadSignal`、`RwSignal`、`Signal`、`Computed`；
启用 `persist` 后还支持 `Persistent`。`Fn() -> T + 'static` 也可作为动态值，
但若 closure 直接捕获响应式句柄，应优先把句柄本身传给 builder，使 runtime 能
验证其归属并在 resource 中建立 tracked 依赖。

## tracked 与 untracked 请求解析

同一组 resolver 有两种读取模式：

```text
resource source / computed ── resolve_tracked ──┐
                                               │ RequestSpec
send / mutation execution ── resolve ──────────┘
```

- `send()`、`as_mutation()` 和 `mutate` 的实际请求使用 untracked 读取，避免把
  一次命令式调用本身变成订阅。
- `as_resource(source, suspense)` 先为请求字段创建 tracked computed，再把外部
  `source` 与请求 spec 合并。URL、query、header、body 任一响应式值变化都会替换
  当前请求；`into_resource` 使用一个恒定 source，适用于只依赖请求字段的资源。
- source 必须和目标 owner 属于同一个 runtime。跨 runtime 的 reactive source 在
  创建资源前返回 fatal `NetErrorKind::Core`，不会留下半初始化的 resource。

## 发送、Resource 与 Mutation

### 直接发送

`send()` 返回 `Future<Output = Result<T, NetError>>`。默认 `HttpBackend` 使用
浏览器 Fetch，Fetch 成功后先要求 `HttpResponse::ok()`（200–299），再交给 codec
解码；非 2xx 直接变为 `HttpStatus`，不会调用 response codec。

```rust
// 这是需要浏览器 async 上下文的 API 形状示意，不是 docs_examples 源码。
let value = HttpClient::get(scope, "/api/message", handler)
    .send()
    .await?;
```

`BrowserTransport::send` 和 `HttpBackend::send` 都返回完整的
`HttpResponse { url, status, status_text, raw_body }`。实现 `Transport` 可以在
测试或特殊 host 中替换它：`send` 必须返回
`TransportFuture<'_>`，并且只有确认没有隐藏凭据时才应覆盖
`supports_persistent_cache()` 返回 `true`。

### Resource

`into_resource(None)` 返回 `Resource<'scope, T, NetError>`；需要外部触发值时使用
`as_resource(source, suspense)`。它把 core 的资源状态和网络 operation 绑定起来：

| `ResourceState` | HTTP 语义 |
| --- | --- |
| `Idle` / `Loading` | 首次请求尚未完成；初始化 effect 通常很快进入 `Loading`。 |
| `Ready(T)` | 当前 operation 成功并已 decode。 |
| `Reloading(T)` | source 改变后请求进行中，但保留上一次成功值。 |
| `Error(NetError)` | 当前 operation 失败；可根据错误类型决定界面或重试。 |

source 变化会生成新的 operation id。旧请求即使之后完成，也只能返回
`Aborted`/被丢弃，不能覆盖新 resource 状态。owner close 会关闭 operation controller
并让挂起的 future 失效；这和 HTTP 服务端是否已经收到请求是两件事，客户端 abort
不代表服务端事务回滚。

### Mutation

`as_mutation()` 返回 `Mutation<'scope, (), T, NetError>`，每次 `mutate(())` 都会
执行同一个请求。需要由输入决定 URL、body 或其它字段时，使用
`as_mutation_with(factory)`，factory 返回当前输入对应的 builder。prepare 阶段的
runtime 校验、请求解析或序列化失败会直接进入 `MutationState::Error`，不会先发布
`Pending`；成功创建 future 后才进入 pending 流程。

连续 mutate 时，最近一次 operation 才能提交 `Success` 或 `Error`。旧请求不会因为
网络响应顺序反转而覆盖新状态，也不表示远端操作已经取消；需要服务端幂等键或
撤销语义时，必须在请求协议中另行设计。

## Codec 与 JSON

`ResponseCodec<T>` 只有一个要求：`Clone + 'static`，并把 raw body 转为
`Result<T, NetError>`。`TextCodec` 原样复制文本。启用 `json` 后：

- `NetJsonCodec<T>::new()` 通过 `serde_json::from_str` 解码，失败为可恢复的
  `NetErrorKind::DecodeError`；
- `builder.json::<T>()` 将响应类型切换为 `T`；
- `json_body(value)` 在构造时序列化静态值，失败立即返回
  `NetErrorKind::SerializeError`；
- `json_body_value(value)` 在请求 resolve 时序列化动态 JSON 字符串；
- WebSocket 的 `message::<T>()` / `send_json` 和 EventStream 的 typed message
  API 复用相同的 JSON decode/encode 边界。

启用 `persist` 时，想让同一类型参与 HTTP cache，codec 还必须实现
`PersistCodec<T>`，也就是满足 `CacheCodec<T>`。响应 decode 成功后才会向 cache
提交值；decode 失败不会污染缓存。

## Retry 与错误处理

`RetryPolicy::new(max_retries, delay)` 的 `max_retries` 只表示初始请求之后的
重试次数。`delay_for_attempt` 使用指数退避：attempt 1 是基础 delay，之后每次
翻倍，可用 `max_delay` 限制；默认启用随机抖动，测试或需要确定时序时使用
`no_jitter`。`max_elapsed` 限制从 retry window 开始到下一次尝试的总时间。

默认 policy 是 `max_retries = 1`、零 delay、带 jitter；零 delay 下抖动仍为零。
只有下列错误可重试：

- `Timeout`；
- `TransportUnavailable`；
- HTTP 状态 408、429 或 500–599。

`Aborted`、`DecodeError`、`SerializeError`、`InvalidConfiguration`、连接状态错误
和其它 JS 错误不会自动 retry。每次失败都会触发 `on_error`，真正安排下一次
尝试前触发 `on_retry`；重试耗尽后，若 `NetworkFirst` 有 cache fallback 才会回退
到缓存，否则返回最后一个网络错误。

所有 `send`、builder 转换和连接控制操作的 `Result` 都应传播或分类处理。不要
用 `unwrap`/`expect` 把错误路径隐藏在应用代码中；错误 handler 主要用于异步
completion、响应式 effect 和 cleanup 报告，并不替代公开方法的返回值。
