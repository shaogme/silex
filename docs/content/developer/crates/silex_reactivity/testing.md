+++
title = "测试与调试"
description = "silex_reactivity 的测试分层、编译期契约、运行时快照和基准边界。"
weight = 40
+++

# 测试与调试

`silex_reactivity` 的正确性同时依赖 Rust 生命周期、运行时 owner 图、依赖事务、
清理阶段和 panic 恢复。只测试“signal 写入后 effect 运行”不足以覆盖这些边界，
维护者应根据行为选择对应的测试层。

## 测试分层

| 测试位置 | 覆盖内容 | 适合验证 |
| --- | --- | --- |
| `tests/docs_examples.rs` | 文档示例的真实编译和执行 | 页面代码与当前公开 API 保持一致 |
| `tests/*.rs` | 运行时集成行为 | 图更新、scope 关闭、错误、panic 和 completion |
| `tests/value_inline_tests.rs` | typed payload 的存储与 drop | 小值/大值、内部可变性和 payload 释放 |
| `tests/ui/*.rs` | `trybuild` 编译期契约 | 句柄逃逸、跨线程、缺失 handler 和过时 API |
| `src/**` 的单元测试 | 内部图与 scratch 状态 | 不变量、队列恢复和存储实现细节 |
| `benches/reactivity.rs` | Criterion 基准 | 在明确基准环境下比较趋势 |

测试名称应表达可观察契约，例如“关闭子 owner 后不再触发 effect”，不要只写
“测试 node id 7”。内部 node id、owner slot 和 generation 只适合诊断，不是公共
行为契约。

## 常用验证命令

在仓库根目录运行 crate 验证：

```text
cargo fmt --all -- --check
cargo check -p silex_reactivity
cargo test -p silex_reactivity
cargo test -p silex_reactivity --features test-support
cargo test -p silex_reactivity --test docs_examples
cargo test -p silex_reactivity --test compile_fail
```

站点检查在 `docs/` 目录运行：

```text
zola check
```

`compile_fail` 测试使用 `trybuild` 扫描 `tests/ui/fail_*.rs`，同时编译
`tests/ui/pass_*.rs`。修改公开签名、生命周期约束或错误类型后，必须检查对应的
`.stderr` 预期输出；只有在编译器诊断确实因预期 API 变化而改变时才更新它。

## 文档示例的验证方式

可执行示例只保留在 `docs/examples/`，页面通过 `load_data` 读取同一个源文件。
`crates/silex_reactivity/tests/docs_examples.rs` 使用 `#[path]` 引入
`docs/examples/silex_reactivity/basic.rs`，因此 `cargo test --test docs_examples`
会同时验证示例的编译和运行。

如果片段省略外层函数、错误类型或业务函数，就把它放在 Markdown 普通 fenced
code 中，并明确说明“不是 CI 编译示例”。尤其不要在页面中复制一份与
`docs/examples/` 分开演进的长 Rust 示例；这会让文档和测试逐渐漂移。

## 运行时行为测试清单

新增节点或修改调度逻辑时，至少考虑以下场景：

- signal 的 tracked/untracked 读取、`set_if_changed`、内部可变值和 `notify`。
- computed 输出相等与 `computed_always` 的差异；失败重算是否保留旧值和旧依赖。
- effect 初始运行、重复运行、`effect_with_previous` 的上一次成功值，以及
  `EffectHandle::stop` 的幂等性。
- watch 的初始 getter、`immediate`、`once`、结果相等和回调 untracked 读取。
- 动态依赖切换、跨 owner 同 runtime 追踪、跨 runtime tracked 拒绝和 untracked
  快照不订阅。
- batch 正常返回和 panic 后的 scheduler 恢复；反馈写入环触发
  `NonConvergent` 后，其他 owner 的工作仍能继续。
- Normal/PostFlush 的执行顺序、PostFlush 写入重新进入 Normal、同阶段注册顺序和
  双队列错误恢复；涉及 DOM 的 focus/Portal 回归还要运行实际浏览器测试。
- `ReactiveTransaction` 的多 signal 暂存、重复目标、提交/回滚和事务深度恢复；
  事务错误还要检查 rollback failures 是否进入关闭诊断队列。
- owner child-first 关闭、cleanup 注册顺序、cleanup 错误聚合、stored value 的
  最终 cleanup 访问和 payload drop。
- completion 的重复 submit、cancel、最后一个 sender drop、callback 错误、close
  错误以及 callback panic 后的终态；同时覆盖 detached endpoint 在 effect
  disposal 后仍可提交，以及 disposal cleanup 中 endpoint drop 的 pending drain
  和重复登记去重。

这些契约当前分布在 `tests/automatic_tracking.rs`、`tests/graph.rs`、
`tests/runtime_compatibility.rs`、`tests/runtime_scope.rs`、
`tests/root_scope.rs`、`tests/watch.rs`、`tests/post_flush.rs`、
`tests/panic_reentry.rs`、`tests/completion.rs`、`tests/owned_scope.rs`、
`tests/native_errors.rs` 和 `tests/read_pipeline.rs`；事务与 guard 的契约还在
`src/transaction.rs` 的单元测试和 `tests/signal_guards.rs` 中。对应回归用例包括
`completion_drop_during_scope_cleanup_uses_the_pending_endpoint_drain` 与
`detached_completion_survives_effect_disposal`。

## `test-support` 与 `RuntimeSnapshot`

开启 `test-support` feature 后，crate 才导出 `RuntimeSnapshot`，并允许测试通过
`OwnerHandle::runtime_snapshot()` 或 `OwnerAccess::runtime_snapshot()` 观察运行时
状态。快照适合验证资源是否回收和队列是否恢复，不应作为应用运行时 API：

```rust
#[cfg(feature = "test-support")]
let snapshot = scope.runtime_snapshot()?;
```

常用字段包括：

- `nodes`、`data`、`edges`、`roots`：节点、payload、依赖边和图根数量。
- `cleanups`、`handlers`、`active_leases`：仍存活的清理、handler 和 lease。
- `queue`、`running_queue`、`queue_recovery`：调度队列是否为空且可继续运行。
- `queue_high_water`：当前 runtime 生命周期内 normal、post-flush 和 worklist
  合计排队量的高水位，仅用于基准和回归诊断。
- `active_owners`、`closing_owners`、`retained_children`：owner registry 的状态。
- `live_typed_slots`、`live_error_slots`：关闭后类型化 payload 和错误槽是否归还。
- `unhandled_close_errors`、`dropped_close_reports`：Drop/panic 路径的关闭诊断。

快照读取本身可能返回 `ReactiveError::BorrowConflict`，因为它遵循与其他 runtime
操作相同的动态借用规则。测试应在稳定边界采样，例如节点创建后、显式 stop 后或
owner close 后；不要在用户 callback 持有写借用时强行读取快照。

## 编译期契约

UI 测试是这个 crate 的重要安全边界，不能只把失败测试当作“编译器噪声”：

- `fail_child_handle_escape.rs`、`fail_callback_escape.rs` 和
  `fail_handler_escape.rs` 保证 transient lifetime 不能逃逸。
- `fail_callback_send.rs`、`fail_send_handler.rs` 保证单线程 runtime 对应的句柄
  不会被误认为 `Send`。
- `fail_root_dispose_borrow.rs` 和相关 root 测试保证 close 后的借用不能继续访问
  已释放节点。
- `fail_missing_error_handler.rs`、`fail_memo_missing_error_handler.rs` 等保证
  计算必须显式提供错误处理入口。
- `fail_mixed_track_batch.rs`、`fail_unscoped_handler.rs` 等保证不同 scope 和
  动态上下文不会被错误拼接。
- `fail_removed_try_*.rs`、`fail_old_*.rs` 保留已删除 API 的迁移护栏，防止旧用法
  悄悄重新进入公共接口。

涉及 lifetime、`UnwindSafe`、`Send` 或 handler 输入类型的修改时，应优先增加一个
最小 UI 用例，再修改实现。通过 runtime 检查补救本应由编译器拒绝的句柄逃逸，会把
错误推迟到更难诊断的异步或关闭阶段。

## 基准与性能说明

`benches/reactivity.rs` 覆盖 signal 创建、tracked/untracked 读取、写入、stored
value、node ref、事务提交、completion 消息、依赖图、动态依赖 remove/reinsert、
Normal/PostFlush 混合队列和 owner churn 等场景。没有在固定硬件、编译 profile 和样本配置下
运行 Criterion 之前，不要在文档或提交信息中写具体延迟、吞吐或复杂度数字。

阶段调度基准使用 `scheduler/mixed-phase` 名称；该基准同时记录每轮工作量和
`RuntimeSnapshot::queue_high_water` 的稳定边界，便于比较最大排队量与吞吐趋势。
事务基准使用 `transaction/commit`，completion 基准使用
`proxy/completion-message`。Wasm
release 体积属于构建产物指标，应使用相同 target/profile 通过 `stat` 或
`wasm-size` 记录，不应混入 native Criterion 数字。

基准只用于比较可重复的实现变化，不能替代生命周期和错误测试。新增优化时应同时
确认：

- scratch pool 或缓存没有延长 owner、payload、handler lease 的生命周期；
- 队列恢复和 cleanup 仍然覆盖 panic、handler 失败和 dynamic borrow conflict；
- 跨 owner 的依赖传播不会把已关闭 owner 留在 scheduler 队列中；
- 资源计数通过 `RuntimeSnapshot` 或 drop 计数测试回到预期状态。

## 调试顺序

遇到响应式行为异常时，建议按以下顺序缩小范围：

1. 先检查句柄是否仍属于活动 owner，以及 tracked 读取是否处在同一个 runtime。
2. 再区分 `ReactiveError`、用户 `E` 和 `HandlerError`，不要只打印顶层字符串。
3. 使用 `RuntimeSnapshot` 检查节点、边、queue、lease 和 slot 是否在预期边界归零。
4. 对动态依赖记录每次成功运行的读取分支；失败运行不应留下临时依赖边。
5. 最后检查 panic 或 cleanup 错误是否进入了 close report queue，并调用
   `Runtime::take_unhandled_close_errors()` 取出 Drop 路径的诊断。
