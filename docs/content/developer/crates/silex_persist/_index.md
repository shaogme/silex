+++
title = "silex_persist"
description = "将响应式状态与字符串持久化后端绑定的 owner-scoped 存储 crate。"
template = "section.html"
sort_by = "weight"
+++

# `silex_persist`

`silex_persist` 将一个 `silex_core` 响应式值绑定到按 key 读写字符串的
`PersistenceBackend`。它负责初始化读取、codec 编解码、本地写入策略、外部
变更同步、错误状态和 owner 清理；它不负责数据库事务、跨线程执行或敏感数据
加密。

## 在 Silex 架构中的位置

```text
应用组件 / silex_dom / silex_router
              │
              ▼
       Persistent<'scope, T>
       value · state · flush
              │
              ▼
       silex_core owner/runtime
              │
              ├── PersistenceBackend → localStorage/sessionStorage
              ├── PersistenceBackend → Router query
              └── PersistenceBackend → custom backend
```

`Persistent` 同时是可读写的响应式 source、`StoreField` 和（满足相应 trait
约束时）`View`。它适合保存设置、路由查询参数或需要跟随组件生命周期的本地
状态；调用方仍必须把 `OwnerAccess<'scope>` 显式传入 builder。

## 稳定入口与核心类型

| 入口 | 主要用途 |
| --- | --- |
| `Persistent::builder` | 创建带 key、owner 和错误处理入口的 typestate builder。 |
| `PersistentBuilder` | 选择 backend、codec、默认值和写入/同步策略；`build` 返回绑定。 |
| `Persistent<'scope, T>` | 读写本地值，观察 `state`，执行 `reload`、`remove`、`reset` 和 `flush`。 |
| `PersistenceBackend<'scope>` | 自定义同步字符串后端及其外部事件订阅契约。 |
| `LocalStorageBackend` / `SessionStorageBackend` | 浏览器 Web Storage 的 local/session 实现。 |
| `QueryBackend<'scope>` | 通过 `RouterContext` 将值映射到 URL 查询参数。 |
| `PersistCodec<T>` | 定义 `encode`、`decode` 和可选的 `should_remove`。 |
| `StringCodec` / `ParseCodec<T>` / `OptionCodec<C, T>` | 常用字符串、`FromStr` 和可选值编解码器。 |
| `PersistJsonCodec<T>` | `json` feature 下基于 serde 的编解码器。 |
| `PersistenceState` | 暴露 `Ready`、`Dirty`、`Syncing` 和各类错误状态。 |
| `PersistenceError` / `PersistenceErrorKind` | 区分可恢复后端/编解码错误与 fatal owner/configuration 错误。 |

应用代码通常从 `silex_persist` 根导入上述入口，并从
`silex_core::prelude` 或 crate 根导入 `Runtime`、`OwnerAccess` 与错误类型。

## 最小可运行流程

下面的示例使用一个内存 backend，因此可以在没有浏览器对象的 native 测试中
执行。页面直接读取 `docs/examples/silex_persist/basic.rs`，不是在 Markdown
中维护另一份会独立演进的代码。

{% set source = load_data(path="examples/silex_persist/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

示例由 `crates/silex_persist/tests/docs_examples.rs` 编译并执行。它展示了
`parse::<i32>()`、`write_default(WriteDefault::Never)`、响应式 `set` 和
立即写入；所有公开操作都通过 `?` 传播错误。

## 数据流与生命周期

```text
build
  │ backend.get(key)
  ├─ raw + decode ─────► value + Ready(raw)
  ├─ missing/error ────► default + state(error/unavailable)
  └─ optional bootstrap write

value.set/update
  │ mark local mutation
  ├─ Immediate ────────► encode → backend.set/remove
  ├─ Manual ───────────► encode → Dirty；flush 时提交
  └─ Debounced ────────► OwnedTimeout → commit latest request

backend event
  └─ CompletionSender → decode/remove policy → value + state

owner close
  └─ cancel timeout → drop subscription → invalidate binding
```

- `Persistent<'scope, T>` 是 `Copy + Clone` 的能力句柄，不拥有 owner，也不
  通过句柄计数管理 backend、subscription 或 timer 的生命周期。创建它的
  `OwnerAccess` 关闭后，继续操作通常返回包含 `ReactiveError::NoSuchNode`
  的 fatal `PersistenceError`；`reset` 对这种关闭后的情况保持幂等。
- 运行时使用 `Rc`、`Cell`、`RefCell` 和浏览器同步 API，是单线程模型。不能
  把 `Persistent`、自定义 backend 或带 scope 的 codec 当作 `Send + Sync` 的
  跨线程状态。
- 外部事件先进入 owner 的 completion endpoint，再修改绑定；owner 关闭后
  endpoint 不会调用已经失效的用户代码。backend subscription 和防抖 timer
  都由 owner cleanup 释放。
- 每次写请求都有 revision。新请求、`reload`、`remove` 或外部快照会使旧的
  timer/request 失效；这保护状态不会被迟到的本地提交覆盖，但不会撤销已经
  发给后端的同步写入。

## Feature flags 与平台

`crates/silex_persist/Cargo.toml` 默认不启用 feature。`json` 只添加
`PersistJsonCodec<T>` 及 serde/serde_json 依赖；它不改变 owner、写入策略或
错误状态语义。Web Storage 和 Query backend 依赖 `web_sys` 的浏览器环境，
但 `PersistenceBackend` 自定义实现和内存测试可以在 native 下运行。

## 专题

- [后端、订阅与外部同步](backends.md)：内置 backend、`BackendSubscription`
  清理、`BackendSubscribeError` 和自定义实现。
- [状态、builder 与 codec](state.md)：typestate 配置、初始化、写入模式、
  `PersistenceState`、编解码和错误路径。
- [测试与调试](testing.md)：native/browser/UI 测试分层、文档示例和诊断顺序。

## 源码、示例与测试索引

- 公开入口和策略枚举：`crates/silex_persist/src/lib.rs`
- backend、Storage hub 和 Query backend：`crates/silex_persist/src/backend.rs`
- typestate builder 和初始化/写入 effect：`crates/silex_persist/src/builder.rs`
- codec：`crates/silex_persist/src/codec.rs`
- request phase、revision 和 timer 所有权：`crates/silex_persist/src/runtime.rs`
- `Persistent`、状态机和外部快照：`crates/silex_persist/src/state.rs`
- 文档示例：`docs/examples/silex_persist/basic.rs`
- 文档示例测试：`crates/silex_persist/tests/docs_examples.rs`
- builder、codec、错误和 cleanup：`crates/silex_persist/tests/builder.rs`
- browser Storage/query/timer 测试：`crates/silex_persist/tests/browser.rs`
- 编译期 scope 契约：`crates/silex_persist/tests/compile_fail.rs`、`tests/ui/`

## 已知限制与维护注意

- `WebStorageBackend` 将编码后的字符串以浏览器 profile 中的明文保存。不要
  用它存储密码、token、长期凭据或其它需要保密的数据；需要保密性时，应在
  backend/codec 外层接入经过审计的加密方案，并单独处理密钥生命周期。
- Web Storage 的 `get`、`set` 和 `remove` 是同步调用。`Immediate` 会在响应式
  更新路径中直接执行 backend 写入；写入频繁或后端较慢时，应评估 `Manual`
  或 `Debounced`，但仓库没有提供未经基准验证的延迟/吞吐保证。
- crate 只为单个 key 提供读写和订阅接口，没有多 key 原子事务、冲突合并或
  服务端一致性协议。外部快照到达时，当前本地待提交 request 会被失效；业务
  若需要合并，必须在 codec/backend 或上层模型中定义规则。
- `PersistenceError::Recoverable` 表示可以显示状态并重试，例如 backend 不可用、
  读写失败或 decode/encode 失败；`Fatal` 通常表示 owner、runtime 或配置失效。
  `PersistenceState` 是给 UI/响应式逻辑观察的状态，不替代调用方对返回的
  `Result` 进行处理。

验证本 crate 文档或公开 API 变更时，至少运行
`RUSTFLAGS=-D warnings cargo check -p silex_persist`、对应测试和
`zola --root docs check`。新增 `docs/examples/silex_persist/` 文件后，优先
运行 `RUSTFLAGS=-D warnings cargo test -p silex_persist --test docs_examples`；
无需为该文档变更运行整个 workspace 或其它 crate 的测试。
