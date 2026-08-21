+++
title = "CSS 宏"
description = "silex_macros 中 CSS 过程宏与 silex_css 运行时之间的解析、展开和生命周期契约。"
sort_by = "weight"
+++

# CSS 宏

CSS 过程宏定义在 `crates/silex_macros/src/css/`，但生成代码依赖
`silex_css`、`silex_dom` 和 `silex_core` 的公开类型。维护时要把三个边界
分开检查：宏在编译期解析并生成代码，CSS compiler 负责把 DSL 变成静态
CSS/动态模板，owner 绑定的运行时负责注入、更新和清理。

## 宏入口

| 宏 | feature | 输入重点 | 主要展开结果 |
| --- | --- | --- | --- |
| [`css!`](css.md) | `css` | CSS block、`$ident`、`$(expr)`、`$(static path)` | class 字符串或 `DynamicCss` |
| [`styled!`](styled.md) | `css` | tag、props、`#[ctx]` 和 CSS block | 组件函数、元素 view、样式绑定 |
| [`global!`](global.md) | `css` | 显式可见性/名称、owner 参数和全局 CSS block | 全局样式 view |
| [`classes!`](classes.md) | `css` | class 表达式或 `class => condition` | `AttributeGroup` |
| [`inject_css!`](inject-css.md) | `css` | 纯静态 CSS block | 静态 registry 注入 |
| [`theme!`](theme.md) | `css` | 主题字段和 `#[theme(...)]` 属性 | Theme struct、变量映射和 Patch |
| [`tw!`](tw.md) | `tw` | Tailwind utility 字符串或条件 tuple | utility class、动态绑定 |
| [`tw_variants!`](tw-variants.md) | `tw` | base、variants、默认值和 compound | 类型安全 variant schema |
| [`tw_verbose!`](tw-verbose.md) | `tw` | 与 `tw!` 相同 | `tw!` 结果及编译期诊断文件 |

`declare_variants!` 不是 `silex_macros` 的过程宏，而是
`crates/silex_css/src/tw/variants.rs` 中的 `macro_rules!`。它是
`tw_variants!` 的运行时承载层；相关的生成类型、字符串解析和复合变体
契约记录在 [`tw_variants!`](tw-variants.md) 页面。

## 共同的 CSS 编译路径

```text
宏输入
  │ syn parser
  ▼
CssBlock / TwInput
  │ property table + value validation
  ▼
CssCompiler
  ├── static CSS      ──► lightningcss ──► inject_style / descriptor
  ├── dynamic value   ──► CSS variable  ──► owner effect
  └── dynamic selector ─► template        ──► DynamicStyleManager
```

CSS block 使用以下规则节点：声明、嵌套 selector、`@` 规则、`unsafe {}`
以及启用 `tw` feature 后的 `@apply`。静态声明会校验属性名和能确定的
取值；`unsafe {}` 只关闭宏侧的属性/取值校验，不关闭生成 CSS 的边界
净化，也不保证浏览器接受该语义。

### 插值的三种边界

- `$ident`、`$path` 和 `$(expr)` 是响应式插值。表达式必须能转为当前 owner
  的 `IntoCssReactive`/属性值 source；它们不能逃出对应的 owner。
- `$(static path)` 是静态插值，只接受 Rust path。宏在生成代码中通过
  `StaticCssValue` 和属性的 `ValidFor` 约束它，并在静态样式模板渲染时复用
  已求出的字符串。
- at-rule 参数和静态全局 CSS 不接受响应式插值。媒体条件需要在编译期确定；
  全局动态声明/selector 应使用 `global!`，而不是 `inject_css!`。

### 输出分层

- `css!`/`tw!` 默认进入 `utilities` layer；`styled!` 进入 `components`
  layer；`global!` 的静态规则进入 `base` layer。
- 组件类名来自编译产物指纹，不来自原始空白。动态声明的表达式文本不参与
  基础 class identity，实际值存放在元素的 CSS custom property 中。
- 静态 CSS 通过 ID 去重；动态 selector、全局动态规则和无元素可挂载的
  CSS 使用动态样式管理器。宏本身不创建 `Runtime`。

## 维护顺序

修改宏时建议按以下顺序定位：

1. 先检查 `css/ast.rs` 或 `css/tw/parser.rs` 是否正确识别输入和 span。
2. 再检查 `compiler/parser.rs` 的静态/动态分叉、placeholder 和属性类型。
3. 最后检查宏展开是否把 getter、descriptor、error handler 和 cleanup 交给
   正确的 `silex_css`/`silex_dom` 运行时入口。

过程宏的绝对路径通过 `crate_path` 解析，因此 renamed dependency 测试很重要；
不要在展开代码中假定调用方依赖名称一定叫 `silex`。

## 验证与测试位置

- `crates/silex_macros/src/css/*/tests` 和各模块 `#[cfg(test)]`：解析器、
  compiler 中间结果、类名和变体合并的 native 单元测试。
- `crates/tests/silex_macros_test/tests/ui/`：过程宏的 pass/fail 编译期
  契约和精确诊断。
- `crates/tests/silex_macros_test/tests/scoped_css_macros.rs`、
  `macro_owner.rs`：owner、动态 CSS、global 和 classes 的集成测试。
- `crates/tests/renamed_dep/`：调用方重命名 facade 依赖时的展开路径。
- `crates/silex_css/tests/owner.rs`、`fallback.rs`：宏产物进入真实 DOM、
  CSSOM 和 `<style>` fallback 后的生命周期行为。

本目录页面中的代码片段是源码契约说明，不是 `docs/examples/` 中的独立 CI
示例；若新增可执行示例，必须按 [开发者文档规范](@/doc-standards/developer.md)
放入 `docs/examples/` 并单独编译测试。

## 分宏专题

- [`css!`](css.md)：CSS block、静态/动态声明、动态 selector 和 `@apply`。
- [`styled!`](styled.md)：组件签名、variants、owner lifetime 和属性绑定。
- [`global!`](global.md)：全局静态/动态样式与无 DOM 节点 view。
- [`classes!`](classes.md)：普通 class、条件 class 和 `AttributeGroup`。
- [`inject_css!`](inject-css.md)：文档级纯静态注入和拒绝动态输入的原因。
- [`theme!`](theme.md)：主题类型、CSS 变量、配置补全和 Patch。
- [`tw!`](tw.md)：utility parser、条件分支、响应式 class 和 marker class。
- [`tw_variants!`](tw-variants.md)：item/表达式形式、严格解析和合并策略。
- [`tw_verbose!`](tw-verbose.md)：诊断文件格式、落点和失败回退。
