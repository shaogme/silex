+++
title = "silex_dom"
description = "Silex 的后端无关 DOM、属性、事件、宿主资源与 browser/SSR 适配层。"
template = "section.html"
sort_by = "weight"
+++

# `silex_dom`

`silex_dom` 只拥有物理 DOM 能力：不透明节点、树操作、attribute/property
写入、事件描述、宿主资源、清理报告以及 browser/SSR backend。View、Element、
owner 和 mount kernel 位于 [`silex_view`](@/developer/crates/silex_view/_index.md)。

## 分层

```text
silex_html / silex / 组件
          │
          ▼
     silex_view
  View · Element · owner
          │ 注入 DomContext
          ▼
     silex_dom
  model · runtime · lifecycle · diagnostics
       ┌────────┴────────┐
  adapters::browser  adapters::ssr
```

高层代码不得通过全局 `document()` 创建节点，也不得从本 crate 导入旧的
View/Element facade。browser 应显式创建
`silex_dom::adapters::browser::BrowserDom`，SSR 应显式创建
`silex_dom::adapters::ssr::SsrDom`，再把 `DomContext` 注入
`silex_view::MountedApp`。

## 公开入口

| 模块 | 责任 |
| --- | --- |
| `model` | `DomNode`、`DomElement`、`ElementSpec`、attribute/event DTO 和 hydration record。 |
| `runtime` | `DomContext`、tree/range 操作以及 owner 可注册的宿主资源。 |
| `lifecycle` | `CleanupReport`、`CleanupSink` 和 scope-bound `NodeRef`。 |
| `diagnostics` | `DomError`、`DomResult` 和同构日志。 |
| `adapters::browser` | 唯一的 `web_sys` DOM/event/style adapter。 |
| `adapters::ssr` | 确定性内存 DOM、HTML serialization 和 hydration record。 |

`silex_dom` 不再提供 `prelude` 或高层 View facade；`View`、`Element`、属性
builder、事件 handler 和 `MountedApp` 应从 `silex_view` 导入。

## Feature 与依赖边界

| feature | 作用 |
| --- | --- |
| `browser` | 启用 `web-sys`、`wasm-bindgen` 和 browser backend；默认启用。 |
| `ssr` | 启用 SSR backend；不启用 web runtime。 |

验证 SSR 边界：

```text
RUSTFLAGS='-D warnings' cargo check --locked -p silex_dom \
  --no-default-features --features ssr
RUSTFLAGS='-D warnings' cargo tree -e features -p silex_dom \
  --no-default-features --features ssr
```

SSR serialization 会转义文本、attribute 和 style value；property 不会被
误写成 markup。事件不会输出 `onclick` 等 attribute，而是保留带
`target_backend`、`target_identity`、`target_kind` 和 `EventSpec` 的
`HydrationRecord`，供后续 browser hydration adapter 使用。

## 相关专题

- [属性与事件](attributes.md)：低层 request、高层 `silex_view` binding 和 `NodeRef`。
- [生命周期与宿主资源](lifecycle.md)：owner cleanup 与物理 resource 的边界。
- [挂载事务与回滚](mounting.md)：该 crate 提供 backend，mount 状态由 `silex_view` 管理。
- [测试与验收](testing.md)：native、SSR、trybuild 和 wasm/browser 分层。
- [View、分支与列表](views.md)：高层 View 契约迁移后的唯一文档入口。
