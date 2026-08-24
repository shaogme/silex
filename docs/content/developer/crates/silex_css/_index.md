+++
title = "silex_css"
description = "Silex 的类型安全 CSS builder、响应式样式源、主题变量和样式表运行时。"
template = "section.html"
sort_by = "weight"
+++

# `silex_css`

`silex_css` 是 Silex 的 CSS 运行时与类型层。它把 CSS 属性和值的约束、
owner 绑定的响应式 source、局部 class、主题变量和文档级样式表注入连接
起来。它位于 `silex_core` 的响应式 owner、`silex_dom` 的 attribute/mount
边界和上层 `silex_macros`/`silex` facade 之间；它不负责 DOM 树的创建，也
不直接实现 `css!`、`styled!` 或 `tw!` 过程宏。

## 在 Silex 架构中的位置

```text
组件 / silex::prelude / silex_macros
       css! · styled! · theme! · tw!
                    │
                    ▼
             silex_css facade
   Style · CssSource · Theme · DynamicCss
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
  silex_view ApplyToDom  样式表 registry/backend
  owner · cleanup         static / dynamic CSS
          │                   │
          └─────────┬─────────┘
                    ▼
       silex_core owner / Rx / effect
                    │
                    ▼
       browser DOM / CSSOM / <style>
```

过程宏位于 `crates/silex_macros/src/css/`，通过 `silex::css` 的重新导出
调用本 crate 的类型和运行时。直接依赖 `silex_css` 时，稳定入口是
`prelude`、`builder`、`source`、`theme` 和 `runtime`。

## 稳定入口与核心类型

| 入口 | 作用 |
| --- | --- |
| `Style<'scope>` / `sty` | 构造局部样式；生成稳定 class，并把动态值绑定到元素的 inline style。 |
| `IntoCssSource` / `CssSource` | 区分拥有值的静态 source 与带当前 owner 生命周期的 `Rx` source。 |
| `ValidFor<P>` / `CssProperty` | 约束 CSS 值能否用于具体属性；属性 builder 由注册表生成。 |
| `DynamicCss<'scope>` | 表示 `css!` 生成的动态类、动态选择器和响应式声明。 |
| `DynamicStyleManager` | 管理一张 owner 绑定的动态样式表，负责更新、共享、退休和清理。 |
| `inject_style` / `StaticStyleRegistry` | 去重并批量注入文档级静态 CSS。 |
| `ThemeToCss` / `ThemeVariables` | 把主题值映射为 CSS 自定义属性，并支持响应式更新。 |
| `set_global_theme` / `ThemePatchToCss` | 注入 `:root` 主题，或在局部元素上增量覆盖变量。 |
| `CssPart` / `GlobalStyleView` | 供宏展开产物描述动态选择器和无 DOM 节点的全局样式。 |
| `css_min!` / `css_max!` / `css_clamp!` | 生成量纲受约束的 `calc()` 表达式。 |

## 生命周期与并发边界

一次局部样式应用的关系如下：

```text
Runtime
└── OwnerAccess<'scope>
    ├── Rx<'scope, T> ──► Style<'scope> / CssSource<'scope, T>
    └── MountOwnerToken<'scope>
        ├── class / inline custom properties
        ├── reactive effects
        └── DynamicStyleManager / cleanup
```

- `sty(ctx)` 需要 `SilexContextProvider<'scope>`。动态 CSS 不会隐式创建
  `Runtime`，`Rx`、`ReadSignal`、`Computed` 等 source 必须属于当前 owner
  和兼容的 runtime。
- `Style` 实现 `silex_view::attribute::ApplyToDom`。应用到元素时，调用方
  传入的 `MountContext` 提供 owner、事务和错误处理；owner 关闭会移除该
  样式产生的 class 以及它拥有的 inline 自定义属性。
- 局部动态声明优先写入 `var(--sb-...)` 引用，再由 effect 更新元素的
  `CssStyleDeclaration`。动态选择器不能挂在元素上，因此由
  `DynamicStyleManager` 生成内容相关的动态 class 并更新样式表。
- runtime 使用 `Rc`、`RefCell` 和线程局部 registry，产生的状态不是
  `Send + Sync` 的跨线程 CSS 状态。CSS registry 只应在同一浏览器
  线程/owner 模型内使用。

## 最小可运行流程

下面的示例来自 `docs/examples/silex_css/basic.rs`。native 构建验证
`Style`、CSS 值类型和 owner 绑定；wasm 构建还会把样式应用到真实元素。
页面读取这一份源文件，不在 Markdown 中维护第二份会独立演进的 Rust 代码。

{% set source = load_data(path="examples/silex_css/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

对应的编译测试是 `crates/silex_css/tests/docs_examples.rs`。示例的
`run` 返回 `Result`，没有用 `unwrap`/`expect` 隐藏 builder、owner 或 DOM
错误。

## 公开模块地图

| 模块 | 责任 | 代表源码 |
| --- | --- | --- |
| `builder` | `Style`、属性 builder、嵌套规则、DOM 应用和动态 inline 绑定 | `src/builder.rs` |
| `source` | `CssSource`、`IntoCssSource`、`IntoCssReactive`、静态值边界 | `src/source.rs` |
| `types` | 单位、颜色、关键字、计算、渐变、简写和 `ValidFor` | `src/types.rs`、`src/types/` |
| `theme` | 主题变量、全局主题、局部 patch 和变量 diff | `src/theme.rs` |
| `runtime` | 静态/动态 registry、CSSOM 后端、模板渲染和生命周期 | `src/runtime/` |
| `class` | `IntoClass` 与 `cx!` 条件 class 组合 | `src/class.rs` |
| `layers` | `base`、`components`、`utilities`、`overrides` 层次常量和包装 | `src/layers.rs` |
| `escape` | 属性名、声明值、selector 片段和 CSS 字符串净化 | `src/escape.rs` |
| `codegen` | 由 MDN CSS 数据生成属性和关键字能力 | `src/codegen/` |
| `tw`（feature） | Tailwind variant schema 入口 | `src/tw/` |

## Feature 与平台

`silex_css` 默认不启用 feature。`tw` 只启用 `silex_tw_core` 和
`silex_css::tw` 的 Tailwind 支持；`test-style-fallback` 是浏览器回归测试
用 feature，用来强制真实 `<style>` 兜底后端。

| Feature | 作用 |
| --- | --- |
| `tw` | 暴露 `VariantSchema` 和宏展开所需的 Tailwind variant 支持。 |
| `test-style-fallback` | 仅用于测试，覆盖 `CSSStyleSheet` 不可用时的 `<style>` 路径。 |

wasm 目标使用 `CssStyleSheet`/`document.adoptedStyleSheets`，无法使用时
回退到插入 `<head>` 的 `<style>`。native 目标不创建浏览器 DOM；其 fake
stylesheet backend 只供 crate 状态机测试观察调用顺序，不能证明浏览器
CSSOM 的行为。

## 专题

- [类型安全样式与 `Style`](@/developer/crates/silex_css/styling.md)：属性能力、单位、计算、class、嵌套和动态值。
- [样式表运行时与清理](@/developer/crates/silex_css/runtime.md)：静态 registry、动态 manager、层次和 CSSOM 兜底。
- [主题与 CSS 变量](@/developer/crates/silex_css/theme.md)：`ThemeToCss`、全局主题、局部 patch 和响应式 diff。
- [CSS 宏](@/developer/crates/silex_css/macros/_index.md)：`css!`、`styled!`、`global!`、`classes!`、`inject_css!`、`theme!`、`tw!` 和 variant 宏的边界。
- [测试与调试](@/developer/crates/silex_css/testing.md)：native、browser、fallback、UI 编译期契约和文档示例。

## 源码、示例与测试索引

- facade：`crates/silex_css/src/lib.rs`
- builder 与值源：`src/builder.rs`、`src/source.rs`
- 类型系统：`src/types.rs`、`src/types/`
- 样式表运行时：`src/runtime/dynamic.rs`、`src/runtime/registry.rs`、`src/runtime/sheet.rs`
- 主题：`src/theme.rs`
- 宏编译层：`crates/silex_macros/src/css.rs`、`src/css/styled.rs`、`src/css/tw/`
- 文档示例：`docs/examples/silex_css/basic.rs`
- 文档示例测试：`crates/silex_css/tests/docs_examples.rs`
- 类型 UI 测试：`crates/silex_css/tests/css_type_safety.rs`、`tests/ui/`
- owner/browser 测试：`crates/silex_css/tests/owner.rs`
- fallback 测试：`crates/silex_css/tests/fallback.rs`

## 已知限制与维护注意

- `Style::render` 是 crate 内部测试入口，不是应用层 CSS 导出 API。应用应
  通过 `silex_view::attribute::ApplyToDom`、`Element::style(...)` 或宏生成的
  view 应用样式，避免绕过 owner cleanup。
- `sty().nest(":hover", ...)` 表示后代选择器 `.class :hover`；需要元素
  自身伪类时使用 `pseudo(":hover", ...)` 或 `on_hover(...)`。
- `raw` 和无类型 `css_unsafe` 只绕过值能力检查，不绕过声明边界净化；
  但它们仍可能表达语义错误的 CSS。优先使用类型化属性、简写和 `CssVar<T>`。
- `ThemeToCss::get_variable_names()` 与 `get_variable_values()` 必须一一
  对应；数量不一致会返回 framework error，不会静默截断。
- 静态样式以调用点生成的 ID 去重。修改宏编译器生成的 style ID、layer 顺序、
  动态 class 哈希或 cleanup 顺序时，必须同时检查 registry、owner 和
  browser/fallback 测试。

验证本页或公开 CSS 运行时变更时，至少运行 `cargo fmt --all -- --check`、
`RUSTFLAGS='-D warnings' cargo check -p silex_css`、对应的
`RUSTFLAGS='-D warnings' cargo test -p silex_css --test docs_examples` 和
`zola --root docs check`；
修改 wasm 后端或 UI 契约时，再追加对应 browser 或 trybuild 测试。
