+++
title = "silex"
description = "Silex 面向应用层的统一 facade、控制流组件和 UI 组件入口。"
template = "section.html"
sort_by = "weight"
+++

# `silex`

`silex` 是 Silex 面向应用层的统一 facade。它不实现独立的响应式 runtime、DOM
挂载事务或 HTML 代码生成，而是把这些 crate 组合到一个依赖入口中，并补充
应用最常用的控制流、错误边界、Portal、Suspense 和 Tailwind UI 组件。

它在架构中的位置如下：

```text
应用组件 / 页面
        │  silex::prelude、silex::components、silex::flow
        ▼
      silex facade
  feature-gated re-export · UI · 组合组件
        │
        ├── silex_core    owner、响应式值、错误和异步状态
        ├── silex_dom     View、属性、挂载和清理
        ├── silex_html    HTML/SVG 标签函数与属性 facade
        ├── silex_css     类型化 CSS、主题和样式 runtime
        ├── silex_router  路由 API
        └── silex_macros  component、CSS、Tailwind 等过程宏
```

因此，应用可以从 `silex` 开始编写页面，但遇到 scope、owner、DOM rollback、
错误聚合或宏展开问题时，应回到对应底层 crate 的契约。`silex` 的 re-export
不会改变这些类型的生命周期或清理语义。

## 稳定入口

| 入口 | 用途 |
| --- | --- |
| `silex::prelude` | 应用层常用的 core、DOM、HTML、CSS、router、宏、components 和 flow 导入。 |
| `silex::components` | `ErrorBoundary`、`Portal`、`Suspense` 以及 CSS feature 下的布局组件。 |
| `silex::flow` | `Show`、`Dynamic`、`Switch`、`Index`、`For` 和 `ForStateful`。 |
| `silex::ui` | Tailwind feature 下的 shadcn 风格 UI 组件和主题辅助函数。 |
| `silex::core` / `silex::html` / `silex::css` | 直接访问被 facade 包装的高层 crate；低层 DOM 请直接使用 `silex::dom` 分组入口。 |
| `silex::macros` | 过程宏 crate 的显式命名空间；常用宏也在 `prelude` 中。 |
| `silex::router` / `silex::hash` | 路由与哈希工具的 facade。 |
| `silex::bootstrap` | `bootstrap` feature 下的应用宿主和浏览器入口。 |
| `silex::persist` / `silex::net` / `silex::i18n` | 对应 feature 下的持久化、网络和国际化 API。 |
| `silex::reexports` | `web_sys`、`wasm_bindgen`、`wasm_bindgen_futures` 等宏和应用常用依赖。 |

通常应用只需要：

```rust
use silex::prelude::*;
```

UI 组件不通过 `prelude` 的 glob 导出，以免和 flow 中的 `Switch` 等名称发生
歧义；应显式写 `use silex::ui::{Button, Dialog};`。`silex::prelude` 中的
`Center`、`Switch`、CSS helper 和 `Link` 是经过显式消歧后的稳定导出。

## 最小可编译流程

下面的示例只创建 owner、context、signal 和 view factory，没有假设 native 环境
存在浏览器 DOM。页面展示的源文件就是测试编译的源文件，不在 Markdown 中维护
第二份 Rust 代码。

{% set source = load_data(path="examples/silex/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

示例由 `crates/silex/tests/docs_examples.rs` 编译并执行。真正的浏览器挂载还需要
`silex_bootstrap::AppHost`/`BrowserBootstrap` 或 `silex_view::app::MountedApp`；native
示例不能证明浏览器 DOM、CSSOM 或事件行为。

## 典型调用链与生命周期

一个应用页面的职责分层如下：

```text
Runtime::new()
└── Runtime::owner() / with_transient(...)
    └── OwnerAccess<'scope>
        ├── ErrorHandlerToken<'scope>
        ├── SilexContext<'scope>
        ├── signal / computed / Resource / Callback
        └── component builder → View<'scope>
            └── MountContext / MountOwner → DOM、effect、listener、cleanup
```

- `silex` 只负责组合这些能力；`Runtime`、`OwnerAccess`、`SilexContext` 和
  `SilexError` 的具体约束见 [`silex_core`](@/developer/crates/silex_core/_index.md)。
- `View` 是 owner-bound 的可重复 mount 描述，不是已经挂载的 DOM 节点。属性、
  event listener、动态 view 和 rollback 由 [`silex_dom`](@/developer/crates/silex_dom/_index.md)
  管理。
- 应用级 target、`MountedApp`、host 状态和 page lifecycle 由
  [`silex_bootstrap`](@/developer/crates/silex_bootstrap/_index.md) 管理；
  `silex::bootstrap` 只是 feature-gated re-export。
- owner-bound 的 signal、callback、view 和 handler 不能逃逸 `'scope`。runtime
  使用单线程的 `Rc`/`RefCell`/`spawn_local` 模型，不应把 facade 句柄当作
  `Send + Sync` 的跨线程状态。

## Feature flags

`crates/silex/Cargo.toml` 中的 feature 同时控制 facade 的命名空间和底层 crate
的对应能力：

| Feature | 增加或启用的能力 |
| --- | --- |
| 默认 `css`、`tw` | CSS facade；Tailwind 宏和 `silex::ui`。`tw` 会同时启用 `css`。 |
| `bootstrap` | `silex::bootstrap`，并启用 `silex_bootstrap/all`。 |
| `persistence` | `silex::persist`、持久化错误，以及 `silex_net` 的 persist 支持。 |
| `json` | `serde`/`serde_json`、persist 和 net 的 JSON 能力；不单独开启 persist 或 net facade。 |
| `net` | `silex::net` 和网络错误。 |
| `i18n` | `silex::i18n` 和国际化错误。 |
| `i18n-json` | `i18n` 加 JSON catalog 支持。 |
| `i18n-persist` | `i18n`、`persistence` 和 i18n 持久化。 |
| `i18n-browser` | `i18n` 加浏览器 locale 检测。 |
| `i18n-intl` | `i18n` 加 Intl facade 和错误类型。 |
| `i18n-macros` | `i18n` 加国际化过程宏。 |

关闭 `tw` 后，`silex::ui` 模块不会编译；关闭 `bootstrap`、`net`、`persistence`
或 `i18n` 后，对应 facade 命名空间也不会存在。feature 组合的错误类型和
底层细节分别见 [`silex_core` 错误文档](@/developer/crates/silex_core/errors.md)、
[`silex_net`](@/developer/crates/silex_net/_index.md) 和
[`silex_i18n`](@/developer/crates/silex_i18n/_index.md)。

## 错误、清理和并发边界

`silex` 暴露 `SilexError`、`SilexResult`、`ErrorHandler`、`ErrorReporter` 和
`ErrorHandlerToken`，但不吞掉底层错误：

- component builder、signal promotion、属性应用和 DOM mount 的 `Result` 应由
  调用方传播或交给当前 owner 的 error handler；示例不应用 `unwrap` 掩盖这些
  路径。
- `ErrorBoundary` 只处理其 boundary handler 收到的 recoverable error；fallback
  构造或挂载失败会转给父 handler。详见[组件边界](components.md)。
- Portal 创建的容器、Suspense 的内容和 UI 组件中的 timer/listener 都绑定当前
  mount owner；关闭 owner 后，它们不能继续调用用户 callback。
- `SilexError` 的 recoverable/fatal 级别不等同于 Rust panic。组件 factory 的
  panic 处理是 `ErrorBoundary` 的局部行为，不能把它当作全局 panic recovery。
- facade 本身不提供线程同步或全局 singleton。跨 runtime 的 tracked 读取、DOM
  cleanup report 和异步取消语义应按底层 crate 的文档处理。

## 专题

- [控制流与列表](@/developer/crates/silex/flow.md)：响应式条件、动态 view、
  keyed/indexed list、row identity 和重复 key 错误。
- [组合组件与生命周期](@/developer/crates/silex/components.md)：错误边界、
  Portal、Suspense 和 CSS 布局组件。
- [Tailwind UI 与主题](@/developer/crates/silex/ui.md)：UI 组件、context、
  signal/callback props 和 shadcn 主题。
- [测试与调试](@/developer/crates/silex/testing.md)：facade 导出、native、
  wasm、trybuild、浏览器和文档示例测试。
- [`silex_core`：owner 与响应式边界](@/developer/crates/silex_core/lifecycle.md)
- [`silex_dom`：View、挂载与清理](@/developer/crates/silex_dom/views.md)
- [`silex_macros`：component 与 PropsBuilder](@/developer/crates/silex_macros/component.md)

## 源码与测试索引

- facade 入口：`crates/silex/src/lib.rs`
- 控制流：`crates/silex/src/flow.rs`、`src/flow/`
- 组合组件：`crates/silex/src/components.rs`、`src/components/`
- Tailwind UI：`crates/silex/src/ui.rs`、`src/ui/`
- facade feature 与依赖：`crates/silex/Cargo.toml`
- 文档示例：`docs/examples/silex/basic.rs`
- 文档示例测试：`crates/silex/tests/docs_examples.rs`
- facade/type export 测试：`crates/silex/tests/bootstrap_facade.rs`、
  `tests/error_handler_alias.rs`
- flow 编译期测试：`crates/silex/tests/for_children.rs`、`tests/ui/`
- CSS facade 与宏测试：`crates/silex/tests/css_math_macros.rs`、
  `tests/css_value_check.rs`
- browser 组件测试：`crates/silex/tests/error_boundary.rs`、`tests/portal.rs`

## 已知限制与维护注意

- `silex::prelude` 是 convenience facade，不代表所有 crate 的全部 API 都在
  glob 中；需要稳定区分模块时优先使用显式路径。
- UI 组件是带响应式 props 的 view 组合，默认样式依赖 Tailwind 生成结果和
  shadcn CSS 变量；使用 UI 前应确认 `tw` feature，并按页面需要注入
  `silex::ui::inject_shadcn_base_styles()`。
- `Dialog`、`Popover`、`Tooltip` 等组件把内容放到 Portal 或根据 signal 改变
  DOM；测试应观察最终 DOM/owner 状态，不应假设固定的 microtask 数量。
- 关闭 `tw` 或某个 facade feature 会改变可见的模块和 prelude 导出，这是编译期
  API 变化；feature matrix 的维护必须和 `Cargo.toml` 同步。
- 不在本 crate 文档中重复底层 crate 的完整属性、资源、runtime 或 bootstrap
  契约；对应行为变化时，应同时检查本页的 facade 链接和底层专题。

验证本 crate 文档和公开 facade 变更时，至少运行 `cargo fmt --all -- --check`、
`RUSTFLAGS='-D warnings' cargo check -p silex`、对应的
`RUSTFLAGS='-D warnings' cargo test -p silex --test docs_examples` 和
`zola --root docs check`。涉及 wasm 组件或 trybuild 时，再运行本页测试索引中
对应的目标测试；不需要为新增文档示例执行整个 workspace 测试。
