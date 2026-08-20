+++
title = "silex_core"
description = "Silex 面向组件和框架生命周期的高层响应式 facade。"
template = "section.html"
sort_by = "weight"
+++

# `silex_core`

`silex_core` 是 Silex 应用层与 `silex_reactivity` 运行时之间的高层 facade。它不重新实现响应式图，而是把底层的 owner、节点、计算和调度包装成带有 `SilexError`、组件上下文、异步任务、资源状态和便捷 trait 的 API。DOM、路由、网络、国际化等上层 crate 可以共享这套生命周期和错误边界。

它主要解决三类问题：

- 让组件通过显式的 `OwnerAccess<'owner>` 创建 signal、computed、effect、watch、callback 和 cleanup；
- 让异步工作（`TaskHandle`、`Resource`、`Mutation`）跟随 owner 关闭，而不是由全局 runtime 或线程局部状态管理；
- 把底层 `ReactiveError`、用户错误、JavaScript 错误和可选领域错误归一到 `SilexError`，同时保留可恢复/致命级别。

## 在 Silex 架构中的位置

```text
应用组件 / DOM / Router / Bootstrap
              │
              ▼
        silex_core facade
  OwnerAccess · SilexContext · SilexError
  signal · effect · Resource · Mutation · TaskHandle
              │
              ▼
      silex_reactivity runtime
  owner tree · dependency graph · scheduler · cleanup
```

`silex_core` 的公开句柄仍然携带 owner 的 Rust 生命周期。句柄是 `Copy` 的能力值，但它们不是脱离作用域的资源所有者；owner 关闭后，继续调用句柄会返回 `SilexError::Fatal(SilexErrorKind::Reactivity(...))`，通常具体为 `ReactiveError::NoSuchNode`。

## 稳定入口与核心类型

| 入口 | 主要用途 |
| --- | --- |
| `Runtime` | 创建一个显式、单线程的运行时；提供 root owner 或 transient scope。 |
| `OwnerHandle` | 持有持久 owner 的 close 权限；通过 `access`/`with_access` 借出 `OwnerAccess`。 |
| `OwnerAccess<'owner>` | 创建和操作 owner 内的所有公开节点、handler、cleanup 与异步任务。 |
| `ReadSignal` / `WriteSignal` / `RwSignal` | 分离读写能力，或以一个值同时携带两种能力。 |
| `Computed` / `EffectHandle` / `WatchOptions` | 创建派生值、副作用和 watcher。 |
| `Rx<'scope, T>` / `Signal<'scope, T>` | 在 signal、computed、stored value 之间统一传递只读值。 |
| `Resource` / `Mutation` | 将异步读取或异步变更表示为可观察状态。 |
| `StoredValue` / `NodeRef` / `Callback` | 分别保存非响应式状态、宿主对象引用和类型化回调。 |
| `SilexContext` | 将 owner 与 `ErrorReporter` 一起传给组件或宏。 |
| `SilexError` / `SilexResult` | 统一错误模型和 `Result<T, SilexError>` 别名。 |

应用代码通常从 `silex_core::prelude` 导入这些类型和 trait。需要保持模块边界清晰时，也可以从 `owner`、`reactivity`、`traits`、`logic` 或 crate 根逐项导入。

## 生命周期与并发边界

一次典型调用的层次如下：

```text
Runtime
├── OwnerHandle（持久 root 或持久 child，拥有 close 权限）
│   └── OwnerAccess<'owner>（短期借用能力）
│       ├── reactive nodes / handlers / cleanups
│       └── scoped tasks / completion endpoints
└── with_transient(...)（回调结束时自动关闭）
```

- 一个 `Runtime` 同时只能有一个活动的 root owner。重复调用 `Runtime::owner()` 会得到 `RuntimeAlreadyRunning`，关闭 root 后才可以重新创建。
- `Runtime::with_transient` 和 `OwnerAccess::with_transient` 用高阶生命周期限制句柄逃逸；异步任务、回调和节点不能把 transient 的借用带到回调之外。
- `OwnerHandle::create_child` 创建可显式关闭的子树。关闭 owner 会清理其子节点、handler、cleanup、completion 和 owner 绑定的任务。
- runtime 使用 `Rc`、`Cell`、`RefCell` 和 `spawn_local`，因此是单线程模型，不应把这些句柄当作 `Send + Sync` 的共享状态。
- 同一 runtime 内的 owner 可以建立 tracked 依赖；不同 runtime 之间的 tracked 读取会返回 `ReactiveError::RuntimeMismatch`。跨 runtime 只读快照只能使用 `get_untracked`/`with_untracked`，且不会建立订阅。

## 最小可运行流程

下面的示例创建 signal、computed 和 effect，并通过 owner 自动关闭 transient scope。它来自 `docs/examples/silex_core/basic.rs`，不是页面中单独维护的一份代码。

{% set source = load_data(path="examples/silex_core/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

示例通过 `crates/silex_core/tests/docs_examples.rs` 编译并执行。代码把外层 `SilexResult` 转成示例边界的 `Box<dyn Error>`，没有用 `unwrap` 或 `expect` 隐藏 API 的错误路径。

## 公开模块地图

| 模块 | 责任 | 代表源码 |
| --- | --- | --- |
| `owner`（crate 根重新导出） | runtime、owner、节点注册、cleanup、task | `src/owner.rs`、`src/task.rs` |
| `reactivity` | signal、computed、effect、watch、promotion、resource、mutation | `src/reactivity/` |
| `traits` | `RxRead`、`RxWrite`、`RxGet`、`ReactiveInput`、`RxOptionExt` 等适配层 | `src/traits.rs` |
| `logic` | map、computed、比较和算术派生 | `src/logic/` |
| `error` | SilexError、handler 别名和 feature-gated 领域错误 | `src/error.rs`、`src/error/` |
| `context` | `SilexContext` 与 context provider 契约 | `src/context.rs` |
| `log` | 浏览器 console 与 native stdout/stderr 的统一宏 | `src/log.rs` |

## Feature flags

crate 默认不启用任何 feature。`test-support` 只转发到底层 runtime 的测试快照能力；领域错误 feature 只在需要对应上层模块时启用。

| Feature | 公开内容或作用 |
| --- | --- |
| `test-support` | 重新导出 `RuntimeSnapshot`，并开放 owner/access 的快照方法；不应作为应用运行时 API。 |
| `error-persistence` | `PersistenceError`、`PersistenceErrorKind`。 |
| `error-i18n` | `I18nError`、`I18nErrorKind`。 |
| `error-i18n-persistence` | 同时启用 `error-i18n` 与 `error-persistence`，并允许 i18n 错误携带 persistence 错误。 |
| `error-router` | 路径、路径参数和 route pattern 错误。 |
| `error-net` | `NetError`、连接状态和可重试判断。 |
| `error-intl` | `IntlError` 与 `IntlErrorKind`。 |
| `error-dom` | mount/dispose、cleanup report、rollback 和 drop 诊断类型。 |
| `error-bootstrap` | `AppHostError`、`BootstrapError`、host 状态；同时启用 `error-dom`。 |

## 专题

- [响应式值与派生](@/developer/crates/silex_core/reactivity.md)：signal、computed、effect、watch、promotion、trait 和宏。
- [owner 生命周期与异步边界](@/developer/crates/silex_core/lifecycle.md)：root/child/transient scope、cleanup、handler、task 和 completion。
- [Resource、Mutation 与异步状态](@/developer/crates/silex_core/async.md)：请求替换、过期结果、取消和 suspense 计数。
- [错误处理与 feature 边界](@/developer/crates/silex_core/errors.md)：`SilexError`、错误 handler、关闭错误和可选领域错误。
- [测试与调试](@/developer/crates/silex_core/testing.md)：集成测试、UI 编译期契约、浏览器测试和文档示例。

## 源码、示例与测试索引

- 公开 facade：`crates/silex_core/src/lib.rs`
- owner 与生命周期：`crates/silex_core/src/owner.rs`、`src/task.rs`
- 响应式 API：`crates/silex_core/src/reactivity/`
- trait 与输入适配：`crates/silex_core/src/traits.rs`
- 错误模型：`crates/silex_core/src/error.rs`、`src/error/`
- 文档示例：`docs/examples/silex_core/basic.rs`
- 文档示例测试：`crates/silex_core/tests/docs_examples.rs`
- 生命周期、runtime 兼容性和错误：`tests/root_scope.rs`、`tests/runtime_compatibility.rs`、`tests/reactivity_errors.rs`
- signal 读取与聚合：`tests/batch_read.rs`、`tests/tuple_traits.rs`、`tests/watch.rs`
- 异步资源、变更和 task：`tests/async_completion.rs`
- 编译期契约：`tests/compile_fail.rs` 与 `tests/ui/`

## 已知限制与维护注意

- `silex_core` 不提供全局 runtime、线程安全句柄或跨 runtime 的 tracked 依赖；上层框架必须把 `OwnerAccess` 显式传入创建 API。
- `Resource` 和 `Mutation` 使用请求 id 丢弃旧结果。旧请求不会因为结果过期而写入当前状态，但请求的生命周期仍由其 effect/owner cleanup 管理。
- `spawn_scoped` 内部将局部 future 暂时擦除为 `'static` 后交给 `spawn_local`。其安全性依赖 owner cleanup 在作用域释放前同步取消并释放 future；修改这条清理顺序时必须同时检查 `src/task.rs` 的不变量和异步测试。
- `ReadSignal::with_name` 与 `WriteSignal::with_name` 当前只返回自身，没有保存或暴露名称；它不能提供调试标签或性能诊断。
- `effect_detached`、`PersistentOwnerAccess`、`RxEffectKind` 以及部分内部宏是框架适配入口，不应被普通应用代码作为稳定生命周期 API 依赖。

验证本页或公开 API 变更时，至少运行 `cargo check -p silex_core`、`cargo test -p silex_core`、`cargo test -p silex_core --test docs_examples` 和 `zola check`；涉及 feature 或编译期契约时，追加 `--all-features` 与 `--test compile_fail`。
