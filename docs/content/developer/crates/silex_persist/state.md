+++
title = "状态、builder 与 codec"
description = "silex_persist 的 typestate builder、初始化流程、写入模式、状态机和编解码。"
weight = 30
+++

# 状态、builder 与 codec

`PersistentBuilder` 用类型状态把“尚未选择 backend、codec 或默认值”的中间
状态从 `build` 入口排除。构建成功后，`Persistent<'scope, T>` 把响应式值和
持久化控制器放在同一个 owner 中；`T` 的读写不会绕过 codec 或写入策略。

## builder 的合法顺序

最小流程是：

```text
Persistent::builder(owner, key, error_handler)
        │
        ├─ .local() / .session() / .query(ctx) / .backend(custom)
        ├─ .string() / .parse::<T>() / .json::<T>() / .custom_codec(...)
        ├─ .default(value) / .default_with(...) / .optional()
        └─ .write_default(...) / .on_decode_error(...) / .on_remove(...)
           .write_mode(...) / .external_sync(...)
           .build()
```

`NoBackend`、`NoCodec` 和 `NoDefault` 是 typestate marker。`build` 只在 backend、
codec、默认值和 `ErrorHandlerInput<'scope>` 都满足约束时可调用；最终还要求
`T: Clone + PartialEq + 'scope`，因为值需要放进 signal、比较外部快照并在
状态转换中复制默认值。

`optional()` 是一个快捷组合：它把当前 codec 包成 `OptionCodec`，目标类型变为
`Option<T>`，默认值为 `None`，并使 `None` 通过 `should_remove` 删除 backend key。
`OptionCodec` 不允许直接 encode `None`；正常的 `Persistent` 写入路径会先检查
`should_remove`，因此 `None` 不会被错误地序列化成字符串。

## 初始化与默认值策略

`build` 先创建 owner-bound value、state 和 controller，再同步调用
`backend.get(key)`：

| backend/codec 结果 | value | state | 后端动作 |
| --- | --- | --- | --- |
| 找到且 decode 成功 | 解码后的值 | `Ready(raw)` | 无，除非 `WriteDefault::Always`。 |
| key 缺失 | 默认值 | `Ready("")` | `IfMissing`/`Always` 会执行 bootstrap write。 |
| backend 不可用 | 默认值 | `Unavailable` | 不把不可用误判为 key 缺失。 |
| 读取失败 | 默认值 | `ReadError(message)` | 不执行默认值写入。 |
| decode 失败 | 默认值 | `DecodeError { raw, message }` | `RemoveAndUseDefault` 尝试删除 raw。 |

`WriteDefault` 的语义如下：

- `Never`：初始化不把默认值写回后端；
- `IfMissing`：只有 `get` 返回 `Ok(None)` 时写入默认值；
- `Always`：缺失时写入默认值，已有值 decode 成功时也重新 encode 并写回，
  可用于规范化字符串表示。

bootstrap write 失败不会让已经创建的 binding 静默消失；状态进入
`WriteError`，后续可以在 backend 恢复后调用 `flush` 重试。任何 `build` 返回的
`PersistenceError` 都应由调用方传播或分类处理。

## `PersistenceState` 状态机

| 状态 | 进入条件 | 读者可采取的动作 |
| --- | --- | --- |
| `Ready(raw)` | value 与 backend 基线一致；raw 为空表示没有 backend 值。 | 正常渲染/读取。 |
| `Dirty(raw)` | `Manual` 模式本地值已 encode，但尚未 flush。 | 提示未保存，调用 `flush`。 |
| `Syncing(raw)` | `Debounced` 模式已捕获本地值，等待 timer 或正在提交。 | 显示保存中；新本地写入会替换旧 request。 |
| `Unavailable` | 当前环境没有 backend。 | 显示降级状态，或换 backend 后重新创建。 |
| `ReadError(message)` | 初始读取或 reload 失败。 | 记录/展示错误并重试 `reload`。 |
| `DecodeError(info)` | raw 无法由当前 codec 解码。 | 根据 policy 修复数据或让用户重置。 |
| `WriteError(message)` | encode、set、remove、timer 或订阅阶段出现写入边界错误。 | 处理返回错误，修复 backend 后用 `flush` 重试。 |

`state()` 返回 `ReadSignal<'scope, PersistenceState>`，因此可以像其它
`silex_core` source 一样被 computed/effect 或 `silex_dom` view 观察。`get()`
建立 tracked 读取，`get_untracked()` 不建立依赖；二者都可能返回
`PersistenceError`。`key()`、`has_persisted_value()`、`reload()`、`remove()`、
`reset()` 和 `flush()` 同样不能忽略 `Result`。

## 本地写入模式

| `PersistWriteMode` | 本地更新后的行为 | 适用边界 |
| --- | --- | --- |
| `Immediate`（默认） | effect 发现 mutation 后立即 encode 并同步 set/remove。 | 低频、小值、需要立即落盘的设置。 |
| `Manual` | 只更新响应式 value，并将与 backend 不同的 raw 标为 `Dirty`；`flush` 才写。 | 表单编辑、批量修改、提交按钮。 |
| `Debounced(duration)` | 每次本地 mutation 替换旧 request，使用 owner-bound `OwnedTimeout` 延迟提交最新 raw。 | 高频输入；必须接受延迟和 timer 取消语义。 |

`flush` 总会取消待执行的 debounce timer，并成为显式 retry 入口。写请求携带
revision；只有当前 request 才能把 `WriteError` 或 `Ready` 写回 state。timer
创建失败会把当前 request 保留为可显式 retry 的失败路径，而不是伪造成功。
owner close、`reload`、`remove` 和外部快照都会取消 timer；它们不会等待异步
写入，因为 backend API 本身是同步的。

## codec 选择

| codec | 约束和表示 |
| --- | --- |
| `StringCodec` | `String` 或 `Cow<'scope, str>` 直接作为 raw。 |
| `ParseCodec<T>` | `T: Display + FromStr + Clone`，`FromStr::Err: Display`。 |
| `PersistJsonCodec<T>` | `json` feature 下要求 `Serialize + DeserializeOwned + Clone`。 |
| `OptionCodec<C, T>` | `Some(T)` 使用内层 codec，`None` 触发删除。 |
| `PersistCodec<T>` 自定义实现 | 自己定义 encode/decode，并可通过 `should_remove` 决定删除。 |

codec 的 `encode`/`decode` 返回 `String` 错误；builder 会把它们映射为
`PersistenceErrorKind::EncodeFailed` 或带原始 raw 的 `DecodeFailed`。decode policy
只影响 decode 失败后的后端处理，不会把错误 raw 当作合法 value。

## 外部值与 remove policy

外部 `Set` 或 `reload` 成功 decode 后会更新 value；这属于外部快照，不会被
`Immediate` effect 当成本地 mutation 再写回。外部 decode 失败时：

- `DecodePolicy::UseDefault` 保留 backend 中的 invalid raw，value 回到默认值，
  state 保留 `DecodeError`；
- `DecodePolicy::RemoveAndUseDefault` 在使用默认值后尝试删除 invalid raw；删除
  失败会把 state 更新为 `WriteError`。

外部 `Removed` 的处理由 `RemovePolicy` 决定：`UseDefault` 将 value 设置为默认值，
`Ignore` 只清空持久化基线并保持当前 value。两种策略都不会因为外部事件自动
重写默认值；下一次真实的本地 mutation 仍必须按当前写入模式处理。

## `Persistent` 的响应式能力

在 trait bounds 满足时，`Persistent`：

- 通过 `RxRead`/`RxWrite` 作为 source 使用；`rx_update_untracked` 仍会标记
  本地 mutation；
- 通过 `ReactiveSource` 转换成 core promotion plan；
- 实现 `StoreField<'scope, T>`，可以作为 store field；
- 通过 `From<Persistent>` 转成 `RwSignal`，也可作为 `View`/attribute source
  交给 `silex_dom`。

转换为 `RwSignal` 只转移响应式读写能力，不会复制 controller，也不会改变
owner 的清理权。binding 的 `remove`、`reset` 和 `flush` 仍应通过
`Persistent` API 调用。

## 对应测试与维护风险

- `crates/silex_persist/tests/builder.rs` 覆盖所有默认值、decode/remove policy、
  immediate/manual 写入、失败重试、optional、外部事件和 reentrant callback。
- `crates/silex_persist/tests/browser.rs` 覆盖 debounced timer、timer 创建失败、
  scope dispose、Storage/query backend 和浏览器资源清理。
- `crates/silex_persist/src/codec.rs` 与 `src/runtime.rs` 的单元测试固定 codec
  映射和 revision/phase transition；它们不是公开的跨线程或事务保证。

修改状态机时，应同时验证“旧 request 不覆盖新 request”“外部快照清除待提交
本地写入”“写入失败可 flush 重试”“关闭后 timer/subscription 被释放”以及
“`PersistenceState` 与返回的 `Result` 不互相矛盾”。
