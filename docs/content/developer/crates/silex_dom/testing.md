+++
title = "测试与调试"
description = "silex_dom 的 native、SSR、Wasm 和文档示例验证方式。"
weight = 70
+++

# 测试与调试

`silex_dom` 的行为同时由 backend identity、opaque handle、节点树不变量、SSR
serializer、事件 lease 和 NodeRef generation 决定。只测试 HTML 字符串不足以证明
browser 与 SSR 的边界安全；新增 API 时应同时验证成功路径、结构化错误和清理。

## 测试分层

| 位置 | 覆盖内容 |
| --- | --- |
| `src/**` 单元测试 | `BackendId` 唯一性、logger、`HostResource` 取消门和 cleanup sink。 |
| `tests/node_ref.rs` | opaque binding、generation-aware cleanup、wrong kind、cross context 和 SSR focus 限制。 |
| `tests/ssr/tree.rs` | fragment、void element、namespace、range、移动、cycle、detached 和 context identity。 |
| `tests/ssr/attributes.rs` | attribute/property 分离、HTML 转义、style property 和确定性序列化。 |
| `tests/ssr/events.rs` | event record、hydration metadata、bridge 不执行和 listener lease 取消。 |
| `silex_view/tests/browser.rs` | BrowserDom、真实 document、focus、element/window event 和 listener cleanup 的 Wasm 场景。 |
| `silex_view/tests/ui/` | `NodeRef<'scope>` 逃逸等由上层 scope 契约约束的编译期失败。 |
| `docs/examples/silex_dom/ssr.rs` | 本 crate 文档总览使用的可执行 SSR 流程。 |

测试名应描述可观察契约，例如
`cross_context_and_wrong_parent_operations_are_structured_errors`，不要依赖 SSR
内部 node id、BTree slot 或 browser wrapper 地址。若契约只属于一个 backend，测试
名称和说明应明确标出 `ssr` 或 `browser`，不要把单 backend 行为概括成通用 DOM 规则。

## 常用 native/SSR 命令

在仓库根目录运行，所有命令都把 warning 视为 error：

```text
cargo fmt --all -- --check
RUSTFLAGS='-D warnings' cargo check -p silex_dom
RUSTFLAGS='-D warnings' cargo check -p silex_dom \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_dom \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test -p silex_dom --test docs_examples \
  --no-default-features --features ssr
cargo clippy -p silex_dom --all-targets --all-features -- -D warnings
zola --root docs check
```

SSR 集成测试依赖 `ssr` feature；默认 `cargo test -p silex_dom` 不会运行
`node_ref`、`ssr` 和 `docs_examples` 三个 required-features test target。

## Browser 与 Wasm 边界

browser feature 的 native check 只能验证 Rust 类型和 feature 组合，不能替代
浏览器中的 `web_sys` 行为。至少执行：

```text
RUSTFLAGS='-D warnings' cargo check -p silex_dom \
  --target wasm32-unknown-unknown \
  --no-default-features --features browser
RUSTFLAGS='-D warnings' cargo test -p silex_view --test browser \
  --no-default-features --features browser --target wasm32-unknown-unknown --no-run
```

有浏览器、`wasm-bindgen-test-runner` 和对应驱动时，再去掉 `--no-run` 执行
browser tests。完整的工具链、Firefox 和 build-std 说明见
[Wasm 测试指南](@/developer/wasm-testing.md)。

browser 测试应覆盖：

- 不同 `BrowserDom` context 的节点混用被拒绝；
- text、element、document、fragment 的能力边界；
- void element、fragment 和 range 的真实 DOM 行为；
- `capture`、`once`、`passive` 与 window listener 的安装/取消；
- input value、pointer、keyboard、rect、prevent default 和 focus control；
- listener `HostResource` 的显式 cancel、drop cancel、取消错误和重复 cancel；
- bridge dispatch 错误被上层 handler 捕获，而不是依赖 `listen()` 返回。

## 文档示例

页面通过 Zola 的 `load_data(..., format="plain")` 读取
`docs/examples/silex_dom/ssr.rs`。测试通过如下模块路径复用同一份源文件：

```rust
#[path = "../../../docs/examples/silex_dom/ssr.rs"]
mod ssr;

#[test]
fn ssr_documentation_example_runs() {
    assert!(ssr::run().is_ok());
}
```

不要在 Markdown 页面复制一份会与示例文件分叉的完整 Rust 程序；只在需要说明
局部 API 时使用普通 fenced code，并明确它不是 CI 示例。示例中的每个 DOM 操作
都应处理 `DomResult`，不要用 `unwrap` 或 `expect` 把错误路径隐藏起来。

## 调试顺序

1. 先检查 backend id、node identity、node kind 和底层节点是否仍有效。
2. 再检查 parent/reference/range 的归属和当前树顺序；失败后重新读取 children。
3. 区分 `Unsupported`、`CrossContext`、`WrongNodeKind`、`Detached`、`NoParent`
   和 backend JavaScript error，不要只看格式化后的字符串。
4. 如果涉及 NodeRef，记录 logical state 和 generation，确认旧 binding cleanup
   是否被 `AlreadyReplaced` 正确忽略。
5. 如果涉及 dispose/drop，分别检查 `CleanupReport`、`DropFailureReport` 和
   `HostResourceState`，确认取消动作是否真的执行。

## 已知测试缺口

当前 `silex_dom` 集成测试主要覆盖 SSR；browser 适配器的事件和 host resource
行为由上层 `silex_view` 的 browser test 间接覆盖。修改 `BrowserDom`、listener
callback 或 `DomBackend` 默认能力时，不能只运行 native SSR 测试。

源码与测试入口：`src/runtime/`、`src/adapters/`、`src/lifecycle/`、
`tests/node_ref.rs`、`tests/ssr/`、`crates/silex_view/tests/browser.rs`。
