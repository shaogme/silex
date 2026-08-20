+++
title = "测试与调试"
description = "silex_dom 的 native、browser、编译期契约和文档示例验证方法。"
weight = 60
+++

# 测试与调试

`silex_dom` 的行为同时由 Rust lifetime、DOM 节点、owner tree、响应式
effect、JavaScript callback 和 mount rollback 决定。只验证“div 能创建”
不足以证明 view 安全；新增 view/attribute/host resource 时，应分别验证
成功路径、partial failure、owner dispose 和编译期 scope 边界。

## 测试分层

| 位置 | 覆盖内容 | 运行环境 |
| --- | --- | --- |
| `src/**` 单元测试 | attribute op/consolidation、native mount error helper、清理 report | native 为主 |
| `tests/mounted_contract.rs` | `MountError`、`DisposeError`、`CleanupReport`、`CleanupSink` | native |
| `tests/mounted_app.rs` | staging boundary、commit、rollback、remount、dispose、host 节点保护 | wasm browser |
| `tests/owner.rs` | owner effect/cleanup、动态 branch、indexed/keyed list、row state | wasm browser |
| `tests/host_resources.rs` | DOM/window listener、timer-like resource、owner lease、callback gate 和幂等取消 | wasm browser |
| `tests/reactive_attribute.rs` | reactive attr/class/style、property restore、SVG namespace | wasm browser |
| `tests/compile_fail.rs` + `tests/ui/` | view、attribute、row updater、host callback 和 mount builder 的 scope escape | native trybuild |
| `tests/docs_examples.rs` | `docs/examples/silex_dom/basic.rs` 的编译与执行 | native + wasm |

browser 集成测试文件通常以 `#![cfg(target_arch = "wasm32")]` 开头，并
使用 `wasm_bindgen_test_configure!(run_in_browser)`。native `cargo test`
不会执行这些 browser case；它们仍会在 wasm target 下参与编译。

## 常用验证命令

在仓库根目录运行：

```text
cargo fmt --all -- --check
RUSTFLAGS=-D warnings cargo check -p silex_dom
RUSTFLAGS=-D warnings cargo test -p silex_dom --test docs_examples
RUSTFLAGS=-D warnings cargo test -p silex_dom --test mounted_contract
RUSTFLAGS=-D warnings cargo test -p silex_dom --test compile_fail
```

仅编译并运行 browser 文档示例时：

```text
RUSTFLAGS=-D warnings cargo test -p silex_dom --test docs_examples \
    --target wasm32-unknown-unknown
```

该命令使用仓库 `.cargo/config.toml` 的
`wasm-bindgen-test-runner`。如果当前环境没有可用浏览器，只做示例的
wasm 编译检查：

```text
RUSTFLAGS=-D warnings cargo test -p silex_dom --test docs_examples \
    --target wasm32-unknown-unknown --no-run
```

站点检查在仓库根目录运行：

```text
zola --root docs check
```

用户只修改文档示例时，不需要为了验证 `silex_dom` 而运行 workspace 或
其它 crate 的测试；至少执行对应的 `docs_examples` 编译/测试和站点检查。

## 运行时契约清单

修改 `MountedApp` 时，至少覆盖：

- 新 host 保留 caller-owned children，boundary 节点只追加在其后；
- builder 返回 primary error 时 staging 内容不提交；
- clean rollback 允许同一 handle retry，cleanup/boundary failure 会 poison；
- remount 先清理旧 session，dispose 在 Ready 状态幂等；
- Drop 阶段的失败进入 `CleanupSink`，sink panic 不穿透 Drop。

修改 view/dynamic/list 时，至少覆盖：

- `View` factory 多次 mount 返回不同 `MountInstance`；
- partial child mount 会关闭 provisional owner 并移除 detached DOM；
- reactive view 更新失败保留前一次成功内容；
- branch key 改变时关闭旧 content owner，key 相同时保留 branch runtime；
- indexed list 失败后恢复旧 rows，keyed list 重排保留正确 identity；
- duplicate key、pending row failure 和 stale `RowUpdater` 都有明确结果。

修改 attribute/event/helper 时，至少覆盖：

- `Attr::Removed`/`Empty`/`String` 的 attribute 语义和 known property 语义；
- static 与 reactive class/style 的合并、更新和 cleanup，不互相删除；
- listener add/remove、typed/untyped event cast、callback error handler；
- rerender 后旧 listener 只移除一次，root close 再移除新 listener；重复
  `HostResource::cancel` 不增加物理 remove/clear 次数；
- owner close 后 timer/listener/closure 不能再调用用户 callback；
- `NodeRef` 类型不匹配和清理时 stale node 的错误策略。

## 编译期契约

trybuild 用例把 lifetime 和所有权约束固定下来：

- `fail_child_view_escape.rs` 及 `pass_scoped_view.rs` 相关用例固定 view
  和 child owner 的 scope 边界；
- `fail_pending_attribute_escape.rs`、`fail_scoped_thunk_escape.rs` 阻止
  属性和擦除 thunk 借用超出 view scope；
- `fail_row_updater_escape.rs` 阻止 row updater 成为过长生命周期的 setter；
- `fail_detached_host_callback.rs` 与 `fail_scoped_host_callback.rs` 区分
  `'static` detached callback 和 owner-bound callback；
- `fail_mounted_app_scope_escape.rs`、`fail_mounted_app_dispose_use.rs`
  固定 builder 和 dispose 后的 API 边界；
- `fail_mount_result_ignored.rs` 确保 `MountInstance` 的 `#[must_use]` 契约
  不被删除。

修改公开签名、lifetime 或 error handler 输入时，应先增加最小 UI case，
再更新实现和预期 `.stderr`。只有诊断确实因为预期契约变化才更新 stderr，
不要用宽松模式掩盖 scope regression。

## 调试顺序

遇到 DOM 或 owner 行为异常时，建议按以下顺序缩小范围：

1. 确认代码运行在 browser target，并区分 host 被外部移除和 app session
   被 dispose；`MountedApp::is_active()` 只反映 owner/session，不是 host
   是否仍挂在 document 上。
2. 检查 `MountError::primary()` 与 rollback report，确认是 builder/DOM
   主错误还是 cleanup/boundary 二次错误。
3. 对动态 view/list 记录 key、row generation、当前 range 和 owner close
   顺序；不要只检查最终 text content。
4. 对属性检查 `ApplyTarget` 是 Attr、Prop、Known、Class 还是 Style，再看
   consolidation 是否覆盖了同名 reactive plan。
5. 对 callback 检查 owner active、destination gate、completion submit 和
   error handler；关闭后 callback 被丢弃是预期行为，不是静默丢事件。
6. 对 cleanup failure 使用 `CleanupReport`/`DropFailureReport` 的结构化
   origin 和 diagnostic，不要只依赖 `Display` 字符串。

## 文档示例

可执行示例只保存在 `docs/examples/silex_dom/basic.rs`，页面通过
`load_data(..., format="plain")` 读取它，测试入口是：

```rust
#[path = "../../../docs/examples/silex_dom/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented DOM example should compile and run");
}
```

DOM 真实挂载依赖 browser，所以示例在 native 分支只构造 view，在 wasm
分支才创建 host 并执行 `MountedApp` mount/dispose。页面中如果需要展示
依赖 context 或省略 host 的短片段，应使用普通 Markdown fenced code，并
明确它不是独立 CI 示例；不要把含伪函数或省略号的代码复制到
`docs/examples/`。
