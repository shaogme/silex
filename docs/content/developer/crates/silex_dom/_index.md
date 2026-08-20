+++
title = "silex_dom"
description = "将 owner 生命周期、响应式视图和宿主资源绑定到浏览器 DOM 的底层渲染 crate。"
template = "section.html"
sort_by = "weight"
+++

# `silex_dom`

`silex_dom` 是 Silex 的 DOM 渲染底层。它把 `silex_core` 的显式
`Runtime`/owner 能力转换成可重复挂载的 `View`、DOM 元素、属性、事件和
宿主资源，并通过浏览器的 `web_sys` API 创建和清理真实节点。它位于
应用组件、`silex_html` 标签 facade 与 `silex_bootstrap`/宿主适配器之下，
不负责组件宏、HTML 标签全集或应用启动流程。

## 在 Silex 架构中的位置

```text
应用组件 / silex_bootstrap / silex_html
                │
                ▼
       MountedApp · MountContext
                │
                ▼
 View · Element · AttrOp · EventHandler
                │
                ▼
 MountOwnerToken · MountState · HostResource
                │
                ▼
   silex_core Runtime / owner / scheduler
                │
                ▼
         web_sys DOM / Window
```

`silex_html` 通过 `silex_dom::define_tag!` 生成 HTML/SVG 标签函数和宏，
而 `silex_dom` 自身提供通用的 `Element`、`TypedElement<T>`、`Tag` 和
挂载契约。需要自定义标签或 SVG 元素时，可以直接使用 `define_tag!`；
普通应用通常从 `silex_html` 导入标签。

## 稳定入口与核心类型

| 入口 | 作用 |
| --- | --- |
| `MountedApp` | 为一个宿主节点管理可重复的应用挂载、提交、回滚和 dispose。 |
| `MountContext<'scope>` | 在一次挂载事务中提供 `OwnerAccess`、owner token、staging parent 和 `mount` 方法。 |
| `View<'scope>` | 可重复执行的视图工厂契约；每次 `mount` 都创建新的 `MountInstance`。 |
| `Element` / `TypedElement<T>` | 构造 HTML 或 SVG 元素、保存子视图和延迟属性。 |
| `MountInstance<'scope>` | 本次挂载产生的 DOM 节点快照，不负责替代 owner 清理。 |
| `AnyView<'scope>` / `chain!` | 在异构视图、可选视图、列表和动态组件之间做类型擦除或组合。 |
| `AttributeBuilder` / `ApplyTarget` / `AttrOp` | 构造 attribute、property、class、style、事件和自定义 DOM 操作。 |
| `MountOwner` / `MountOwnerToken` | 为子树注册 effect、cleanup、动态状态和宿主 callback。 |
| `AutoReactiveView` | 让 `Rx`、signal、computed 或 stored value 作为文本或动态视图挂载。 |
| `HostResource` | 由 owner 注册表管理的 window listener、timer、animation frame 等宿主资源；通过 `cancel`/`finish` 显式结束。 |
| `helpers` | window/document 访问、事件 target 读取、owner-bound timer 和 debounce 辅助函数。 |

应用代码通常从 `silex_dom::prelude` 导入上述 DOM 类型；需要保持依赖
边界清晰时，应从 `mounted`、`view`、`element`、`attribute` 和 `helpers`
逐项导入。

## 生命周期与并发边界

一次 `MountedApp::mount` 的 owner 层次如下：

```text
MountedApp
└── Runtime::owner() → OwnerHandle
    └── MountContext<'scope>
        └── MountOwnerToken<'scope>
            ├── element / component child owners
            ├── reactive effects and MountState
            └── HostResource / DOM cleanup
```

- `MountedApp` 的 builder 通过高阶生命周期接收 `MountContext<'scope>`。
  context、owner-bound view 和 error handler 不能从 builder 返回或保存到
  更长的作用域。
- `View::mount` 不取得 DOM 的全局所有权。元素、子视图、响应式 effect、
  事件监听器和 timer 都挂在传入的 `MountOwner` 子树上；owner 关闭时，
  子 owner、effect 和 cleanup 按生命周期顺序释放。
- owner-bound `HostResource` 的公开值不是 `Clone`/`Drop` 取消句柄。创建时
  会立即登记 owner 私有 lease；调用方可提前调用 `cancel()` 或一次性资源的
  `finish()`，owner cleanup 仍会最多执行一次物理取消。
- `Runtime` 和 DOM owner 都是单线程模型，内部使用 `Rc`、`Cell`、
  `RefCell` 与 `web_sys`。这些句柄不能当作 `Send + Sync` 的跨线程状态。
- 视图可以重复挂载；同一个 `View` 工厂的不同调用返回彼此独立的物理
  节点，但它们共享调用方传入的父 owner，直到该 owner 被关闭。

## 最小可运行流程

下面的代码保存在 `docs/examples/silex_dom/basic.rs`。native 构建只验证
视图和属性 API；wasm 构建会创建宿主节点、使用 `MountedApp` 提交一次
挂载，并显式 dispose。页面直接读取这一个源文件，不在 Markdown 中维护
第二份会独立演进的 Rust 代码。

{% set source = load_data(path="examples/silex_dom/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

对应的测试是 `crates/silex_dom/tests/docs_examples.rs`。native 测试执行
无浏览器分支；wasm 测试使用 `wasm-bindgen-test` 的 browser runner。

## 公开模块地图

| 模块 | 责任 | 代表源码 |
| --- | --- | --- |
| `mounted` | 应用级 mount boundary、提交、回滚、dispose 和清理诊断 | `src/mounted.rs` |
| `view` | `View` 契约、owner、动态视图、列表、响应式视图和 mount kernel | `src/view/` |
| `element` | untyped/typed element、标签 marker、DOM 事件安装 | `src/element.rs`、`src/element/tags.rs` |
| `attribute` | 属性 builder、属性操作、响应式 class/style、事件 mixin | `src/attribute/` |
| `event` | `EventDescriptor`、事件类型和带/不带参数的 handler | `src/event/` |
| `helpers` | window/document、event target、timer、animation frame、debounce | `src/helpers.rs`、`src/helpers/detached.rs` |

crate 根还提供 `document()` 和 `setup_global_error_handlers()`。前者是
缓存的浏览器 `Document` 入口；后者注册 debug panic hook、window `error`
和 `unhandledrejection` 监听器，适合在应用启动时调用。

## Feature、平台与外部边界

`crates/silex_dom/Cargo.toml` 没有声明 feature flag。crate 的运行时能力
依赖浏览器的 `web_sys`/`wasm-bindgen`，因此真正创建 DOM、访问
`window()`/`document()` 或运行 browser test 需要 `wasm32` 环境。native
构建仍可用于编译公开类型、运行 mount 错误模型和编译期 scope 契约；不要
在没有 window/document 的 native 进程里调用会主动 `expect` 的 DOM helper。

公开的 JavaScript 边界集中在 `JsCast::unchecked_into`、事件闭包和
`Closure` 资源管理：事件描述符的 `EventType` 必须与实际 DOM 事件匹配，
`on_untyped` 则由调用者承担类型转换责任。crate 没有自行暴露裸指针或
`unsafe` 容器，但这些 wasm 类型转换仍然是必须维护的不变量。

## 专题

- [挂载事务与回滚](mounting.md)：`MountedApp`、staging boundary、错误报告和可重试/毒化状态。
- [视图、动态分支与列表](views.md)：`View`、`AnyView`、响应式视图、keyed list 和 `RowUpdater`。
- [属性、事件与响应式绑定](attributes.md)：attribute/property、class/style、事件 handler 和 `IntoStorable`。
- [owner 生命周期与宿主资源](lifecycle.md)：`MountOwner`、cleanup、`MountState`、window listener、timer 和 detached helper。
- [测试与调试](testing.md)：native/browser/UI 测试分层、文档示例和失败定位顺序。

## 源码、示例与测试索引

- 公开入口：`crates/silex_dom/src/lib.rs`
- 应用 mount：`crates/silex_dom/src/mounted.rs`
- view 与 owner：`crates/silex_dom/src/view/`
- element、标签 marker 和事件安装：`crates/silex_dom/src/element.rs`、`src/event.rs`
- attribute 合并与响应式计划：`crates/silex_dom/src/attribute/`
- 文档示例：`docs/examples/silex_dom/basic.rs`
- 文档示例测试：`crates/silex_dom/tests/docs_examples.rs`
- mount/rollback 契约：`crates/silex_dom/tests/mounted_app.rs`、`tests/mounted_contract.rs`
- owner、dynamic branch 和 list：`crates/silex_dom/tests/owner.rs`
- host resource：`crates/silex_dom/tests/host_resources.rs`
- reactive attribute/style：`crates/silex_dom/tests/reactive_attribute.rs`
- 编译期 scope 契约：`crates/silex_dom/tests/compile_fail.rs`、`tests/ui/`

## 已知限制与维护注意

- `MountedApp` 只管理自己提交的 boundary 节点；host 中调用方预先放入的
  节点会被保留。外部移除 host 不会自动改变 `MountedApp::is_active()`，
  仍应显式调用 `dispose()`。
- mount builder 返回错误时，必须同时检查 `MountError::primary()`、
  `rollback()` 和 `availability()`。主错误与回滚失败属于不同来源；回滚
  不干净时 handle 会变成 poisoned，不能假定还可以安全重试。
- 响应式属性初始安装、更新和清理由同一个 owner 绑定。动态 class/style
  cleanup 只撤销动态贡献，不能用无条件 `remove_attribute` 破坏其它静态
  或其它 binding 的值。
- `RowUpdater::bind` 只能成功一次；row 被移除或 key generation 失效后，
  旧 updater 的 `update` 返回 `false`。不要把它当作可永久保存的全局 setter。
- `helpers::window()`、`document()`、`event_target()` 和非 `Result` 的
  event value helper 会在缺少对象或 target 时 panic/返回默认值；需要将
  环境缺失作为错误处理时，优先使用 `try_window`、`try_document` 或
  `event_target_value_result`。
- `helpers::detached` 中的 listener/timer 明确不绑定 `MountOwner`；它们
  只能通过返回的 RAII handle 或显式 cancel/clear 管理。页面级全局错误
  handler 使用 `Closure::forget`，这是有意延长到页面生命周期的资源。

验证本 crate 文档或公开 API 变更时，至少运行
`cargo fmt --all -- --check`、`cargo check -p silex_dom`、目标测试和
`zola --root docs check`。新增或修改 `docs/examples/silex_dom/` 后，
优先运行 `cargo test -p silex_dom --test docs_examples`，再按环境追加
wasm 编译或 browser runner 验证。
