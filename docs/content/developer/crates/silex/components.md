+++
title = "组合组件与生命周期"
description = "silex 的 ErrorBoundary、Portal、Suspense 和布局组件。"
weight = 20
+++

# 组合组件与生命周期

`silex::components` 是应用层组合器：它把 core 的错误 handler、异步状态和
owner 与 dom 的 View/branch/mount API 接起来。它不拥有独立 runtime；所有组件
都必须在调用方提供的 `SilexContextProvider<'scope>` 和 mount owner 中运行。

本页的 Rust 代码块是依赖外层 scope 的 API 契约片段，不是独立的 CI 编译示例；
可执行 facade 示例见总文档引用的 `docs/examples/silex/basic.rs`。

## `ErrorBoundary`

`ErrorBoundary(ctx, children)` 的 `children` 是
`Fn(Ctx) -> V`，而 `.fallback(|error| view)` 接收 `SilexError`。child factory
收到的 context 已替换为 boundary-local reporter，因此 child 在创建阶段注册
的 effect、callback 和 resource 可以直接把错误送到这个 boundary：

```rust
let view = ErrorBoundary(ctx, move |child_ctx| {
    RiskyView(child_ctx).build()
})
.fallback(|error| div(format!("failed: {error}")))
.build();
```

行为分为两个阶段：

1. 初始 child mount 返回 recoverable error 时，boundary 直接创建 fallback；
2. 已挂载 child 在 deferred flush 中报告 recoverable error 时，先关闭当前
   child branch/content owner，再挂载 fallback；同一 generation 的重复错误不会
   重复挂载 fallback。

boundary 用 branch key 区分 `Child(generation)` 和
`Fallback(generation)`。child 已经切换后，fallback 的构造或 mount 错误会交给
父 handler，并使当前 boundary 终止；错误不会再发回已失败的 child branch。child
factory 的 panic 会转换成 fatal JavaScript error，再进入 boundary 的切换流程。

使用时应注意：

- 通过传入的 `child_ctx.error_reporter()` 或 context 创建 owner-bound effect 和
  cleanup；不要把错误转发给 completion endpoint；
- fallback 也应传播 `Result`/mount error，不能在 fallback 内用 `unwrap` 隐藏；
- 协调测试应观察 DOM 或 owner cleanup 完成，不应依赖固定数量的 JavaScript
  microtask；
- boundary close 后 pending error 不应再挂载 fallback。

## `Portal`

Portal 有两个显式入口。`PortalHost` 只负责把子 view mount 到当前 DOM 树之外的
稳定 host；带 `open` 的 `Portal` 通过内部 visibility root 管理可见状态。默认
目标都是 `document.body`，`.mount_to(Some(target_node))` 可以指定其它目标：

```rust
let host = PortalHost(ctx)
    .children(dialog_view)
    .mount_to(Some(target_node.clone().into()))
    .build();

let diagnostic_attrs = PortalHostAttrs::new()
    .data("owner", "profile-dialog")?;
let modal = Portal(ctx, open)
    .children(dialog_view)
    .content_mode(PortalContentMode::KeepAlive)
    .host_attrs(diagnostic_attrs)
    .build();
```

Portal 的 DOM 结构固定为：

```text
body / mount_to target
└── div[data-portal-host]                 host
    └── div[data-portal-visibility-root]  private visibility root
        └── Portal children or content slot
```

host 是稳定的 `MountInstance` 节点，只负责挂载、诊断 marker 和保持
`display: contents`。visibility root 不接受用户 attrs，是唯一的可见性边界：

| 状态 | root `display` | `aria-hidden` | `pointer-events` | `data-state` |
| --- | --- | --- | --- | --- |
| open | `contents` | `false` | `auto` | `open` |
| closed | `none !important` | `true` | `none` | `closed` |

关闭时依赖 root 的 `display: none !important`，不依赖 host 的 `hidden` 属性或
UA stylesheet。`PortalHost` 的 root 初始状态固定为 open；带 `open` 的 Portal
只更新 root，不删除 host、overlay、wrapper 或 KeepAlive content。

普通 Portal attrs 仍写入 host，但 `hidden`、`aria-hidden`、`inert`、`data-state`
和 `style` 属于框架保留字段，会返回错误。需要写入 host 的诊断信息时，使用
`PortalHostAttrs` 的 `attr`、`data`、`class`、`id` 或 `title` 方法；该入口同样
拒绝保留字段。Portal children 的属性仍作用于 content 节点。

`PortalContentMode::KeepAlive`（默认）在关闭时保留内容 owner 和 DOM；
`PortalContentMode::UnmountWhenClosed` 保留 host 和 slot，但在关闭时卸载内容，
再次打开时重新挂载。无论哪种模式，子 view 都由同一个 context、error reporter
和 owner 生命周期管理；owner cleanup 会移除 host，子 view mount 失败或 panic
时也会回滚已创建的节点。

不要把 Portal 放进 `Show`、`if open` 或其它响应式结构分支；这些分支会销毁
Portal owner，重新创建 host，并重新触发弹层内容的 mount。需要条件显示时，
应让带 `open` 的 Portal 始终存在，并通过 signal 控制 visibility root 状态。

默认目标需要浏览器 `document.body`。在 native 或 detached document 测试中，应
只构造 Portal view，或显式提供有效的 `Node`；不要把 native 构造测试当作真实
body mount 的证明。

## `Suspense`

`Suspense(ctx, children)` 创建一个 `SuspenseContext`，并把它传给 children
factory。factory 中用该 context 注册的 `Resource` 会增加 pending count，
从而控制 fallback：

```rust
let view = Suspense(ctx, move |suspense_ctx| {
    load_view(suspense_ctx)
})
.fallback(div("Loading..."))
.build();
```

`SuspenseMode` 有两个值：

| 模式 | pending 时 | 完成后 |
| --- | --- | --- |
| `KeepAlive`（默认） | 保留 content DOM，只把 content 隐藏并显示 fallback。 | 重新显示原 content。 |
| `Unmount` | content view 变为空，只显示 fallback。 | 初次使用初始化 view，之后重新执行 children factory。 |

children factory 在组件初始化时会执行一次，使 Resource 绑定稳定的组件作用域。
`Unmount` 模式在后续重新显示时才重新执行 factory；这不是对 view factory 的
通用缓存保证，具体 resource 生命周期仍由 owner 和 core suspense context 管理。

## CSS 布局组件

`Stack`、`Center`、`Grid` 位于 `components/layout.rs`，仅在 `css` feature 下
编译。它们通过 `styled!` 生成带响应式 CSS props 的 view：

| 组件 | 主要 props | 默认行为 |
| --- | --- | --- |
| `Stack` | `direction`、`align`、`justify`、`gap`、`style` | `flex` column，stretch，flex-start。 |
| `Center` | `style` | flex 水平、垂直居中。 |
| `Grid` | `columns`、`gap`、`style` | grid，默认一列。 |

这些组件的 `style`、`gap` 等响应式输入仍由 context owner 创建；CSS runtime 和
清理规则见 [`silex_css`](@/developer/crates/silex_css/_index.md)。`Center` 在
facade prelude 中有显式导出以解决 glob 名称冲突。

## 失败、清理和 owner 边界

组合组件的共同原则是“结构和资源一起提交”：

- branch、Portal 容器、fallback/content 和 listener 都在当前 mount owner 下
  注册 cleanup；
- partial mount 时 provisional owner 先关闭，失败后不能继续使用 detached
  child；
- 清理失败不会被 `Display` 字符串替代，生产代码应保留结构化
  `CleanupReport`/error handler 结果；
- component builder 的错误先在构造阶段返回，已经挂载后的错误由 owner-bound
  reporter 异步送达 boundary 或父 handler；
- owner 是单线程的，不能把 context、`AnyView<'scope>`、timer 或 callback
  放入 `'static` 的跨线程容器。

底层 branch、动态 view 和 mount rollback 的完整不变量见
[`silex_dom 生命周期`](@/developer/crates/silex_dom/lifecycle.md)、
[`silex_dom 视图`](@/developer/crates/silex_dom/views.md) 和
[`silex_core 生命周期`](@/developer/crates/silex_core/lifecycle.md)。

## 源码与测试

- 实现：`crates/silex/src/components/error_boundary.rs`、`portal.rs`、
  `suspense.rs`、`layout.rs`
- ErrorBoundary browser 契约：`crates/silex/tests/error_boundary.rs`
- Portal browser 契约：`crates/silex/tests/portal.rs`
- layout 宏展开：`crates/silex/src/components/layout.rs` 与
  [`silex_macros component`](@/developer/crates/silex_macros/component.md)
- DOM mount/rollback：[`silex_dom 挂载`](@/developer/crates/silex_dom/mounting.md)

浏览器测试必须在 wasm-bindgen test runner 中执行；native `cargo check` 只能
验证 facade 和类型边界，不能验证 `document.body`、DOM range、事件或 CSS 行为。
