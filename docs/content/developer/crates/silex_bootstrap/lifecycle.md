+++
title = "页面生命周期与浏览器入口"
description = "PageController、BrowserBootstrap 的页面事件监听、target 解析和 JavaScript transfer。"
weight = 30
+++

# 页面生命周期与浏览器入口

`PageController` 是 `AppHost` 的页面级适配器：它可以拥有 window listener，
在页面生命周期事件到来时请求 host unmount，同时把自动操作的错误交给
`LifecycleReporter`。`BrowserBootstrap` 在此基础上增加从当前 document 解析
target 和转移到 `JsAppHost` 的 convenience API。

## `PageLifecyclePolicy`

| policy | 安装的事件 | 触发条件 |
| --- | --- | --- |
| `Manual` | 无 | 只由调用方显式 `mount` / `replace` / `unmount`。 |
| `PageHide` | `window` 的 `pagehide` | 每次事件都请求 unmount；host 已空时操作保持幂等。 |
| `PageHideAndVisibilityChange` | `pagehide`、`visibilitychange` | 只有 `document.hidden()` 为 true 时请求 unmount。 |

`PageController::install_page_lifecycle(policy, reporter)` 会先移除当前
controller 管理的 listeners，再安装新 policy。因此安装失败不会保留旧
policy。`Manual` 会直接成功且不访问 window；其它 policy 在 window 不存在、
document 不存在或 listener 安装失败时返回 `BootstrapError::Listener`。

reporter 的类型是：

```rust
type LifecycleReporter = Rc<dyn Fn(BootstrapError) + 'static>;
```

自动 unmount 成功、目标已经 unmounted 或重复 page event 不会调用 reporter。
如果 host 处于外层 dispose、listener callback 重入，或 cleanup 返回错误，
reporter 会收到 `BootstrapError`。reporter 必须是 `'static`，因为 listener
的寿命由 `PageController` 而不是当前 mount builder 决定。

## 页面事件的所有权和顺序

```text
window event
     │
     ▼
WindowListenerHandle
     │ Weak<RefCell<AppHost>>
     ├── host 已 drop → 忽略事件
     ├── host 可借用 → AppHost::unmount()
     └── host 正在借用 → reporter(ReentrantOperation)
```

controller 只保存由 `silex_dom::host` browser resource 注册得到的
`HostResource`。`remove_page_lifecycle` 清空 handle 集合，从而移除
所有由本 controller 安装的 listener；`Drop for PageController` 也会先做
同样的操作，再让内部 `AppHost` drop。这样页面 listener 不会在 host 已经
开始清理后继续触发自动 unmount。

页面 listener 是 page-controller resource，不属于 mount owner。若页面层同时使用
owner-bound listener，应让两者分别拥有明确的清理边界，不能期待关闭 root
owner 自动移除 `PageController` 的 listeners。

## 基本用法

下面是依赖真实 browser target 的 API 片段；`target` 和 `cleanup_sink` 的
创建代码被省略，因此它不是独立的 CI 示例：

```rust
let mut controller = PageController::new(target, cleanup_sink);
controller.mount(runtime, builder)?;
controller.install_page_lifecycle(
    PageLifecyclePolicy::PageHideAndVisibilityChange,
    Rc::new(|error| log_bootstrap_error(error)),
)?;

// 需要接管清理时先移除 listener。
controller.remove_page_lifecycle();
controller.unmount()?;
```

安装 policy 不会自动 mount，也不会注册全局入口。controller 可以在 mount
之前安装 listener；事件到来时 host 会返回 `AlreadyUnmounted`，随后仍可
显式 mount。

## `BrowserBootstrap`

`BrowserBootstrap` 只在 `browser-bootstrap` feature 下可用，内部持有
`PageController` 和一份当前 policy：

| API | 行为 |
| --- | --- |
| `new(target, sink)` | 使用 caller-owned `Node`，初始 policy 为 `Manual`。 |
| `from_element(element, sink)` | 将 `Element` 转为 `Node` 后调用 `new`。 |
| `from_id(id, sink)` | 通过 `try_document().get_element_by_id(id)` 解析 target；找不到返回 `TargetNotFound(id)`。 |
| `mount` / `replace` / `unmount` | 委托给内部 `PageController`，错误类型为 `BootstrapError`。 |
| `state` / `is_active` / `target` | 读取内部 host，不主动检查 target 是否在 document 中。 |
| `install_page_lifecycle` | 替换当前 policy；失败时 policy 保持 `Manual`。 |
| `remove_page_lifecycle` | 移除 listeners 并把记录的 policy 设为 `Manual`。 |
| `into_js_host` | 仅在 policy 为 `Manual` 时转移内部 host。 |

`from_id` 在 target 不存在时不会创建“半初始化”的 controller。它使用
`try_document`，因此 document 尚未建立的错误会表现为 `TargetNotFound`，而不
是由 `document()` 的 `expect` 触发 panic。

## 转移给 JavaScript 前的规则

`BrowserBootstrap::into_js_host` 不会隐式转移页面 listener。调用顺序必须是：

1. mount 或 replace 完成 Rust 应用配置；
2. 如果安装过 policy，调用 `remove_page_lifecycle()`；
3. 确认 controller 回到 `Manual`；
4. 调用 `into_js_host()`，把 host 的唯一所有权交给 `JsAppHost`。

如果仍有非 `Manual` policy，方法返回 `BootstrapError::Lifecycle`，原
`BrowserBootstrap` 不会被转移。JavaScript owner 只接管 host，不接管已经被
controller 管理的 page listener。

## 生命周期错误和调试

自动 unmount 的错误不会被 window event 的调用栈直接返回。调试时应在
reporter 中按层检查：

- `BootstrapError::Host(AppHostError::Dispose(error))`：读取
  `error.report()` 中的 cleanup/boundary failure；
- `BootstrapError::Host(AppHostError::ReentrantOperation)`：查找 cleanup 或
  event callback 中是否同步触发了新的 page event；
- `BootstrapError::Listener(error)`：检查 window/document 是否存在以及
  detached listener 的 JavaScript 安装是否失败；
- `BootstrapError::TargetNotFound(id)`：确认 target id 的创建时序，不要用
  `unwrap` 掩盖初始化竞态。

## 对应测试

- `tests/page_controller.rs`：三种 policy、重复事件、remove listener、drop
  顺序、可见性过滤、reentrancy 和 reporter。
- `tests/browser_bootstrap.rs`：`from_id`、缺失 target、manual transfer 和
  非 manual policy 的拒绝。
- `src/page_controller.rs`：`Drop` 清理 listener 和 JS transfer 的内部
  `Rc::try_unwrap` 约束。
