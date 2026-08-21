+++
title = "测试与调试"
description = "silex facade 的 native、wasm、trybuild 和文档示例验证边界。"
weight = 40
+++

# 测试与调试

`crates/silex` 的测试分成 facade 编译契约、native API smoke test、trybuild 和
wasm browser 行为测试。测试目标不同，不能用一个 native `cargo test` 推断
DOM、事件、Portal 或 CSS runtime 已经正确。

## 测试分层

| 层级 | 入口 | 固定的契约 |
| --- | --- | --- |
| 文档示例 | `tests/docs_examples.rs` | `docs/examples/silex/basic.rs` 能在当前 facade feature 下编译并运行。 |
| native unit/integration | `tests/error_handler_alias.rs`、`css_math_macros.rs` | 顶层 re-export、错误 handler alias、CSS math macro 的可达性与类型调用点。 |
| trybuild pass/fail | `tests/for_children.rs`、`tests/css_value_check.rs`、`tests/ui/` | component render_fn 参数推导、CSS 静态值拒绝和 feature-gated namespace。 |
| wasm browser | `tests/error_boundary.rs`、`tests/portal.rs`、`tests/tw_tests.rs` | DOM mount、异步 error boundary、Portal cleanup、Tailwind macro 运行环境。 |
| 相关示例 | `examples/ui/tests/browser.rs`、`examples/error_boundary_demo/tests/browser.rs` | 真实组件组合、交互和应用级 host 行为。 |

## 文档示例测试

开发者页面通过 Zola `load_data(..., format="plain")` 读取
`docs/examples/silex/basic.rs`，测试通过 `#[path = "..."] mod basic` 复用同一
源文件。这样页面代码和编译测试不会各自演进。

示例当前只构造 `Runtime`、`SilexContext`、signal、`Show` 和 HTML view，不调用
浏览器 host。修改示例后执行：

```text
RUSTFLAGS='-D warnings' cargo test -p silex --test docs_examples
```

不要为了这个文档示例运行整个 workspace 或其它 crate 的测试。若示例开始调用
`document`、`MountedApp`、Portal 或 browser event，应把验证移动到 wasm 测试，
并为 native 分支保留仅类型/构造路径。

## Facade 与 feature 测试

feature-gated API 应同时有“打开”和“关闭”检查：

- `bootstrap_facade.rs` 在 `bootstrap` feature 下确认 `AppHost`、
  `BrowserBootstrap`、`JsAppHost` 等类型从 `silex::bootstrap` 导出；
- `bootstrap_facade_off.rs` 通过 `fail_bootstrap_facade_off.rs` 固定关闭 feature
  后命名空间不可用；
- `error_handler_alias.rs` 固定 `ErrorHandlerToken::view()` 可以作为
  `ErrorHandler`/`ErrorReporter` 使用；
- `css_math_macros.rs` 固定 `css_min!`、`css_max!`、`css_clamp!` 从顶层
  `prelude` 可用，并能在真实 property call site 通过类型检查。

修改 `lib.rs` 的 glob re-export 时，先检查命名冲突和 feature 条件，再更新这些
测试。特别是 `ui` 只在 `tw` feature 下编译，`bootstrap`、`net`、`persist` 和
`i18n` 也各自有 facade 边界。

## trybuild 与编译期诊断

`tests/for_children.rs` 使用 pass case 验证 `For`、`Index` 和 `ForStateful`
的 children 闭包可以推导出 item/index/updater 类型。`tests/css_value_check.rs`
使用 fail cases 验证 `css!` 在宏展开期拒绝未知 keyword、错误 property value
和多分量值。`fail_bootstrap_facade_off.rs` 验证 feature-gated import。

修改公开函数签名、`#[prop(render_fn(...))]` 参数或 feature export 时：

1. 先增加最小 pass/fail case；
2. 运行对应的 trybuild 测试，确认诊断指向调用点；
3. 只有错误契约确实变化时才更新 `.stderr`；
4. 不用宽松匹配或删除 fail case 掩盖 lifetime、类型或 feature regression。

## wasm browser 测试

仓库 `.cargo/config.toml` 将 wasm test 的 runner 配置为
`wasm-bindgen-test-runner`。browser 测试文件通过 `#![cfg(target_arch =
"wasm32")]` 隔离，在 browser 环境验证：

- ErrorBoundary 初始错误、deferred child 错误、fallback 错误、重复错误和 root
  close 期间的 pending error；
- Portal 重复 toggle 不会留下重复 modal 或 detached container；测试同时检查
  private visibility root 的 computed `display`、content 布局矩形、open/closed
  状态、关闭态 `elementFromPoint` 命中结果以及 host/root/content identity；
- Tailwind macro 产生非空 class；
- UI 示例中的 Dialog、Popover、Tooltip、Slider、Tabs 等真实交互。

这类测试应使用 DOM/owner 状态作为完成条件。`error_boundary.rs` 已提供
`wait_until_dom_text`、`wait_until_owner_closed` 和 `wait_until_condition` 等
轮询 helper；不要把“等待固定 N 个 microtask”写成组件契约。

Portal 浏览器回归位于 `crates/silex/tests/portal.rs`，应用级 UI 回归位于
`examples/ui/tests/browser.rs`。测试查询约定是：

```text
body > div[data-portal-host] > [data-portal-visibility-root]
```

不要用 host 的 `hidden` 属性或 host computed style 证明关闭态不可见；应读取
root 的 computed `display`，并检查 content 的布局矩形为零。Firefox 和
Chromium 都应执行这些测试；如果当前机器缺少某个浏览器，只能在进度记录中
标记待补充，不能把另一浏览器的结果扩大解释为完整覆盖。

## 调试顺序

遇到 facade 组件问题时，按以下边界定位：

1. 先确认 feature 和 target：`ui` 是否启用 `tw`，测试是否真的运行在 wasm
   browser；
2. 再确认 `SilexContextProvider`、owner lifetime 和 error handler 是否属于
   同一 runtime；
3. 对动态 flow 记录 source 值、key/generation、row range 和 owner close；
4. 对 Portal/Dialog/Popover/Tooltip 区分 caller target 被外部移除、session
   dispose 和 body mount 失败；
5. 对属性和控件区分 attribute、property、reactive plan 和 event listener；
6. 对错误边界查看 boundary handler、phase handler、parent handler 以及
   `CleanupReport`，不要只比较 `Display` 字符串；
7. 最后检查底层 crate 的失败回滚和 cleanup report，确认 facade 没有把主错误
   与清理错误混在一起。

## 验证命令

只修改 `crates/silex` 文档、示例或 facade 测试时，建议按以下顺序：

```text
cargo fmt --all -- --check
RUSTFLAGS='-D warnings' cargo check -p silex
RUSTFLAGS='-D warnings' cargo test -p silex --test docs_examples
zola --root docs check
```

修改 native facade 测试时追加目标 test；修改 wasm 组件时在已配置 browser runner
的环境追加对应测试；修改宏 UI 契约时追加 `cargo test -p silex --test for_children`
或 `--test css_value_check`。这些验证都应保持在目标 crate/文档范围内，除非
变更实际影响了 workspace 其它 crate。

## 源码索引

- facade：`crates/silex/src/lib.rs`
- feature：`crates/silex/Cargo.toml`
- 文档示例：`docs/examples/silex/basic.rs`
- 示例测试：`crates/silex/tests/docs_examples.rs`
- trybuild harness：`crates/silex/tests/for_children.rs`、`css_value_check.rs`、
  `bootstrap_facade_off.rs`
- browser harness：`crates/silex/tests/error_boundary.rs`、`portal.rs`、
  `tw_tests.rs`
- 真实应用交互：`examples/ui/`、`examples/error_boundary_demo/`
