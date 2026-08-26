+++
title = "测试与验证"
description = "说明 silex_view 的 native/SSR、browser/Wasm、trybuild 和文档示例验证方式。"
weight = 80
+++

# 测试与验证

`silex_view` 的契约跨越 View mount、DOM backend、响应式 runtime 和 owner cleanup。
只验证 SSR HTML 不足以覆盖 browser listener、focus、property 类型和真实节点
identity；只验证 browser 也不能替代 SSR rollback、hydration record 和结构化错误
测试。

## 测试分层

| 位置 | 覆盖内容 |
| --- | --- |
| `src/**` 单元测试 | transaction 状态、owner close、NodeRef/row updater 等局部不变量。 |
| `tests/kernel.rs` | View dispatch、primitive/typed element、composite mount、动态 View、branch、indexed/keyed list 和 rollback。 |
| `tests/ssr_mount.rs` | SSR 序列化、属性/响应式更新、event record、NodeRef cleanup、mount retry 和 poison。 |
| `tests/browser.rs` | BrowserDom、真实 event dispatch、window listener、focus、property、bind value、DOM identity 和 list reconcile。 |
| `tests/ui/` | `MountBuilderContext<'scope>`、`MountDomAction<'scope>` 和 `NodeRef<'scope>` 逃逸的编译期失败。 |
| `tests/docs_examples.rs` | 编译并运行 `docs/examples/silex_dom/basic.rs`，验证最小 View 示例。 |

测试应检查可观察契约，例如 dispose 后 host 为空、event callback 不再触发、
NodeRef 已清空、foreign context 被拒绝、keyed row 顺序与 identity 正确；不要依赖
SSR 内部 node id、`Rc` 地址或 browser wrapper 的实现细节。

## Native 与 SSR 命令

在仓库根目录运行：

```text
cargo fmt --all -- --check
RUSTFLAGS='-D warnings' cargo check -p silex_view \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_view \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_view \
  --test kernel --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_view \
  --test ssr_mount --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_view \
  --test docs_examples --no-default-features --features ssr
cargo clippy -p silex_view --all-targets --all-features -- -D warnings
zola --root docs check
```

`browser` 是默认 feature，但 native `cargo check` 不能替代 Wasm 中的
`web_sys` 行为；SSR 集成测试必须显式启用 `ssr`，否则 required-features test
target 不会运行。

## Browser 与 Wasm

先检查 browser feature 和目标类型：

```text
RUSTFLAGS='-D warnings' cargo check -p silex_view \
  --target wasm32-unknown-unknown \
  --no-default-features --features browser
RUSTFLAGS='-D warnings' cargo test -p silex_view --test browser \
  --no-default-features --features browser \
  --target wasm32-unknown-unknown --no-run
```

有 `wasm-bindgen-test-runner`、浏览器和对应驱动时，再去掉 `--no-run` 执行
browser tests。至少应覆盖：

- `MountedApp` host 校验、mount/dispose 和重复 mount；
- element/window listener 的 dispatch、取消和 callback error handler；
- `MountDomAction` + `NodeRef` 的 browser focus、detached 行为与 owner close gate；
- attribute/property 分离、空 value、boolean property、class/style 更新和 `bind_value`；
- dynamic View、stable branch、indexed list、render-only keyed list、stateful keyed list；
- keyed duplicate/error/panic rollback、row updater generation 和多节点 row identity。

browser test 使用真实 DOM 节点 identity；如果测试依赖异步 runtime effect，应按
现有测试方式 flush browser tasks 后再断言，不要用固定 sleep 推断完成时间。

SSR-only 的 NodeRef focus `Unsupported`、`bind_window_event` 的 window listener
`Unsupported` 和 SSR hydration record 应在 `tests/ssr_mount.rs` 或对应
`silex_dom` SSR 测试中验证，不能列为 browser coverage。

## trybuild 作用域契约

`tests/compile_fail.rs` 运行 `tests/ui/fail_*.rs`。这些测试故意不能编译，配套的
`.stderr` 是 trybuild 输出契约；修改公开 lifetime 或 capability 时，需要同步
确认失败原因仍然是 scope escape，而不是新的无关编译错误。当前覆盖：

- `MountBuilderContext<'scope>` 不能返回为更长 lifetime；
- `MountDomAction<'scope>` 不能转换成 `'static`；
- `NodeRef<'scope>` 不能逃逸创建它的 scope。

## 文档示例

开发者文档总览引用的最小示例是
`docs/examples/silex_dom/basic.rs`。它在 Wasm 分支中创建 `BrowserDom`、选择
document body、创建 `MountedApp`、mount 一个 `Element`，再显式 dispose；native
分支保留一个不依赖 browser backend 的 compile smoke test。`tests/docs_examples.rs`
通过 `#[path = ...]` 复用同一个源文件，避免 Markdown 中复制一份会分叉的程序。

新增需要独立验证的 View 示例时，应放入 `docs/examples/` 并在对应 test target
中引入；Markdown 中只展示普通 API 片段时，明确说明省略的 context/错误处理和
验证方式。示例中的失败操作应使用 `?` 或显式 `match`，不要用 `unwrap`/`expect`
掩盖错误路径。

## 调试顺序与已知边界

1. 先确认 `DomContext` 与 host/node 的 backend identity；
2. 再检查 `MountError` 的 availability 和 rollback report；
3. 对动态内容检查 anchor range、row key/order、owner active 和 updater generation；
4. 对事件检查 HostResource、owner gate 和 SSR hydration record；
5. 对 cleanup 检查 NodeRef 是否先清除、listener 是否取消、boundary 是否仍有 parent。

`silex_view` 的 browser 测试依赖可用 Wasm/browser 工具链；在只运行 native SSR
测试时，必须把未覆盖的 browser 边界作为验证缺口记录，不要把 SSR 结果概括成
通用 browser 行为。当前仍需补充或持续关注的测试边界包括：

- `MountContext::on_commit` 经由 `MountedApp` 的集成提交行为；
- cleanup handler 失败、cleanup panic 与 poisoned 状态下 `dispose()` 的报告/幂等语义；
- `MountAncestry::closest_logical_element` 固定返回 `Unsupported` 的契约；
- 真实浏览器运行环境中的 hydration 重连，而不仅是 Wasm 测试编译。
