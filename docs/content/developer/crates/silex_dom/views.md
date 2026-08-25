+++
title = "View 与 mount 的上层边界"
description = "说明 silex_dom 与 silex_view 的职责边界和 DOM 注入方式。"
weight = 80
+++

# View 与 mount 的上层边界

`silex_dom` 不定义 `View`、`Element`、`MountedApp` 或 mount transaction。它提供
这些上层类型所需的 `DomContext`、backend-neutral node、attribute request、event
bridge、`NodeRef` 和 host resource。需要编写组件或管理 mount/dispose 时，应阅读
[`silex_view` 总览](@/developer/crates/silex_view/_index.md)。

## 两层职责

```text
silex_view
  View · Element · AttributeBuilder · EventHandler
  MountedApp · MountContext · owner · rollback
                         │
                         ▼
silex_dom
  DomContext · DomNode · DomElement · NodeRef
  AttributeRequest · PhysicalEventRequest · HostResource
                         │
                         ▼
              browser / SSR backend
```

`silex_view` 决定什么时候创建、更新和销毁一棵 View；`silex_dom` 只执行已经
验证过的物理请求。这样 SSR 与 browser 可以共享 View mount 契约，而不让公共
View API 携带 `web_sys` 类型。

## Context 注入

上层应用应在创建 `MountedApp` 或测试 fixture 时选择 backend，并把同一个
`DomContext` 与 host `DomNode` 交给上层：

- browser：`BrowserDom::from_window()`，host 通常来自 `document_body()`；
- SSR：`SsrDom::new()`，host 可以使用该 context 的 document node，或使用
  `context.create_element(...)` 创建并自行挂接的元素；
- 测试：可以直接使用 `SsrDom`，或注入实现 `DomBackend` 的测试 backend。

host 和 View 内部节点必须来自同一 backend context。把另一个 `BrowserDom` 或
`SsrDom` 的 node 作为 host 会在首次 DOM 操作时得到 `CrossContext`。

## mount 期间的 NodeRef 与 listener

View mount glue 通常使用 `NodeRef::bind_for_mount` 保存本次节点，并把返回的
`NodeRefBinding` 注册到本次 mount 的 cleanup。动态 branch 被替换时，旧 binding
的 `clear_if_current()` 只会清理自己的 generation；它不会影响新 branch。

事件 attribute 最终会生成 `PhysicalEventRequest` 和 `DomEventBridge`；成功监听
后返回的 `HostResource` 必须纳入 owner cleanup。SSR 不执行事件 callback，而是
保存 `hydration_records()`，由 browser hydration 阶段重新安装 listener。

## rollback 与 dispose

`silex_dom` 的 `DomContext` 操作是单步 `DomResult`，不会自动回滚一组 mutation。
上层 `silex_view` 必须记录已创建节点、NodeRef binding、listener 和 owner，mount
失败时按自己的事务协议清理；清理失败交给 `CleanupReport` 或 `CleanupSink`。

不要把“DOM 操作成功”理解成“mount transaction 已提交”：DOM context 不知道
上层 View 是否还有其他属性、effect 或 owner 初始化步骤未完成。

## 何时直接使用 silex_dom

直接使用 `silex_dom` 适合：

- 编写 browser/SSR backend 或测试 backend；
- 实现低层 DOM utility、SSR serializer 检查或 hydration record 处理；
- 在上层框架中消费 opaque node、NodeRef、event control 或 host resource。

如果目标是声明式 HTML、响应式属性、事件 handler、动态 branch、列表或应用级
mount，应使用 `silex_view`，不要在应用代码中手工复制一套 owner/rollback 逻辑。

相关文档：[节点树与渲染](rendering.md)、[生命周期与 NodeRef](lifecycle.md)、
[事件与宿主资源](events.md)、[backend 与互操作](interop.md)。
