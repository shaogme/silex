+++
title = "测试与调试"
description = "silex_router 的 native、trybuild、browser 和文档示例测试分层。"
weight = 60
+++

# 测试与调试

`silex_router` 将纯路径逻辑、宏生成契约和浏览器生命周期分层测试。这样
native 测试可以快速发现编码/匹配回归，trybuild 可以锁定编译期诊断，只有
依赖 `window`、DOM 和 History API 的部分才进入 browser runner。

## 测试分层

| 层级 | 位置 | 覆盖内容 |
| --- | --- | --- |
| path/context/link/table 单元测试 | 各 `src/*.rs` 的 `#[cfg(test)]` | 规范化、percent codec、base 边界、active class、matcher priority 和 nesting。 |
| route macro 集成测试 | `tests/routes_macro.rs` | 生成 enum 的 path/match/table、typed decode、fallback 和 child table。 |
| context 输入测试 | `tests/context_inputs.rs` | foreign runtime 的 read/write source 在 context 创建前被拒绝。 |
| trybuild | `tests/compile_fail.rs`、`tests/ui/` | 合法 route/组件 scope，以及参数、字段、nested prefix、旧 API 和 detached scope 的编译契约。 |
| browser integration | `tests/router.rs` | History 导航、`popstate`、query、layout/outlet、Link active、listener 和 owner cleanup。 |
| 文档示例 | `tests/docs_examples.rs` + `docs/examples/silex_router/basic.rs` | 页面展示的 route API 真实可编译，并执行无浏览器的最小流程。 |

## 常用验证命令

在仓库根目录执行：

```text
cargo fmt --all -- --check
cargo check -p silex_router
cargo test -p silex_router --test docs_examples
cargo test -p silex_router --test routes_macro
cargo test -p silex_router --test context_inputs
cargo test -p silex_router --test compile_fail
zola --root docs check
```

`docs_examples` 是新增/修改文档示例后的最小验证入口；它只编译并执行
`silex_router` 的示例，不要求运行 workspace 或其它 crate 的测试。完整的
`cargo test -p silex_router` 还会包含 native 单元测试和 trybuild；
`tests/router.rs` 带有 `wasm32` 条件，只在 browser runner 中实际执行。

matcher 相关示例必须把 `RouteEnum::compile()`、raw `RouteMatcher` 或
`RouteEnum::table(...)` 放在 setup 区域；循环、signal effect、outlet evaluation
和导航 handler 只借用已保存对象。这样测试能同时锁定初始化错误边界和 matcher
不在 pathname 热路径重复编译的契约。

## browser 测试边界

`tests/router.rs` 使用 `wasm-bindgen-test`，并验证这些资源的实际生命周期：

- `/app` base 下的 push、replace、popstate 与 logical path；
- 同一个 layout 在 child route 变化时只创建一次；
- nested outlet 组合 parent prefix 与 child path；
- `Link` 的显示 href 和 active class；
- query 的空值、重复 key、删除和响应式变化；
- owner close 后 popstate listener、route cleanup 和旧 branch 不再执行；
- listener 注册失败时不调用 route handler，也不继续 mount outlet；
- `RouterView` factory 重建时旧 dynamic owner 只清理一次。

浏览器测试中如果出现“文本没更新”，先区分三类问题：当前 URL 是否通过
`History`/`popstate` 更新、`RouterContext::path` 是否变化、matcher 是否有
候选并成功解析参数。若只在卸载后出现回调，检查 owner-bound listener 或
dynamic branch cleanup，而不是添加 detached listener 绕过生命周期。

## trybuild 失败定位

修改 `router!` 解析或生成代码后，先运行 `cargo test -p silex_router --test
compile_fail`。如果 stderr 发生有意变化，应只更新对应的
`tests/ui/*.stderr`，并检查诊断仍然指出真正的 pattern、字段或 scope 位置。
重点保持这些契约：

- 动态 nested prefix 不能通过编译；
- wildcard 字段必须是 `PathTail`；
- route field 名称必须与 pattern 参数一致；
- `RouterContext`、事件和 route closure 不能逃逸为 `'static`；
- 已移除的旧 Router constructor 继续给出明确错误，而不是静默适配。

## 文档示例约束

`docs/examples/silex_router/basic.rs` 只使用不依赖 `window` 的 API，所以
`tests/docs_examples.rs` 可以在 native target 中执行。示例遵循以下约束：

- 所有主动返回错误的 path/macro/table 操作用 `?` 传播；
- typed matching 先保存 `RouteEnum::compile()?` 的结果，raw matching 先保存
  `RouteMatcher::from_patterns(RouteEnum::patterns())?` 的结果；
- 不使用 `unwrap`/`expect` 掩盖 `PathError`、`PathParamError` 或
  `RoutePatternError`；
- 页面通过 Zola `load_data(..., format="plain")` 读取源文件，避免 Markdown
  与 CI 中的 Rust 代码分叉；
- 如果以后示例需要挂载 `Router`、`Link` 或 DOM，必须加入相应的 wasm/browser
  测试，而不能在 native 示例中伪造 `window`。

## 相关源码

- 示例：[总文档](_index.md)中的可运行流程和 `docs/examples/silex_router/basic.rs`。
- 路径与 matcher：`src/path.rs`、`src/route_table.rs`。
- 浏览器行为：`tests/router.rs`。
- 编译期行为：`tests/compile_fail.rs` 和 `tests/ui/`。
