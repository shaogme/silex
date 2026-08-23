+++
title = "测试与调试"
description = "silex_core 的测试分层、编译期契约、异步验证和文档示例。"
weight = 40
+++

# 测试与调试

`silex_core` 的行为同时由 Rust lifetime、owner registry、响应式图、异步 cleanup 和 feature-gated 错误类型决定。单独验证“signal 能读写”不足以证明组件生命周期安全；新增 API 时应同时检查运行时行为、编译期拒绝和异步释放。

## 测试分层

| 位置 | 覆盖内容 | 维护重点 |
| --- | --- | --- |
| `src/**` 单元测试 | 错误转换、状态 id 过滤、局部 trait 行为 | 内部不变量和纯函数语义。 |
| `tests/*.rs` | runtime、signal guard、watch、transaction、错误、异步集成行为 | 可观察 API 契约，不依赖 node id。 |
| `tests/ui/*.rs` | `trybuild` pass/fail 编译测试 | lifetime escape、`Send`、handler 和已删除 API。 |
| `tests/docs_examples.rs` | `docs/examples/silex_core/basic.rs` 编译/执行 | 页面代码与当前 facade API 同步。 |
| `tests/async_completion.rs` | wasm-bindgen 浏览器异步测试 | future drop、completion、Resource/Mutation 和 task cancellation。 |

测试名应描述用户可观察的契约，例如“foreign tracked read is rejected”或“stale mutation completion is ignored”，不要把内部 slot、node id 或 registry index 当成稳定行为。

## 常用验证命令

在仓库根目录运行：

```text
cargo fmt --all -- --check
cargo check -p silex_core
cargo test -p silex_core
cargo test -p silex_core --test docs_examples
cargo test -p silex_core --test compile_fail
cargo check -p silex_core --all-features
cargo clippy -p silex_core --all-targets --all-features -- -D warnings
```

站点检查在 `docs/` 目录运行：

```text
zola check
```

仓库 CI 额外使用 `RUSTFLAGS=-D warnings`、workspace test 和全 workspace clippy；文档改动至少应确认 core crate、文档示例与 Zola 检查通过。

## 文档示例

可执行示例只保存在 `docs/examples/silex_core/basic.rs`。页面通过 Zola 的 `load_data(..., format="plain")` 读取同一文件，测试通过：

```rust
#[path = "../../../docs/examples/silex_core/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    assert!(basic::run().is_ok());
}
```

页面中的短片段如果省略外层函数、context、错误类型或业务函数，必须明确说明它不是 CI 编译示例。不要把含有 `...`、伪函数或未声明变量的片段复制到 `docs/examples/`。

## 运行时行为清单

修改 owner 或响应式 API 时，至少覆盖：

- `Runtime::owner` 的单 root 限制、root/child close 和 transient 自动 close；
- tracked/untracked signal 读取、`ReadGuard`/`WriteGuard` 的 finish/commit/abort、`notify`、`StoredValue` 和 `NodeRef` 的 stale 行为；
- same-runtime child tracked 依赖、foreign tracked `RuntimeMismatch` 和 foreign untracked 无订阅；
- computed equality gate、`computed_always`、effect 初始运行、stop 幂等性和 watch `immediate`/`once`；
- tuple source、`batch_read!`、`batch_read_untracked!` 和 projection 的依赖边；
- borrow conflict、handler 分发、用户错误与 runtime fatal error 的分层；
- Resource 的 Loading/Reloading、suspense count、source 替换、旧 request completion；
- Mutation 的 Pending/Success/Error、prepare 失败、请求顺序逆转和 stale completion；
- transaction 的多 signal 原子发布、snapshot 不追踪、duplicate target、foreign runtime 和用户错误回滚；
- scoped task 的 cancel、future drop、owner close 和 callback panic 后的 endpoint 状态；
- cleanup 错误聚合、`take_unhandled_close_errors` 和 `test-support` 快照（若修改底层 runtime 契约）。

## 编译期契约

`tests/compile_fail.rs` 运行 `trybuild`：

- `fail_*_escape.rs` 保证 transient handle、callback、handler 和 scoped task 不能逃逸；
- `fail_send_handler.rs`、`fail_scoped_handle_in_future.rs` 等保证单线程 owner capability 不被误送到不兼容边界；
- `fail_missing_error_handler.rs`、`fail_resource_without_handler.rs` 和 mutation 相关用例保证延迟错误有明确交付路径；
- `fail_transaction_escape.rs`、`fail_transaction_across_await.rs` 保证 transaction 不能逃逸 owner 或跨越 `await`；
- `fail_root_symbols.rs`、`fail_old_*.rs`、`fail_removed_*.rs` 防止旧 API 或内部 root symbol 重新成为公共用法；
- `pass_*.rs` 验证 scoped handler、copyable mutation 和合法 future 使用仍然可编译。

修改 lifetime、`UnwindSafe`、`Send`、handler 输入或公开签名时，先增加最小 UI 用例，再更新实现和对应 `.stderr`。只有诊断确实因预期 API 变化而改变时才更新 stderr。

## 异步测试边界

`tests/async_completion.rs` 使用 `wasm-bindgen-test` 和 `gloo-timers`，并通过 `#[cfg(target_arch = "wasm32")]` 隔离浏览器执行器。native `cargo test -p silex_core` 不会运行这些 browser tests；涉及 `spawn_local`、JavaScript console 或 timer 的变更必须在仓库既有 wasm 测试流程中复核。

测试异步代码时，除了最终状态，还要用 drop 计数或等价观察确认：

- owner close 后 pending future 被释放；
- source 替换后旧请求不会覆盖新状态；
- prepare error 会使前一 completion 失效；
- suspense 每个 request 只 decrement 一次；
- completion callback 错误和 close 错误都能交给 handler。

## 调试顺序

遇到 core 行为异常时，建议按以下顺序缩小范围：

1. 检查句柄所属 owner 是否活动，以及 source 与 target 是否同一 runtime。
2. 区分 `SilexErrorKind::Reactivity`、用户 `E`、handler error 和 `CloseError`，不要只看 Display 文本。
3. 对 tracked/untracked、batch 和 watcher 记录实际读取分支，确认失败运行没有留下错误依赖。
4. 对 Resource/Mutation 记录 request id、状态转换和 completion drop，确认旧结果只是被丢弃而不是错误地写入状态。
5. 检查 task/future 的 drop 时机、cleanup 错误聚合和 panic recovery；启用 `test-support` 时，在稳定边界读取 `RuntimeSnapshot`。

## 对应测试索引

- `tests/root_scope.rs`：root、transient 和 owner access。
- `tests/runtime_compatibility.rs`：same-runtime 与 foreign-runtime source。
- `tests/reactivity_errors.rs`：borrow conflict、stale node、NodeRef 和 error mapping。
- `tests/signal_guards.rs`：scoped guard、owned snapshot、projection 和 guard 生命周期。
- `tests/batch_read.rs`、`tests/tuple_traits.rs`、`tests/watch.rs`：聚合读取和 watcher。
- `tests/transaction.rs`：原子提交、snapshot、用户错误和 runtime transaction error。
- `tests/error_reporter.rs`：handler/reporter 行为。
- `tests/async_completion.rs`：Resource、Mutation、completion 和 scoped task。
- `tests/for_loop_source.rs`：`ForLoopSource` 的 Vec/Option/Result 输入。
- `tests/compile_fail.rs` 与 `tests/ui/`：编译期边界。
- `docs/examples/silex_core/basic.rs` 与 `tests/docs_examples.rs`：可执行文档流程。
