+++
title = "事件与 backend-neutral payload"
description = "说明 silex_view 的元素事件、window 事件、handler 作用域和 SSR 记录。"
weight = 40
+++

# 事件与 backend-neutral payload

View 事件不暴露 `web_sys::Event`。`events::DomEvent` 只保存
`EventSpec`、opaque `DomNode` target 和可选的 backend control；browser adapter
负责填充 payload。SSR 对元素事件记录安装信息，但对 window event 直接返回
`DomError::Unsupported`。这样同一个 View 可以在 browser 和 SSR 中复用元素事件
描述，但 window listener 必须由支持该能力的 backend 提供。

## 元素事件

`AttributeBuilder::on(event, callback)` 和常用的 `on_click`、`on_input`、
`on_change`、`on_pointer_down`、`on_pointer_move`、`on_pointer_up`、
`on_pointer_cancel`、`on_mouse_enter`、`on_mouse_leave` 会把 handler 加入元素
的 `AttrOp`。内置描述符包括：

| 描述符 | `EventKind` | 常见 payload |
| --- | --- | --- |
| `click`、`dblclick`、`mouseenter`、`mouseleave` | `Mouse` | `mouse_data()` |
| `input` | `Input` | `input_value()` |
| `change`、`submit` | `Form` | 由 backend 提供的 control |
| `keydown`、`keyup` | `Keyboard` | `key()` |
| `focus`、`blur` | `Focus` | 由 backend 提供的 control |
| `pointerdown`、`pointerup`、`pointermove`、`pointercancel` | `Pointer` | `pointer_data()` |
| `wheel` | `Wheel` | 由 backend 提供的 control |

事件描述符实现 `EventDescriptor`；自定义描述符可以实现该 trait，或用
`Event::new(name, kind)` 创建 `Event`。trait 要求 descriptor 是 `Copy + Clone +
'static`，因为它会被保存到 owner-bound listener bridge。

## 两种 handler 签名

`EventHandler` 用 marker 区分带参数和不带参数的 callback：

```rust
let with_event = Element::with_child("button", "Click")
    .on_click(|event: DomEvent| {
        event.prevent_default();
        let _mouse = event.mouse_data();
        Ok(())
    });

let without_event = Element::with_child("button", "Click")
    .on_click(|| Ok(()));
```

callback 返回 `SilexResult<()>`。错误会通过 mount 时提供的 error handler 处理，
而不是从 browser 的同步 listener 安装调用直接返回。`DomEvent` 的通用能力包括：

- `spec()`：读取 name、kind、bubbles 和 cancelable 等 metadata；
- `target()`：取得 opaque event target；
- `prevent_default()`：请求 backend 阻止默认行为；
- `mouse_data()`、`pointer_data()`、`key()`、`input_value()`、`rect()`：读取可选
  payload；
- `focus_target()`：请求 backend focus；不支持时返回 `DomError::Unsupported`。

payload 是 `Option`，因为 SSR record 和并非所有物理事件都具有对应 control。
不要把 `mouse_data()` 或 `input_value()` 当成一定存在；没有 value 时应显式决定
是忽略事件还是返回业务错误。

## window 事件与资源清理

`bind_window_event(context, event, callback, error_handler)` 把 owner-bound
callback 安装到全局 window。它要求带 `DomEvent` 参数的 handler，并返回
`SilexResult<()>`；底层 `HostResource` 会被交给 owner，在 owner close 时取消。
普通元素 listener 同样保存 `HostResource`，另外先关闭 callback gate，再取消
物理 listener，避免清理过程中仍 dispatch 到已经关闭的 owner。window listener
当前只依赖 `HostResource` 的取消，不额外注册元素 listener 使用的显式 callback
gate；两者都不应在 dispose 后继续使用。

事件 callback 捕获的 `MountDomAction`、`NodeRef` 或 signal 必须与同一个
`'scope` 相关联。`MountDomAction::with_context` 在 owner inactive 后会返回
`ReactiveError::NoSuchNode`；不能在 dispose 后继续对旧 node ref 做 DOM action。

## SSR 行为

SSR mount 不执行元素事件 callback，也不把 handler 序列化成 `onclick` 等 HTML
属性。元素事件通过 `SsrDom` 保留 event record/hydration record，其中包含 target
identity、target kind 和 `EventSpec`，供后续 hydration 层在 browser 中重新安装
listener。owner cleanup 仍会取消这些记录对应的 host resource，所以 mount
rollback 和 dispose 后 event record 应为空。

`bind_window_event` 在 SSR backend 中调用 `listen_window`，直接返回
`DomError::Unsupported { capability: "window event listener" }`；它不会产生
hydration record，也不会安装 callback。需要覆盖该分支时，应在 SSR 集成测试中
匹配这个错误，而不是把它归入元素事件记录。

```text
View event binding
    │
    ├── Element::on_click / browser: 物理 listener → DomEventBridge → owner handler
    ├── Element event / SSR: EventRecord/HydrationRecord，不执行 callback
    └── bind_window_event / SSR: DomError::Unsupported，不生成 record
```

SSR record 不是 HTML 合同：serializer 不会输出事件属性，hydration 层仍需依据
自己的流程把 handler 与 browser 节点重新关联。

## 错误与清理边界

listener 安装可能因为 foreign context、无效 event name、backend 不支持或 owner
已经关闭而失败。安装阶段的错误会使当前元素 mount rollback；已经成功安装的
listener 必须由 owner cleanup 取消。`HostResource::cancel` 是幂等的，但取消动作
失败仍应交给 error handler 或 cleanup report；不要使用 `finish` 代替正常 listener
取消。

实现与测试入口：`src/kernel/events/descriptor.rs`、`listener.rs`，以及
`tests/ssr_mount.rs` 的 SSR record/cleanup 测试和 `tests/browser.rs` 的 element、
window、focus 与 dispose 测试。
