+++
title = "测试与验收"
description = "silex_dom backend 与 silex_view mount 的 native、SSR、trybuild 和 wasm/browser 验收分层。"
weight = 60
+++

# 测试与验收

阶段 6 的测试必须区分 backend、View kernel 和真实 browser runner；通过
`--no-run` 只能证明 wasm 测试二进制生成成功，不能标记为 browser 通过。

## native / SSR

```text
RUSTFLAGS='-D warnings' cargo test --locked -p silex_dom \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test --locked -p silex_view \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo test --locked -p silex_view --test ssr_mount
```

覆盖包括确定性 serialization、文本/属性转义、void/SVG、property omission、
SSR event omission、hydration record、NodeRef set/clear、builder rollback、
retry/poison、动态属性清理和 keyed identity。

## trybuild 与文档

`crates/silex_view/tests/compile_fail.rs` 负责 scope escape；不要使用
`TRYBUILD=overwrite`。文档示例位于 `docs/examples/`，高层 View 示例由
`crates/silex_view/tests/docs_examples.rs` 编译；站点检查：

```text
RUSTFLAGS='-D warnings' cargo test --locked -p silex_view --test compile_fail
zola --root docs check
```

## wasm-testing 分层

严格遵循 [`wasm-testing.md`](@/developer/wasm-testing.md)：

1. `cargo check --target wasm32-unknown-unknown` 只记录 compile check；
2. `cargo test --target wasm32-unknown-unknown --no-run` 只记录 binary generation；
3. `cargo test --target wasm32-unknown-unknown --test <name>` 才是 Firefox runner；
4. nightly `wasm-test-nightly` 只用于 `panic=unwind`/build-std 验收。

browser target 实际运行时必须记录 `firefox`、`geckodriver`、
`wasm-bindgen-test-runner` 的探测结果、runner 命令和环境变量。无法运行时要
记录具体缺少的 binary、端口或 sandbox 原因，不能用 `--no-run` 代替。

## 回归清单

- SSR：markup deterministic、event omission、hydration record target identity；
- View：NodeRef active/rollback/dispose、owner cleanup、rollback retry；
- API：无 DOM crate 内的 View、高层 element/mounted/helpers 或旧顶层 attribute
  facade，
  `legacy-browser` 或 `legacy_timer`；
- feature：`silex_dom`/`silex_view` 的 SSR check 不激活 web runtime；
- wasm：check、no-run、真实 Firefox runner、nightly build-std 分开记录。
