+++
title = "持久缓存与安全边界"
description = "silex_net 的 persist feature、HttpCache、cache policy、请求 key、淘汰和敏感数据限制。"
weight = 35
+++

# 持久缓存与安全边界

启用 `silex_net/persist` 后，`HttpCache<'scope, T>` 为一组 HTTP builder 提供
owner-bound 的内存 LRU 状态和 `localStorage` 持久快照。它不是通用 HTTP cache，
也不绕过 `silex_persist` 的 owner/codec 契约：cache handle、codec 和完成 ticket
都必须属于创建它的 scope。

## 创建与绑定

`HttpCache::new(scope, config, codec)` 要求 `T: Clone + 'static`，codec 同时满足
`ResponseCodec<T>` 和 `PersistCodec<T>`，即 `CacheCodec<T>`。`CacheConfig` 的默认
值是 capacity 32、无 TTL、淘汰时删除持久值；可用以下 builder 方法修改：

- `capacity(n)`：限制内存 entry 数量；`0` 会禁用 entry；
- `ttl(duration)` / `without_ttl()`：按最后访问时间判断过期；
- `eviction(CacheEviction::RemovePersisted | KeepPersisted)`：控制内存淘汰时
  是否同时删除 localStorage raw。

在 HTTP builder 上调用 `.cache(policy, cache)`。cache scope 与 builder 不同时，
该方法返回 fatal `InvalidConfiguration`；这项检查必须保留，因为后续响应完成
需要使用同一 owner 的 codec、backend 和 completion。

```rust
// 这是 persist feature 下的 API 形状示意；实际 scope、handler 和 feature 配置省略。
let cache = HttpCache::new(scope, CacheConfig::default().capacity(16), TextCodec)?;
let builder = HttpClient::get(scope, "/api/items", handler)
    .credentials(CredentialsMode::Omit)
    .cache(CachePolicy::NetworkFirst, cache)?;
```

## Cache policy

| Policy | 有快照时 | 无快照或网络失败时 |
| --- | --- | --- |
| `None` | 不读写 cache。 | 只走网络。 |
| `CacheFirst` | 立即返回 snapshot，不发网络请求。 | 走网络；成功后写入 cache。 |
| `NetworkFirst` | 先走网络；请求最终失败时返回 snapshot。 | 走网络并把最终网络错误返回。 |
| `StaleWhileRevalidate` | 立即返回 snapshot，同时以 owner-scoped task 后台刷新并更新 cache。 | 走网络并写入 cache。 |

“网络最终失败”指 transport、可重试 HTTP 状态等经过 retry policy 后仍失败；
decode error 不会被伪装成 cache fallback。`Resource` 和 `Mutation` 也使用同一
policy，但 resource 的 stale refresh 只有当前 operation 仍有效时才更新 resource，
旧刷新结果会被 operation guard 丢弃。

## 请求身份与安全筛选

`RequestSpec::cache_key()` 以 `net-request-v2-` 加 SHA-256 digest 标识请求。身份
包含 method、规范化后的 query 顺序、fragment、URL、credentials、按小写 header
名排序后的 headers、timeout 和 body fingerprint；长度分段避免不同字段拼出同一
原始 key。key 不包含原始 secret/body，但 hash 不是加密。

即使配置了 cache，builder 也只有在以下条件全部满足时才建立 cache binding：

- method 是 `GET` 或 `HEAD`；
- `CredentialsMode::Omit`；
- URL 没有 userinfo，也没有被当前规则识别为 credential、token、secret、password、
  session、cookie、csrf、`key` 或 `*_key` 的 `name=value` query 参数；
- body 是 `RequestBody::Empty`；
- header 名和值没有认证、token、secret、password、cookie、session 等凭据内容；
- transport 的 `supports_persistent_cache()` 返回 `true`。

不安全请求不会返回错误，而是跳过 cache，继续按普通网络请求执行。自定义
transport 只有在确认不会隐式携带 cookie、authorization 或其它用户凭据时才可
返回 `true`；否则应保持默认 `false`。

这套筛选是防止把明显的凭据请求写入持久缓存的最小边界，不是数据分类器或加密
方案。`localStorage` 对同源 JavaScript 可读，URL 和 request header 也可能进入
日志、历史或宿主诊断边界；密码、token、个人资料和服务端授权响应不应放入
`HttpCache`。

## 读写与 generation

第一次访问某个 key 时，cache 从 localStorage decode snapshot 并加入内存 entry；
decode 失败会按 eviction 配置删除坏的 persisted raw。每次 binding 都分配新的
generation。成功 response 通过 `CompletionOnce<T>` encode 后写 localStorage，
只有 generation 仍然活动且 operation guard 仍 current 时，才更新内存 snapshot。

因此以下顺序不会让旧请求污染新值：

```text
request A -> generation A ───────── late completion ──┐
request B -> generation B -> current snapshot        │
                                                      └─ A 被 guard/generation 丢弃
```

owner close 会取消 completion；builder/resource/mutation 的 operation controller
也会使旧 response 失效。cache 的一致性保护不能撤销服务端已经完成的副作用，特别
是不要把 POST/PUT 强行改成 GET 来获得 cache。

## 与 `silex_persist` 的边界

`HttpCache` 内部使用 `LocalStorageBackend` 和 codec 的 `PersistCodec` 能力，但它
不是 `Persistent<T>` binding：应用不能用 `Persistent` 的 `get`、`set`、`reload`
或外部同步 API 直接操作 cache entry。需要用户可编辑设置、query state 或跨 tab
外部同步时，应使用 `silex_persist` 的正式 builder；需要 HTTP response snapshot
和上述 cache policy 时才使用 `HttpCache`。

cache 当前没有公开的手动 `flush`、单 key remove 或 cache-wide clear API；容量、TTL、
eviction 和 owner close 是公开的生命周期控制点。若业务需要主动失效，应改变
请求 key/响应式 source、重新创建 cache，或在 `silex_persist` 外层管理更明确的
版本号，而不是依赖 localStorage 的内部 key 格式。

## 维护风险

修改 cache key、safety predicate、generation 或 completion 时必须同时验证：

- query/header 顺序变化不会改变同一个安全请求的身份；
- Authorization、cookie、敏感 query 和 body 不会建立 persistent binding；
- cache-first 不会触发不必要的网络请求，stale refresh 不会覆盖新 operation；
- TTL、capacity=0 和 eviction 的 raw 删除语义一致；
- owner close、失败 response、decode 失败和迟到 completion 都不会泄漏或写回旧值。
