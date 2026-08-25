+++
title = "silex_bootstrap"
description = "将应用级 DOM 挂载、页面生命周期和 JavaScript 所有权连接起来的宿主适配 crate。"
template = "section.html"
sort_by = "weight"
+++

# `silex_bootstrap`

`silex_bootstrap` 为已经由 `silex_dom` 描述的应用挂载提供宿主边界。它把一个
调用方提供的 `web_sys::Node` 与单个 `MountedApp` 绑定，统一处理 mount、replace、
unmount、回滚诊断和清理失败；可选模块再把这条边界连接到浏览器的页面事件或
JavaScript owner。

它不负责创建组件、生成 HTML 标签，也不提供全局应用入口。应用仍然通过
`MountContext` 构造和挂载 view；bootstrap 只负责“谁拥有这次挂载、何时结束它、
结束失败后能否重试”。

## 在 Silex 架构中的位置

```text
应用组件 / silex_html
          │ MountContext builder
          ▼
      silex_view::MountedApp
          │ 应用级 boundary、owner、rollback
          ▼
  silex_bootstrap::AppHost
      │             │
      │             ├── PageController / BrowserBootstrap
      │             │       └── pagehide / visibilitychange
      │             └── JsAppHost
      │                     └── JavaScript unmount API
      ▼
 caller-owned DOM Node
```

`AppHost` 的稳定核心依赖 `silex_core` 的 `Runtime`、`SilexError` 和
`silex_view` 的 `MountedApp`/`MountContext`。因此本 crate 的文档只说明宿主层
新增的状态和所有权；staging boundary、view owner、属性和 DOM 清理的底层
语义见 [`silex_dom` 挂载事务](@/developer/crates/silex_dom/mounting.md) 与
[`silex_dom` 生命周期](@/developer/crates/silex_dom/lifecycle.md)。

## 稳定入口与核心类型

| 入口 | 作用 |
| --- | --- |
| `AppHost` | 持有 caller-owned target，并管理一个 active `MountedApp`。 |
| `HostState` | 表示 `Ready`、`Mounting`、`Active`、`Disposing` 或 `Poisoned`。 |
| `UnmountOutcome` | 区分实际 dispose 的 `Disposed` 与幂等空操作 `AlreadyUnmounted`。 |
| `AppHostError` | 表示重复挂载、无 active app、挂载/清理失败、重入和 poisoned 状态。 |
| `PageController` | 在 `AppHost` 上增加可移除的页面生命周期监听；需要 `page-controller`。 |
| `PageLifecyclePolicy` | 选择 `Manual`、`PageHide` 或隐藏文档下的 `PageHideAndVisibilityChange`。 |
| `BrowserBootstrap` | 解析 browser target 并组合 `PageController`；需要 `browser-bootstrap`。 |
| `JsAppHost` | 将已配置的 Rust host 作为 opaque JavaScript 对象；需要 `js-object`。 |
| `BootstrapError` | 统一包装 host、target、lifecycle 和 listener 错误。 |

应用若通过 facade crate 使用 bootstrap，需要启用 `silex` 的 `bootstrap`
feature；该 feature 会启用本 crate 的 `all` feature，并在 `silex::bootstrap`
下重新导出这些类型。

## 最小可运行流程

下面的源文件保存在 `docs/examples/silex_bootstrap/basic.rs`。native 分支只
验证示例边界可编译；wasm 分支创建一个 detached target，使用 `AppHost` 挂载
一个 view，再显式 unmount。页面读取的就是这一个源文件，不在 Markdown 中
复制第二份 Rust 示例。

{% set source = load_data(path="examples/silex_bootstrap/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

对应测试入口是 `crates/silex_bootstrap/tests/docs_examples.rs`。如果运行
wasm browser runner，示例会执行真实的 mount/unmount；native 测试只执行无
浏览器分支。

## `AppHost` 的生命周期与并发边界

一次 host 操作的层次如下：

```text
AppHost
└── MountedApp
    ├── Runtime::owner() → root OwnerHandle
    ├── MountContext<'scope> → view / DOM owner / effects
    └── MountBoundary → caller-owned target 中的已提交节点
```

- `AppHost::new` 只保存 target、`CleanupSink` 和 `Ready` 状态，不会创建 root
  owner 或 DOM boundary。
- `mount(runtime, builder)` 在 builder 成功且 boundary 提交后才发布 active
  app。active host 再次调用 `mount` 会返回 `AlreadyMounted`。
- `replace` 要求当前为 `Active`，先完整 dispose 旧 app；旧 app 清理失败时
  不会调用新 builder，host 进入 `Poisoned`。新 builder 失败但回滚干净时，
  host 回到 `Ready`，旧 app 已经不再存在。
- `unmount` 只关闭当前 root 并移除本次 boundary。没有 active app 时返回
  `AlreadyUnmounted`；显式调用仍是推荐的错误可见清理路径。
- `is_active` 反映 host/session 和 root owner 状态，不检查 target 是否仍挂在
  document 中。caller 外部移除 target 后仍应调用 `unmount`。
- runtime、owner、DOM callback 和 host listener 都是单线程能力，不能把这些
  句柄当作 `Send + Sync` 的跨线程状态。

builder 使用以下高阶生命周期签名：

```rust
for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>
```

`MountContext`、`OwnerAccess`、`MountOwnerToken`、owner-bound view 和错误
handler 不能逃逸 builder 的 scope。需要创建 signal、handler 或挂载 view 时，
应在 callback 内通过 `context.access()` 和 `context.mount(...)` 完成；不要把
它们写入 `'static` 容器或交给页面级 callback。

## Feature flags

| Feature | 增加的公开能力 |
| --- | --- |
| 默认（无 feature） | `AppHost`、`AppHostError`、`HostState`、`UnmountOutcome` 和 `BootstrapError`。 |
| `js-object` | `JsAppHost`、`bootstrap_error_to_js`；增加 `wasm-bindgen` 与 `js-sys`。 |
| `page-controller` | `PageController`、`PageLifecyclePolicy`、`LifecycleReporter`。 |
| `browser-bootstrap` | 同时启用 `js-object` 与 `page-controller`，提供 `BrowserBootstrap`。 |
| `all` | 启用 `browser-bootstrap`。 |

这些 feature 只控制适配层是否编译进 crate；真正访问 `window`、`document`、
DOM 事件或 JavaScript 对象仍需要 wasm/browser 环境。native 构建适合检查
核心类型、错误模型和编译期 scope 契约。

## 错误和清理原则

bootstrap 层必须保留“主操作失败”和“回滚/清理失败”这两个来源：

- `AppHostError::Mount(error)` 中的 `MountError::primary()` 是 builder、view
  或 DOM 操作的主错误；`rollback()` 保存 root、provisional owner 和
  boundary 的清理报告。
- `AppHostError::Dispose(error)` 中的 `DisposeError::report()` 保存 dispose
  阶段的 `CleanupReport`。
- clean rollback 的 mount error 可重试；rollback 不干净、dispose 失败、
  重入或 panic 会让 host 进入 `Poisoned`，后续 mount 不应继续尝试。
- `CleanupSink` 用于 `Drop` 无法返回 `Result` 的诊断。需要调用方立即知道
  清理是否成功时，应调用 `unmount`，不要只依赖 Drop。

不要只记录 `Display` 文本。生产宿主应按 `mount_error()`、`dispose_error()`、
`MountError::availability()`、`CleanupReport::cleanup_failures()` 和
`boundary_errors()` 分别记录结构化原因。

## 专题

- [host 挂载、替换与错误](@/developer/crates/silex_bootstrap/hosting.md)：`AppHost` 状态机、回滚、重入和清理所有权。
- [页面生命周期与浏览器入口](@/developer/crates/silex_bootstrap/lifecycle.md)：`PageController`、`BrowserBootstrap` 和 listener ownership。
- [JavaScript owner 边界](@/developer/crates/silex_bootstrap/javascript.md)：`JsAppHost`、结构化错误对象和 raw pointer 不变量。
- [测试与调试](@/developer/crates/silex_bootstrap/testing.md)：native、wasm、UI scope 和文档示例验证。

## 源码、示例与测试索引

- crate 入口：`crates/silex_bootstrap/src/lib.rs`
- 应用 host：`crates/silex_bootstrap/src/app_host.rs`
- 页面生命周期：`crates/silex_bootstrap/src/page_controller.rs`
- 浏览器入口：`crates/silex_bootstrap/src/browser_bootstrap.rs`
- JavaScript 适配：`crates/silex_bootstrap/src/js_object.rs`
- 文档示例：`docs/examples/silex_bootstrap/basic.rs`
- 文档示例测试：`crates/silex_bootstrap/tests/docs_examples.rs`
- host browser 契约：`crates/silex_bootstrap/tests/app_host.rs`
- page listener 契约：`crates/silex_bootstrap/tests/page_controller.rs`
- browser target 与 transfer：`crates/silex_bootstrap/tests/browser_bootstrap.rs`
- JavaScript 对象与错误转换：`crates/silex_bootstrap/tests/js_object.rs`
- native 错误模型：`crates/silex_bootstrap/tests/error.rs`
- builder scope：`crates/silex_bootstrap/tests/compile_fail.rs` 与 `tests/ui/`

## 已知限制与维护注意

- `AppHost` 只拥有自己的 `MountedApp` boundary，不拥有 caller 预先放在 target
  中的其它节点；target 被外部移除不会自动触发 unmount。
- `replace` 不是双阶段原子切换：旧 app 必须先 dispose，只有清理成功才会
  执行新 builder。旧清理失败时既不会恢复旧 session，也不会启动新 session。
- `PageController` 的自动 unmount 只报告失败，不会把错误返回给触发浏览器
  事件的调用方；必须通过 `LifecycleReporter` 记录 `BootstrapError`。
- `BrowserBootstrap` 不注册全局入口。`into_js_host` 只接受已经移除页面
  listener 的 `Manual` policy，不会隐式把 listener ownership 转交给 JavaScript。
- `JsAppHost` 不暴露通用 mount API；Rust 必须先创建并配置 `AppHost`，再把
  所有权转移给 JS。raw pointer 的唯一所有权和一次性释放是 unsafe 边界的不变量。
- `catch_unwind` 只能在使用 unwind panic 策略的目标上捕获 panic；wasm
  `panic=abort` 下不能依赖它把 builder 或 cleanup panic 转换成可恢复错误。
