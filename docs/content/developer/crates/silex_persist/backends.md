+++
title = "后端、订阅与外部同步"
description = "silex_persist 的 PersistenceBackend、Web Storage、Query backend 和订阅清理契约。"
weight = 20
+++

# 后端、订阅与外部同步

`silex_persist` 的 backend 只处理字符串和 key，不知道 `T` 的业务类型。
`PersistentBuilder` 在 backend 之上安装 codec；这样 backend 可以复用在字符串、
数字、JSON 或自定义值之间，而不会把序列化策略混入浏览器资源管理。

## `PersistenceBackend` 契约

backend 必须是 `Clone + 'scope`，并实现三个同步操作：

```rust
pub trait PersistenceBackend<'scope>: Clone + 'scope {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError>;
    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError>;
    fn remove(&self, key: &str) -> Result<(), PersistenceError>;

    fn subscribe(
        &self,
        owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>>;
}
```

这是 API 形状片段，用于说明参数和所有权，不是 `docs/examples/` 中的独立 CI
示例。公开操作返回 `Result`；backend 不应把暂时不可用伪装成空值，也不应在
错误路径中用 `unwrap` 代替错误报告。

实现时应遵守以下不变量：

- `get` 返回 `Ok(None)` 表示 key 不存在；这与 `BackendUnavailable` 或
  `ReadFailed` 不同，builder 会据此决定是否使用默认值和执行 bootstrap write。
- `set`/`remove` 的失败应使用 `WriteFailed`/`RemoveFailed` 等合适的
  `PersistenceErrorKind`，使绑定能进入 `WriteError` 并保留显式 `flush` 重试路径。
- `subscribe` 创建的所有 host 资源、listener 或响应式 effect 必须由返回的
  `BackendSubscription` 管理。即使创建订阅最终失败，也必须把已经创建的资源
  放进 cleanup token。
- `BackendSubscription::cleanup` 和 `Drop` 都只执行一次 cleanup；调用方可以
  提前 cleanup，但 owner cleanup 仍会再次 drop 它。
- subscription 的 `sink` 只能发送 `BackendEvent`；需要把 backend 错误异步交回
  owner 时，应使用 `BackendSubscribeError` 或内部错误 sink，而不是直接调用
  带 scope 的用户回调。

## 内置 backend

| backend | 数据源 | 外部同步 | 注意事项 |
| --- | --- | --- | --- |
| `LocalStorageBackend` | `window.localStorage` | `PersistExternalSync::StorageEvents` | 浏览器 profile 明文存储；同步读写。 |
| `SessionStorageBackend` | `window.sessionStorage` | `PersistExternalSync::StorageEvents` | 生命周期通常短于 local storage，但同样不是密文存储。 |
| `QueryBackend<'scope>` | `RouterContext::query_map` 与 `Navigator` | `PersistExternalSync::QueryChanges` | 值进入 URL，并通过 router history API 更新地址。 |
| 自定义 `PersistenceBackend` | 调用方提供 | 由 `subscribe` 自定义 | 必须满足 owner、scope 和 cleanup 契约。 |

`WebStorageBackend<true>` 和 `WebStorageBackend<false>` 是同一个 const-generic
实现的两个类型别名。构造时如果没有可用的 browser storage，backend 保留不可用
状态；后续操作返回 recoverable `BackendUnavailable`，而不是在构造阶段 panic。

`QueryBackend::new(ctx)` 从 `RouterContext` 取得 navigator 和解析后的 query map。
`set` 会更新或删除单个 query key，`remove` 删除 key；router 的 query 变化通过
owner effect 发送 `Set` 或 `Removed`。需要构造一个明确不可用的 backend 时，
可以使用 `QueryBackend::unavailable()`，这主要用于环境检测和测试。

## `BackendEvent` 与同步选择

```text
browser storage / router query / custom source
                    │
                    ▼
       BackendEvent::Set / Removed / ExternalRefresh
                    │
                    ▼
       owner CompletionSender<'scope>
                    │
                    ▼
       key check → cancel pending write → decode/remove policy
                    │
                    ▼
            Persistent value + state
```

`PersistExternalSync` 的选择决定 builder 是否调用 `subscribe`：

- `Disabled` 不创建订阅。适用于单向读取、测试，或业务已经有自己的冲突处理；
- `StorageEvents` 订阅 Web Storage event；
- `QueryChanges` 订阅 `QueryBackend` 的响应式 query map。

事件到达后先检查 key。非当前 key 的 `Set`/`Removed` 会被忽略；
`ExternalRefresh` 不携带 key，会触发当前绑定重新调用 backend `get`。有效外部
快照会取消待执行的 debounce timer，清除 local mutation 标记，并成为新的
`last_backend_raw` 基线。它不会再次触发本地写入。

外部 raw 解码成功时，绑定更新 `value` 和 `PersistenceState::Ready(raw)`；
解码失败时按照 `DecodePolicy` 保留或删除原始值，并使用默认值。外部删除按照
`RemovePolicy` 使用默认值或仅清空持久化基线。移除/解码的后续 backend 失败会
进入 `WriteError`，调用方仍应处理 `reload`/`flush` 返回的 `Result`。

## 订阅失败与 rollback

`BackendSubscribeError<'scope>` 同时携带主错误和 `BackendSubscription` cleanup
token。backend 在订阅阶段已经注册资源、但之后发生错误时，使用
`BackendSubscribeError::with_cleanup(error, cleanup)`；已有 subscription 在
转为错误时使用 `subscription.into_error(error)`。调用方取出错误时调用
`into_error()`，它会先执行 cleanup，再返回 `PersistenceError`。

builder 对订阅失败的处理是分层的：backend 不可用会保留 binding，但状态为
`Unavailable` 或后续可观察错误；fatal configuration error 会 rollback
binding 初始化并返回错误；其它订阅错误会保留 binding，并把消息写入
`WriteError`。维护 backend 时必须测试“错误返回前资源被释放”和“owner close
后订阅只 cleanup 一次”这两个路径。

## 明文与 URL 边界

Web Storage 和 Query backend 都把 codec 产出的字符串暴露给宿主环境：前者可被
同源脚本读取，后者会出现在 URL、历史记录、复制链接、日志或 referrer 相关
边界。它们不适合密码、token、个人敏感资料或其它长期凭据。`PersistCodec` 只
负责表示转换，不等于加密；需要保密、完整性或服务端授权时，应使用专门的
安全存储/服务端方案，并把密钥和失效策略放在本 crate 之外管理。

## 对应测试

- `crates/silex_persist/tests/builder.rs`：自定义 mock backend、事件 sink、
  subscription rollback、外部快照和 scope cleanup。
- `crates/silex_persist/tests/browser.rs`：local/session storage event、query
  history、不可用 backend、listener 物理移除和浏览器 timer。
- `crates/silex_persist/src/backend.rs`：Storage hub listener 状态机、订阅
  数量、generation 和 reentrant cleanup 的 native 单元测试。
- `crates/silex_persist/tests/ui/`：backend callback、static sink、view 和
  builder lifetime 不能逃逸当前 scope 的编译期契约。

修改订阅或外部同步时，至少检查：事件不会进入错误 key；owner 关闭后不会调用
用户 closure；listener、completion endpoint 和 timer 的失败不会丢失；外部快照
不会让旧的本地 request 再次提交。
