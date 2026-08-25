+++
title = "生命周期与宿主资源"
description = "silex_view owner 与 silex_dom 物理宿主资源的生命周期边界。"
weight = 30
+++

# 生命周期与宿主资源

owner/mount 生命周期属于 `silex_view`；物理 listener、timer、animation frame
等资源属于 `silex_dom::runtime::HostResource`。View owner 只保存注册和清理能力，
不重新定义 browser handle。

## 关闭顺序

```text
MountedApp dispose / rollback
        │
        ▼
View child owner close
        │
        ├─ effect / MountState close
        ├─ event bridge 与 NodeRef cleanup
        └─ HostResource cancel（最多一次）
```

cleanup 错误进入 `silex_dom::lifecycle::CleanupReport`；primary mount error、
`MountError`、`RollbackError` 和 `DisposeError` 由 `silex_view::error` 管理。
两个报告不能混为一个 backend error。

## backend 与 owner

`DomContext` 是显式注入、可 clone 的 backend context。`BrowserDom` 和 `SsrDom`
各自拥有 context；不允许把来自不同 context 的 node、element 或 range 混用。
SSR 的 HostResource 是 inert resource，但仍遵守相同的 owner cleanup 形状。

`NodeRef`、动态 class/style 和事件 listener 都绑定到当前 child owner。失败
mount 必须同时撤销已经创建的节点、effect、resource 和 reference；成功 dispose
必须幂等。

## browser 与 detached API

需要 browser/window 的 helper 必须位于显式 browser adapter 或拥有 browser
feature 的上层 crate。`silex_dom` 不再提供全局 `document()` 或
`setup_global_error_handlers()`。页面启动代码应显式调用
`BrowserDom::from_window()`，SSR/native 代码使用 `SsrDom` 或 capability
错误，不应依赖 window 全局。

不绑定 owner 的 detached listener/timer 仍可存在于专门的宿主 crate，但它们
不能伪装成 View cleanup；调用方必须持有返回的取消句柄。

## 维护检查

- cleanup 只能撤销当前 binding 的 DOM 贡献；
- cancel 先关闭 callback gate，再执行物理取消，重复调用无副作用；
- rollback 后 NodeRef 为空、staging tree 为空，且可重试的 `MountedApp` 未 poison；
- SSR 不引入 `web_sys` runtime，也不注册真实 listener。

相关源码：`silex_dom/src/runtime/host.rs`、`silex_dom/src/lifecycle/cleanup.rs`、
`silex_view/src/owner.rs`、`silex_view/src/mounted.rs`。
