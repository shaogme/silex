+++
title = "测试与调试"
description = "silex_persist 的 native、browser、UI 编译测试和文档示例验证方法。"
weight = 40
+++

# 测试与调试

`silex_persist` 的行为同时由 typestate lifetime、响应式 effect、同步 backend、
浏览器 host resource 和外部事件决定。只验证一次 `set` 成功不足以证明绑定
安全；新增 backend、codec、写入模式或状态分支时，应分别验证正常路径、失败
路径、外部快照和 owner cleanup。

## 测试分层

| 位置 | 覆盖内容 | 环境 |
| --- | --- | --- |
| `src/**` 单元测试 | codec 映射、request phase/revision、Storage hub 状态机、错误转换 | native 为主 |
| `tests/builder.rs` | builder 初始化、默认值、写入策略、codec、错误、外部事件、cleanup | native |
| `tests/browser.rs` | local/session storage、query history、Storage event、debounce timer、browser cleanup | wasm browser |
| `tests/compile_fail.rs` + `tests/ui/` | builder、backend callback、view、static sink 的 scope escape | native trybuild |
| `tests/docs_examples.rs` | `docs/examples/silex_persist/basic.rs` 编译和执行 | native |

browser 测试文件使用 `#![cfg(target_arch = "wasm32")]` 与
`wasm_bindgen_test_configure!(run_in_browser)`；native `cargo test` 不会执行
这些 browser case。新增文档示例使用内存 backend，因此无需浏览器即可验证
公开 builder API。

## 常用验证命令

在仓库根目录运行：

```text
RUSTFLAGS=-D warnings cargo fmt --all -- --check
RUSTFLAGS=-D warnings cargo check -p silex_persist
RUSTFLAGS=-D warnings cargo test -p silex_persist --test docs_examples
RUSTFLAGS=-D warnings cargo test -p silex_persist --test builder
RUSTFLAGS=-D warnings cargo test -p silex_persist --test compile_fail
RUSTFLAGS=-D warnings cargo check -p silex_persist --all-features
zola --root docs check
```

只检查浏览器测试的 wasm 编译时：

```text
RUSTFLAGS=-D warnings cargo test -p silex_persist --test browser \
    --target wasm32-unknown-unknown --no-run
```

如果环境配置了 `wasm-bindgen-test-runner` 和浏览器，再运行同一测试而不加
`--no-run`。文档示例新增或修改后，至少运行 `docs_examples`；不需要为了这一
类文档改动执行 workspace 或其它 crate 的测试。

## 运行时契约清单

修改 builder/初始化时，至少覆盖：

- 缺失、可用 raw、backend unavailable、读失败和 decode 失败的状态分别正确；
- `WriteDefault::Never` 不写默认值，`IfMissing` 只处理缺失，`Always` 能规范化
  已有 raw；
- builder 返回错误时已创建的 owner 节点、subscription 和 resource 不泄漏；
- `DecodePolicy`、`RemovePolicy` 不会把外部变化误标成本地 mutation。

修改写入/runtime 时，至少覆盖：

- Immediate 只提交当前本地 mutation，Manual 在 flush 前保持 `Dirty`；
- Debounced 每次新写入都取消旧 timer，只有最新 revision 能提交；
- timer 创建/取消失败进入 `WriteError`，显式 `flush` 仍可重试；
- backend 写入失败保留请求语义，成功后才更新 `last_backend_raw` 和 `Ready`；
- owner close、reload、remove 和外部快照都取消 timer，不再接受旧 callback。

修改 backend/外部同步时，至少覆盖：

- 外部 `Set`、`Removed` 和 `ExternalRefresh` 的 key 过滤与状态转换；
- subscription 在正常 close、订阅失败 rollback 和 reentrant listener 中只清理一次；
- callback error、close error 和 backend error 都能交给对应 error handler；
- Query backend 不会修改其它 query key，Storage hub 不会为每个绑定重复安装
  永久 listener。

修改公开 lifetime 或错误签名时，先增加最小 `tests/ui/` case，再更新对应
`.stderr`。不要通过放宽 lifetime、改成 `'static` 或忽略错误来掩盖 scope regression。

## 文档示例测试

可执行示例只保存在 `docs/examples/silex_persist/basic.rs`，页面通过
`load_data(..., format="plain")` 读取它。测试入口保持简单：

```rust
#[path = "../../../docs/examples/silex_persist/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    assert!(basic::run().is_ok());
}
```

页面中如果需要展示 browser-only 的 `.local()`、`.session()` 或 `.query(ctx)`
配置，可使用普通 Markdown fenced code，并明确它依赖 browser/context、不是
独立 CI 示例。不要把带省略号、伪 backend 或未声明 error handler 的片段放入
`docs/examples/`。

## 调试顺序

1. 先确认 binding、owner 和 backend key 是否仍活动；关闭后的句柄返回
   `ReactiveError::NoSuchNode` 是生命周期信号，不是 backend 空值。
2. 读取 `PersistenceState`，再检查操作返回的 `PersistenceError`；区分
   `BackendUnavailable`、`ReadFailed`、`DecodeFailed`、`WriteFailed` 和
   `Reactivity`，不要只看 `message()`。
3. 对本地写入记录 mode、当前 raw、`Dirty`/`Syncing`/`Ready` 转换和 flush 时机；
   对 Debounced 检查旧 timer 是否取消，以及 callback 是否命中当前 revision。
4. 对外部事件记录 `BackendEvent`、key、decode/remove policy 和
   `last_backend_raw`；确认外部快照不会再次设置 local mutation 标记。
5. 对 browser 资源检查 Storage listener、completion endpoint、`OwnedTimeout`
   和 owner cleanup 的释放顺序；若发生 close failure，保留结构化错误，不要只
   依赖字符串日志。
6. 对自定义 backend 检查 `BackendSubscribeError` 是否携带了失败前创建资源的
   cleanup token，并确认 `BackendSubscription::Drop` 不会重入用户业务代码。

## 对应测试索引

- `tests/builder.rs`：builder、codec、state、写入/重试和 native cleanup。
- `tests/browser.rs`：Storage/query/timer 的浏览器边界。
- `tests/compile_fail.rs` 与 `tests/ui/`：编译期 owner/lifetime 契约。
- `tests/docs_examples.rs` 与 `docs/examples/silex_persist/basic.rs`：可执行文档流程。
