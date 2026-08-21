+++
title = "测试与调试"
description = "silex_net 的 native、browser、UI 编译期契约和文档示例验证方法。"
weight = 40
+++

# 测试与调试

`silex_net` 的行为横跨 owner lifetime、响应式 source、浏览器 host callback、
Fetch abort、connection retry 和可选持久 cache。只验证一次请求返回成功，不能
证明旧 operation、scope cleanup 或凭据筛选是安全的。测试应按运行环境和契约分层。

## 测试分层

| 位置 | 覆盖内容 | 环境 |
| --- | --- | --- |
| `src/lib.rs`、`src/state.rs`、`src/operation.rs`、`src/builder/**` 单元测试 | retry predicate、cache key、安全筛选、resolver、operation id 和 builder resolve | native 为主 |
| `tests/resolver.rs` | 跨 runtime source 在 resource 创建前失败，以及失败时目标 runtime 不泄漏节点 | native |
| `tests/connections.rs` | 跨 runtime URL/source connection 被拒绝 | native |
| `tests/browser.rs` | Fetch、Resource/Mutation、cache policy、WebSocket/EventSource mock host、retry、迟到事件和 cleanup | wasm browser |
| `tests/compile_fail.rs` + `tests/ui/` | builder/connection lifetime、借用 handler、foreign source、feature API 和 static transport | native trybuild |
| `tests/docs_examples.rs` + `docs/examples/silex_net/basic.rs` | 公开 builder/request model 的文档示例 | native |

browser 测试使用 `wasm_bindgen_test_configure!(run_in_browser)`，并通过 JS mock
替换 `WebSocket`、`EventSource` 等 host constructor；native `cargo test` 不会执行
这些 case。UI 测试中的 compile-fail stderr 是生命周期和 feature 契约的一部分，
修改公开 lifetime 或错误类型后应一起更新。

## 常用验证命令

仓库根目录下，文档和目标 crate 的最小检查为：

```text
RUSTFLAGS=-D warnings cargo fmt --all -- --check
RUSTFLAGS=-D warnings cargo check -p silex_net
RUSTFLAGS=-D warnings cargo test -p silex_net --test docs_examples
zola --root docs check
```

新增或修改 `docs/examples/silex_net/basic.rs` 后，至少执行
`cargo test -p silex_net --test docs_examples`；不需要为了文档示例执行整个
workspace 或其它 crate 的测试。涉及 browser host 时，再只编译目标测试：

```text
RUSTFLAGS=-D warnings cargo test -p silex_net --test browser \
    --target wasm32-unknown-unknown --no-run
```

若环境配置了 wasm-bindgen test runner 和浏览器，去掉 `--no-run` 才会真正运行
browser case。启用 feature 时可分别检查 `json`、`persist` 和组合配置，避免只
验证默认 feature 导致条件编译分支失真。

## 请求与 operation 契约

修改 HTTP builder、Resource 或 Mutation 时至少检查：

- path/query 编码、fragment 插入位置、大小写不敏感的 header 替换和 dynamic
  resolver 的 tracked/untracked 行为；
- 非 2xx 进入 `HttpStatus`，decode/serialize 失败不被错误地 retry；
- retry 次数是初始请求之外的次数，max delay/max elapsed 和 jitter 不突破配置；
- source 或 mutate 替换后，迟到响应不能覆盖最新状态；
- owner close 会关闭 operation controller，并让挂起 future 的 completion 失效；
- foreign runtime source 在创建 target resource 前失败，且 runtime snapshot 不变。

## 连接与 host cleanup 契约

修改 WebSocket/EventStream 时至少检查：

- lazy/open 的初始 state、非 active reconnect no-op、manual close 不触发 retry；
- WebSocket 只有 OPEN 才允许 send，EventStream 的 named event 和
  `max_messages` 保持正确顺序；
- constructor error 在没有成功 host registration 时仍调用一次 `on_error`；
- operation id 会忽略旧 socket/source 的 open、message、error、close；
- owner close 会移除所有 host callback、关闭 registration、取消 completion 和
  retry task，迟到事件不会调用用户 callback；
- callback error、completion error 和 close error 不会互相覆盖，error handler
  仍收到每个结构化错误。

## Cache 契约

启用 `persist` 时，browser 测试还应覆盖：

- only GET/HEAD + `CredentialsMode::Omit` + 无敏感 header/query/body 才建立 cache；
- 自定义 transport 默认不能启用 persistent cache，显式 opt-in 后才可使用；
- CacheFirst、NetworkFirst 和 StaleWhileRevalidate 的网络调用次数和 fallback
  结果分别正确；
- cache generation、operation guard 和 owner close 不允许 stale response 写回；
- capacity、TTL、坏 raw decode 和 eviction 不会留下与内存 snapshot 矛盾的状态。

## 调试顺序

1. 先确认句柄和请求 builder 是否仍在创建它的 owner scope 内；
   `NetErrorKind::Core`/`NoSuchNode` 通常表示 scope 已关闭或 runtime 不匹配。
2. 对 HTTP 记录最终 `RequestSpec` 的 method、URL、credentials、body 类型和
   retry policy；不要把 `Authorization` 或 cookie 原文写入日志。
3. 区分 `NetError::Recoverable` 与 `Fatal`，再检查 `kind()`；网络 transient、
   HTTP 状态、decode、配置和 core 错误的恢复动作不同。
4. 对 resource/mutation 比较 operation 顺序、`ResourceState`/`MutationState` 和
   transport 调用次数；若看到旧值回写，优先检查 source 替换和 guard，而不是先
   放宽 lifetime。
5. 对 connection 检查 state、registration 是否存在、retry task 是否活动，以及
   mock/浏览器的 callback property 是否已清空；关闭后出现用户 callback 通常是
   gate 或 completion cancel 顺序错误。
6. 对 cache 检查 `RequestSpec::is_persistent_cache_safe()`、transport opt-in、
   cache policy、generation 和 localStorage raw；不要把“跳过缓存”误判为网络失败。

## 对应测试索引

- `tests/browser.rs`：所有浏览器异步和 host registration 行为。
- `tests/resolver.rs`、`tests/connections.rs`：跨 runtime 和 transactional setup。
- `tests/compile_fail.rs`、`tests/ui/`：owner/lifetime、handler、feature 和 API 迁移契约。
- `tests/docs_examples.rs`、`docs/examples/silex_net/basic.rs`：可执行开发者文档。
- `src/state.rs` 与 `src/operation.rs` 的单元测试：cache key、retry 和 operation
  controller 的纯逻辑边界。
