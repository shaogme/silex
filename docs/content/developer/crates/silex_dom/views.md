+++
title = "View、分支与列表"
description = "高层 View kernel 已迁移到 silex_view；silex_dom 只提供其注入的物理 backend。"
weight = 50
+++

# View、分支与列表

View API 的唯一公开归属是 `silex_view`。`silex_dom` 不再提供
`silex_dom::view`、`Element`、`AnyView`、owner 或列表 facade。

## View 契约

`silex_view::View<'scope>::mount(&MountContext<'scope>)` 创建一次独立的
`MountInstance<'scope>`。context 提供 `DomContext`、物理 target、逻辑 ancestry、
owner 和 transaction；View 不直接保存 browser concrete type。

`AnyView` 只做 scope-bound 类型擦除，不拥有 root owner；`Option`、`Vec`、
`ViewCons`、`SilexResult` 和响应式 view 都通过 child owner 参与同一事务。

## 分支与列表

`DynamicRenderer` 维护稳定的 comment/range anchor；branch identity 变化时
只关闭旧 content owner，runtime owner 可继续存在。keyed list 以 key 保持 row
identity，失败时恢复旧 snapshot 并 dispose pending rows。`RowUpdater` 绑定
generation，row 删除后旧 updater 返回 inert/false。

这些语义依赖 `silex_dom::DomRange` 和抽象 tree operation，但 diff、owner、
rollback 和 scope 约束全部由 `silex_view` 实现。

## backend 注入

| backend | 构造 | 结果 |
| --- | --- | --- |
| browser | `BrowserDom::from_window()` | 真实 DOM、事件和宿主 resource。 |
| SSR | `SsrDom::new()` | 确定性树和 serialization；事件只生成 hydration record。 |

这也是为什么 View 的公共签名不能出现 `web_sys::Node`、`web_sys::Element`
或 `wasm_bindgen` 类型。

## 失败路径

自定义 View 必须覆盖 partial child mount、属性错误、NodeRef cleanup、owner
cleanup error 和重复 dispose。若 rollback report 非 clean，`MountedApp` 必须
进入 poisoned；不得通过重新引入旧 `silex_dom` 高层 facade 来绕过状态机。

高层设计、API 与测试见 [`silex_view`](@/developer/crates/silex_view/_index.md)。
