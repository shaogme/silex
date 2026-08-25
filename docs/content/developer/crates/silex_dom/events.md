+++
title = "事件与宿主资源"
description = "silex_dom 的事件描述、bridge、browser 控件和 listener 生命周期。"
weight = 30
+++

# 事件与宿主资源

事件 API 把“监听什么”和“如何处理浏览器事件”分开。`EventSpec` 描述名称、
类别、冒泡和可取消属性；`DomEventBridge` 接收 backend-neutral 的 `DomEvent`；
browser adapter 才负责把 `web_sys::Event` 转换为可读取的 payload。

本文的片段用于展示真实 API 关系，省略外层函数；它们不是 CI 编译示例。

## EventSpec 与 descriptor

`EventKind` 覆盖 Mouse、Keyboard、Input、Focus、Pointer、Form、Touch、Drag、
Wheel、Animation、Composition 和 Custom。`EventSpec::new(name, kind)` 默认
`bubbles = true`、`cancelable = true`，可用 `with_flags` 覆盖。
这两个字段是 backend-neutral 元数据；当前 browser listener 安装时实际传给
`addEventListener` 的是 `EventOptions` 中的 `capture`、`once` 和 `passive`，
不会把 `bubbles` 或 `cancelable` 转换成 listener options。

如果上层拥有一个可复制、`'static` 的事件描述符，可以实现
`EventDescriptor`。trait 至少提供 `name()`；默认 `spec()` 使用 Custom 类别，
也可覆盖以提供准确的 `EventKind` 和 flags。

## 创建监听请求

元素 listener 使用 `PhysicalEventRequest::new(&element, spec)`，window listener
使用 `WindowEventRequest::new(spec)`。两者都支持 `with_options` 和
`with_bridge`；`EventOptions` 对应 browser 的 `capture`、`once`、`passive`。

```rust
let spec = EventSpec::new("click", EventKind::Mouse);
let request = PhysicalEventRequest::new(&button, spec)
    .with_options(EventOptions {
        capture: false,
        once: false,
        passive: true,
    })
    .with_bridge(Rc::new(move |event: DomEvent| {
        let _ = event.mouse_data();
        Ok(())
    }));
let listener = context.listen(request)?;
```

`validate()` 会拒绝空 event name。监听请求持有 element clone 和 bridge clone；
创建成功后，真正的取消权由返回的 `HostResource` 持有。

## DomEvent 与 browser control

`DomEvent` 暴露 `spec()`、`target()` 和可选的控制/读取方法：

| 方法 | browser 能力 |
| --- | --- |
| `prevent_default()` | 调用底层 event 的 `prevent_default`。 |
| `mouse_data()` | button 与 ctrl/meta/shift/alt 修饰键。 |
| `input_value()` | 从 input、textarea 或 select 读取 value。 |
| `key()` | 读取 keyboard event 的 key。 |
| `pointer_data()` | client 坐标和 pointer id。 |
| `rect()` | 读取 listener element 的 bounding rect。 |
| `focus_target()` | focus 当前 listener target。 |

这些方法在没有对应 control 时返回 `None` 或 `Unsupported`。特别是 `rect()`
使用 listener 的 `current_target`，而不是冒泡来源的动态 child，避免布局读取
落到错误元素。

`DomEventControl` 是 adapter 提供的能力 trait。应用通常只处理 `DomEvent`，不应
把 `web_sys::Event` 放入公共 handler 签名；这样同一 bridge 才能在 SSR 与 browser
之间共享。

## Bridge 与错误传播

`DomEventBridge` 的 `dispatch` 签名为 `DomResult<()>`，并且可以由满足
`Fn(DomEvent) -> DomResult<()> + 'static` 的闭包实现。`silex_view` 在更高层把
带 owner 的 handler 接入这个 bridge。

SSR listener 不执行 bridge，只在 `SsrDom::event_records()` 和
`hydration_records()` 中记录 event id、目标 backend、目标 identity、目标类别和
`EventSpec`。browser listener 的 callback 当前会忽略 bridge dispatch 的返回值；
因此上层 bridge 必须在自身边界把错误报告给 handler，不能期待 `listen()` 在事件
触发后再次返回错误。

## HostResource 的取消语义

`DomContext::listen` 和 `listen_window` 返回 `HostResource<'static>`。公开状态枚举
包含 `Active`、`Finished`、`Cancelled` 和 `Inert`；当前 listener 构造路径实际从
`Active` 转为 `Finished` 或 `Cancelled`，不会产生 `Inert`：

- `cancel()` 只有从 Active 转换时才执行底层取消动作；重复调用返回 `Ok(())`。
- 取消动作执行前先关闭 active gate，因此动作重入时不会重复执行。
- 取消动作返回错误时，资源仍保持 Cancelled，不会被重新激活。
- `finish()` 标记资源 Finished，并丢弃取消动作；适合一次性资源自然结束的路径。
- `Drop` 会调用 `cancel()`，但 Drop 无法向调用方返回取消错误；上层若需要诊断，
  应在 dispose 前显式 cancel 并处理结果。

对应实现和测试：`src/model/event.rs`、`src/model/event/request.rs`、
`src/model/event/bridge.rs`、`src/runtime/host.rs`、`src/adapters/browser/event.rs`、
`src/adapters/browser/listener.rs`、`src/adapters/ssr/event.rs`、
`tests/ssr/events.rs`。
