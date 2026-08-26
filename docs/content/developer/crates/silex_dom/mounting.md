+++
title = "挂载边界与清理"
description = "说明 silex_dom 提供的低层挂载原语，以及实际 mount 事务所属的上层。"
weight = 85
+++

# 挂载边界与清理

本页是跨 crate 导航入口：`silex_dom` 本身没有 `mount` 或 `dispose` 函数。实际
的应用挂载事务在 `silex_view::app::MountedApp`、`MountContext` 和 owner 中完成；
`silex_dom` 只提供 mount glue 使用的低层原语。

## `silex_dom` 提供什么

- `DomContext`：在显式 backend 中创建和更新节点；
- `InsertRequest`、`RangeRequest`、`DomRange`：组织连续节点和 branch；
- `AttributeRequest`、`PropertyRequest`：写入 DOM 状态；
- `PhysicalEventRequest`、`HostResource`：安装并取消 listener；
- `NodeRef`、`NodeRefBinding`：让 mount cleanup 按 generation 清除当前 binding；
- `CleanupReport`、`CleanupSink`：保留 cleanup failure 和 boundary error。

这些 API 是单步、显式、可返回错误的操作。它们不会知道某个 View 是否已经
commit，也不会自动逆序执行之前的 mutation。

## 上层事务责任

上层 mount 实现应在 provisional 阶段记录：

1. 创建的 owner 和响应式资源；
2. 插入的 DOM 节点和连续 range；
3. NodeRef binding 与 generation；
4. listener 的 `HostResource`；
5. 需要恢复或移除的属性/property 状态和错误 handler；`silex_dom` 本身不会为
   attribute/property 写入自动生成 cleanup。

只有所有初始化步骤成功后才提交；任一步返回 `DomError` 或上层错误时，应按
上层事务规则 rollback，并把清理失败保留在 report 中。不要把 `HostResource::finish`
当作 listener 的移除，它只丢弃取消闭包；正常 rollback/dispose 应调用 `cancel()`。

## 继续阅读

- [View 与 mount 的上层边界](views.md)
- [`silex_view` 总览](@/developer/crates/silex_view/_index.md)
- [生命周期与 NodeRef](lifecycle.md)
- [错误模型](errors.md)
