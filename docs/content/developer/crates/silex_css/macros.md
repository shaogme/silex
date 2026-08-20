+++
title = "宏、全局样式与 Tailwind"
description = "silex_css 与 silex_macros 之间的 css、styled、global、inject_css、theme 和 Tailwind 运行时契约。"
weight = 40
+++

# 宏、全局样式与 Tailwind

CSS 过程宏定义在 `silex_macros`，但生成物依赖 `silex_css` 的类型和运行时。
维护宏时要把“编译期生成什么”与“owner 何时注入/清理”分开检查：宏可以
生成静态描述符、响应式 source 和 `AttrOp`，真正的 DOM/CSSOM 副作用仍由
`silex_css`/`silex_dom` 的 owner 边界执行。

## 宏入口和输出

| 宏 | 输入/用途 | 主要运行时产物 |
| --- | --- | --- |
| `css!` | 局部 CSS block；支持静态声明、动态声明值和动态 selector。 | 静态时返回 class；动态时返回 `DynamicCss`/`SilexResult`。 |
| `styled!` | 声明带 tag、props、`#[ctx]` 的 styled component。 | `silex_dom` view、静态 CSS descriptor、reactive binding。 |
| `global!` | 绑定 owner 的文档级全局样式。 | `GlobalStyleView` 与 managed dynamic style。 |
| `inject_css!` | 只编译编译期静态 CSS（可包含 `$(static ...)`）。 | 通过 `inject_style` 注入共享静态表。 |
| `theme!` | 生成主题类型与 `ThemeToCss` 实现。 | `ThemeType`、变量名和值映射。 |
| `tw!` | 在启用 `tw` 时解析 Tailwind utility。 | 静态 class、静态 CSS 或响应式动态规则。 |
| `tw_variants!` / `declare_variants!` | 声明可复用的 variant schema。 | `VariantSchema` 和 variant 解析数据。 |

## `css!` 的静态与动态分叉

静态输入可以在没有 owner 的情况下编译和注入：

```rust
let class = css! {
    color: #0f172a;
    padding: 8px;
};
```

如果出现 `$(source)` 或动态 selector，必须显式提供当前错误处理边界：

```rust
let css = css!(error_handler; {
    width: $(width);
    &:hover { color: $(hover_color); }
})?;
```

动态插值要求 source 能转换为 `IntoCssReactive<'scope>`；宏不会把普通
静态值、未声明的 signal 或 foreign runtime 自动包装成响应式 source。动态
声明通过 element CSS variable 更新，动态 selector 通过 `DynamicCss` 的
动态 class 和样式表更新。

`inject_css!` 只接受编译期静态 CSS；它会在编译器进入 lightningcss 前拒绝动态
声明值、动态 selector 和动态 at-rule 参数；这样错误会指向真实的动态输入，
而不是被静态 CSS parser 转成无关语法错误。

## `styled!` 的 owner lifetime

`styled!` 的组件必须声明且只声明一个 `#[ctx]` 参数。只有静态 CSS 时，
组件可以使用普通静态返回类型；当存在动态值、动态规则或 variants 时，
生成代码必须携带显式 owner lifetime。宏不会创建隐式 runtime，也不允许
动态 signal 逃出组件 owner：

```rust
styled! {
    pub Card<'owner><div>(
        #[ctx] ctx: SilexContext<'owner>,
        children: AnyView<'owner>,
        color: Signal<'owner, Hex>,
    ) {
        padding: 12px;
        color: $(color);
    }
}
```

动态 `styled!` 会先通过 `owner.with_runtime` 验证所有 property getter，
然后把静态 descriptors、动态 `ReactiveBindingPlan` 和 variants 组合到
一个 `StyledVariantBinding`。variant 切换时，旧动态 class/manager 要先
失效，新的 class 才能成为当前值。

## 全局样式边界

`global!` 是 owner 绑定的无 DOM 节点 view；它可以在挂载时注入静态样式，
并为动态全局规则注册 `inject_managed_dynamic_style`。全局动态规则的
声明值没有元素可挂 CSS variable，因此使用模板占位符的一遍替换；selector
片段和 declaration value 使用不同的 escape 路径。

`inject_css!` 是文档级纯静态入口，适合 reset、`@font-face`、`:root` 或
全局 at-rule。主题值若要响应式变化，应使用 `set_global_theme`，而不是
试图把 signal 放进 `inject_css!`。

## Tailwind feature

启用 `silex_css/tw` 与 `silex_macros/tw` 后，`tw!` 的解析器把 utility、
modifier、variant 和 theme token 转换为 `DynamicCss`/styled 运行时数据。
`VariantSchema` 保存可验证的 variant 选项；未知选项不能静默成为任意
selector，应通过 `UnknownVariantOption` 处理或修正调用方配置。

Tailwind class 的优先级仍由 `layers::UTILITIES` 和生成的 layer order
决定。组件想覆盖 utility 时，应使用 `Style` 的 `overrides` 或明确的
CSS layer，而不要依赖两个 stylesheet 恰好谁后注入。

## 宏维护检查表

- 静态属性和静态插值要生成 `ValidFor` 编译期断言；不能把错误推迟到浏览器。
- 动态属性要把 getter 绑定到当前 `ErrorReporter`，并拒绝没有 owner scope
  的 signal；动态 selector 要单独做 selector fragment 净化。
- 全局静态 CSS 必须在 owner/source 完整验证前不产生 document side effect。
- 生成的 style ID、layer、template placeholder 和动态 class 要与
  `runtime::template` 的渲染函数保持一致，避免二次替换或 class 前缀误伤。
- 修改宏展开契约后，运行 `crates/tests/silex_macros_test` 的相关 UI 测试，
  同时运行 `silex_css` 的 owner/fallback 测试；只修改文档示例时无需运行
  workspace 全部测试。
