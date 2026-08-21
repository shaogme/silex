+++
title = "styled!"
description = "styled! 的组件签名、静态样式、动态绑定和 variants 展开契约。"
weight = 20
+++

# `styled!`

`styled!` 把一个带 HTML tag、props 和 CSS block 的声明展开为组件函数。
它先使用 `CssCompiler` 生成样式描述，再交给 component 生成器构造 view、
props builder 和 DOM attribute 操作。入口实现位于
`crates/silex_macros/src/css/styled.rs`。

## 输入结构

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

这段是依赖 facade 类型的语法示意，不是独立 CI example。输入按以下顺序
解析：外部属性、可见性、可选 `unsafe`、组件名/泛型、`<tag>`、参数列表、
可选 `where` 和 CSS block。已知 HTML tag 返回对应的 typed element；未知
tag 走满足 view/attribute builder trait 的 opaque 返回类型。

`#[ctx]` 参数是 owner 和错误处理的来源。当前实现通过 `find_ctx_parameter`
取得第一个带 `#[ctx]` 的参数来作存在性检查和后续 component 展开；维护时
应保持每个 `styled!` 只声明一个 `#[ctx]`，不要依赖多个标记参数的未定义
行为。动态 CSS 或 variants 还必须在泛型中声明显式 owner lifetime；宏不会
为组件创建隐式 `Runtime`。

## 静态样式路径

完全静态的 CSS 会在函数被调用时执行静态样式初始化，生成的元素带上稳定
class。静态 `$(static Theme::PRIMARY)` 先生成一次静态值，再通过模板渲染
样式；静态值必须是 Rust path，且同时满足 `StaticCssValue` 与属性的
`ValidFor`。样式表 descriptor 会交给 `inject_style` 去重。

宏生成的组件大致包含以下运行时组成部分：

```text
component props
    │
    ├── element(tag, children)
    ├── class(static_class)
    ├── optional style prop
    └── CombinedStyles / StyledVariantBinding
```

如果 props 中有名为 `children` 的参数，它被传给非 void tag；没有时使用
空值。名为 `style` 的参数会额外接到元素的 style attribute。其它参数由
component 层继续生成 builder/product 逻辑。

## 动态声明值

动态属性值会被转换为 `make_property_val` getter，并生成
`ReactiveBindingPlan::style_property`。规则中的值是
`var(--slx-st-<hash>-N)`，实际值由 owner 绑定的 effect 写入元素 inline
style。挂载 owner 负责：

- 读取 source 并报告错误；
- 添加/更新 custom property；
- 在 dispose 时移除 custom property 和生成的 class。

动态 value 不需要把 source 文本放入 class hash；改动 source 的当前值只会
触发 effect，不会重新生成 CSS 结构。动态 selector 则不能使用元素上的
custom property，会生成 `StyledDynamicRule` descriptor，挂载时交给
`StyledVariantBinding` 和动态样式表管理。

## `variants` 块

`variants` 是 `styled!` 自己的 inline variant DSL，不要与
`tw_variants!` 的 schema 混淆：

```rust
styled! {
    pub Panel<'owner><div>(
        #[ctx] ctx: SilexContext<'owner>,
        children: AnyView<'owner>,
    ) {
        variants: {
            mode: {
                light: { color: white; background: black; },
                dark: "bg-slate-900 text-white",
            }
        }
    }
}
```

块形式直接使用 CSS block；字符串形式只在 `tw` feature 开启时可用，并先
经过 Tailwind parser/codegen 转为 CSS block。若 variant 名没有对应 prop，
宏会补一个 `Signal<'owner, String>`，并带 `#[prop(into)] #[chain]`；已有
同名 prop 则复用它。每个 variant 都有自己的静态 descriptor、动态 value
getter 和动态 rule descriptor。

切换 variant 时，`StyledVariantBinding` 根据当前 signal 选择 class/rule，
并保证旧动态 rule 失效后再使用新值。variant 动态 CSS 与基础动态 CSS 共用
当前 owner；不要把 variant signal 保存到 owner 之外。

## `unsafe` 与错误边界

`styled!` 前置的 `unsafe` 只影响 CSS compiler 的静态属性/取值校验；它不
绕过动态 source 的类型转换、owner lifetime、selector/value 净化或 DOM
清理。动态声明的生成函数需要 `SilexResult`，错误由 component/view 的
挂载流程继续处理。

## 维护与测试

改动 `styled.rs` 时重点检查：

- 参数 lifetime 归一化是否只给 owner-bound 类型补入当前 scope；
- 静态 descriptor、模板、动态 getter 的序号是否对应同一 variant；
- `style`/`children` 特殊参数和 component metadata 是否仍保持兼容；
- dynamic rule 的旧 class、动态样式表 lease 和 owner cleanup 是否成对；
- `#[ctx]` 诊断和多个 `#[ctx]` 的处理是否与文档和错误消息一致。

相关 UI 诊断包括 `fail_macro_unscoped_styled_signal.rs`、
`fail_macro_styled_child_escape.rs` 和静态值失败用例；动态 owner 行为见
`crates/tests/silex_macros_test/tests/macro_owner.rs`。
