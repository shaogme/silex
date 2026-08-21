+++
title = "inject_css!"
description = "inject_css! 的纯静态 CSS 编译、静态插值和动态输入拒绝边界。"
weight = 50
+++

# `inject_css!`

`inject_css!` 是文档级纯静态 CSS 入口，定义在
`crates/silex_macros/src/css.rs`。它适合 reset、`:root`、`@font-face`、
`@keyframes` 和其它不依附某个元素的固定规则。宏展开为一次或多次
`silex_css::inject_style` 调用，不返回 owner-bound style view。

## 输入

```rust
inject_css! {
    :root { --brand: #6366f1; }
    @font-face {
        font-family: "App Sans";
        src: url("/fonts/app.woff2") format("woff2");
    }
}
```

这是依赖调用方 CSS facade 的语法示意，不是独立 CI example。宏先把输入
解析为 `CssBlock`，然后在进入 LightningCSS 之前调用
`reject_dynamic_global`，保证错误指向动态输入本身。

## 允许的动态字面边界

“纯静态”不等于只能写裸 CSS literal。`$(static Path::TO_VALUE)` 是允许的
静态插值：它只接受 Rust path，并在生成代码中通过
`static_css_value<Property, _>` 求值一次，最终用模板渲染后注入。静态插值
的结果必须满足当前属性的 `StaticCssValue`/`ValidFor` 约束。

以下输入会在宏阶段拒绝：

- 声明值中的 `$ident`、`$path` 或 `$(expr)`；
- selector 中的动态片段；
- at-rule 参数中的动态片段，例如动态 `@media` 条件；
- 任何需要 owner、effect 或动态 stylesheet 的 source。

拒绝动态值是设计边界：静态 registry 要先完成 CSS 解析、压缩、layer
处理和 ID 去重，不能把运行时 source 的 placeholder 当成最终 CSS 语法。
需要动态声明或 selector 时使用 `global!`；需要动态 `:root` 主题时使用
`set_global_theme`。

## 展开与 registry

```text
CssBlock
  │ reject dynamic input
  ▼
CssCompiler::compile_global
  ├── static CSS / lifted @rules
  ├── static value bindings
  └── ValidFor assertions
          │
          ▼
render_static_template → inject_style(id, css)
```

全局 compiler 不生成 `.class` 包装，规则进入 `base` layer；需要被
LightningCSS 提升的 `@import`、`@charset`、`@layer` 语句和
`@keyframes`/`@font-face` 由 compiler 保持在合法位置。`inject_style` 根据
稳定 ID 去重；调用点多次执行不会重复追加同一静态内容。

## `@apply`

启用 `silex_macros/tw` 后，`@apply` 可以在此入口展开 Tailwind utility。
展开后的 `--tw-*` 和其它机器生成声明不会再经过用户 CSS 属性表的校验，
但 utility 名称本身仍由 Tailwind parser 解析。未启用 `tw` feature 时，
`@apply` 会报告 feature 错误，而不是静默输出原文。

## 维护与测试

修改 `inject_css_impl` 时要保持“先拒绝动态输入、后进入 LightningCSS”的
顺序，否则用户会得到无关的 CSS 语法错误。还要检查：

- `$(static ...)` 的值绑定和两个静态 style ID 是否使用同一组 values；
- 全局 rule 的 layer 与 at-rule 提升是否正确；
- registry 注入失败时 ID 是否仍可重试。

`pass_macro_static_css.rs` 覆盖静态入口，
`fail_macro_inject_css_dynamic_identifier.rs`、
`fail_macro_inject_css_dynamic_selector.rs` 和
`fail_macro_inject_css_dynamic_at_rule.rs` 固定拒绝诊断。
