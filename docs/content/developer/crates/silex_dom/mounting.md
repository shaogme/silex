+++
title = "挂载事务与回滚"
description = "silex_view MountedApp 使用 silex_dom backend 的 staging、commit、rollback 和 dispose。"
weight = 20
+++

# 挂载事务与回滚

应用级挂载由 `silex_view::MountedApp` 管理；`silex_dom` 只提供节点和树操作。
新版 `MountedApp::new` 接收 `(Runtime, DomContext, host: DomNode, CleanupSink)`，
因此 browser/SSR 只通过 context 注入，不需要公共签名携带 `web_sys`。

## 边界与状态

```text
Ready ── mount builder ──► Mounting ── commit ──► Mounted
  ▲                         │                      │
  └──── retryable error ◄────┘                dispose
                                  │              │
                                  └── cleanup ────┘
                         rollback failure ──► Poisoned
```

builder 期间节点先进入 detached staging fragment。只有 builder 成功且事务
提交后，owned nodes 才追加到 caller-owned host；host 中原有节点不会被删除。
builder 错误会关闭 provisional owner、撤销 Attribute/事件/NodeRef，并清空
staging。cleanup report 为空时可重试，否则句柄进入 poisoned 状态。

## builder 与 View context

应用 builder 使用 `silex_view::MountBuilderContext`：

```rust
app.mount(|context| {
    let handler = context.access().error_handler(|_| {})?;
    context.mount_unit(
        silex_view::Element::with_child("main", "content"),
        handler,
    )
})?;
```

`MountBuilderContext` 和 View kernel 的 `silex_view::MountContext` 是不同语义：
前者提供应用 host/access，后者提供单个 View 的 target/ancestry/transaction。

## SSR 与 browser

SSR 用 `SsrDom::new()` 建立内存 document，browser 用
`BrowserDom::from_window()` 或显式 `BrowserDom::new(document)`。两者都可以
注入相同的 `DomContext`，但 node 不能跨 backend 混用。SSR serialization 只
输出树与安全属性；listener 进入 hydration record，不进入 HTML。

## 验收重点

- caller-owned host 节点在 mount、remount、dispose 后保持不变；
- builder 失败后 primary error、rollback report 和 retry/poison 状态可区分；
- rollback 清除 provisional owner、DOM、NodeRef、事件 record 所对应的实际树；
- dispose 可重复调用；
- `MountInstance` 不能逃逸当前 scope。

实现和测试位于 `crates/silex_view/src/mounted.rs`、
`crates/silex_view/tests/ssr_mount.rs` 与 `crates/silex_dom/src/ssr.rs`。
