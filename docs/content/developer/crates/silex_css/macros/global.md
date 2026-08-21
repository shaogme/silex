+++
title = "global!"
description = "global! 的全局样式签名、静态/动态分叉、placeholder 替换和 owner 边界。"
weight = 30
+++

# `global!`

`global!` 用一个显式命名的函数描述文档级 CSS。它生成一个没有实际 DOM
节点的 view：静态路径负责把 CSS 放入静态 registry，动态路径生成
`GlobalStyleView`，由 owner 管理动态样式表。实现与 `styled!` 共用
`crates/silex_macros/src/css/styled.rs` 中的 global 分支。

## 输入结构

```rust
global! {
    pub AppGlobal<'owner>(owner: OwnerAccess<'owner>) {
        body { margin: 0; }
    }
}

global! {
    pub DynamicGlobal<'owner>(
        error_handler: ErrorReporter<'owner>,
        color: Signal<'owner, Hex>,
        selector: Signal<'owner, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}
```

这是依赖 owner 类型的语法示意。宏要求显式可见性和名称；省略 `pub` 或
名称会在宏解析阶段失败。必须有显式 owner lifetime，并且至少有一个参数
的类型携带该 lifetime，例如 `OwnerAccess<'owner>`、`Signal<'owner, T>`。
这样生成的 view 不会把 source 从 owner 中偷出，也不会创建隐式 runtime。

## 静态与动态分叉

### 静态 global

没有响应式声明值或动态 selector 时，宏生成返回 view 的函数，调用函数时：

1. 对静态声明生成属性断言和静态值绑定；
2. 使用 `CssCompiler::compile_global` 生成 `base` layer 的全局 CSS；
3. 通过 `inject_style` 放入共享静态 registry；
4. 返回无 DOM 节点的空 view。

静态样式使用 registry 的 ID 去重，不由该空 view 持有动态 stylesheet lease；
因此“调用函数”是触发静态文档样式注册的动作，不能把它理解成组件卸载时
一定会撤销该静态规则。`$(static Theme::PRIMARY)` 可以出现在静态声明中，
但必须是静态 path。

### 动态 global

存在响应式声明值或动态 selector 时，生成函数返回
`SilexResult<impl View<...>>`，并构造 `GlobalStyleView`：

```text
声明值 $(value)  ──► var(--slx-dyn-N) ──► 文本 replacement
selector $(name) ──► CssPart::SelectorVal ──► 动态 rule template
                                      │
                                      ▼
                         owner-bound dynamic stylesheet
```

全局规则没有元素可写 inline custom property，所以声明值的
`var(--slx-dyn-N)` 是模板占位符，运行时会替换成净化后的 CSS value。动态
selector 使用 selector 专用片段；不能把 selector 当普通声明值替换。

动态 global 必须声明一个类型名结尾为 `ErrorReporter`、`ErrorHandler` 或
`ErrorHandlerToken` 的参数。宏使用该参数生成 handler reference，所有
getter 和动态 selector source 的读取错误都交给它。动态 view 的样式表由
owner 管理，dispose 时释放 effect、动态 class/template 和 stylesheet lease。

## 全局 CSS 约束

- `global!` 使用不包 `.class` 的 compiler 模式，静态规则最终放在 `base`
  layer；它适合 reset、`body`、`:root` 和文档级 at-rule。
- 动态 selector 可以生成动态规则，但 selector 内不能放语句式 at-rule，
  例如 `@import`；这类规则没有合法的运行时嵌入位置。
- `@keyframes`、`@font-face` 等需要提升到规则外层的内容仍由 compiler 处理。
- `unsafe` 只关闭 CSS compiler 的静态校验；不会关闭 placeholder 的替换、
  selector/value 净化、owner lifetime 或错误处理。

如果规则完全静态，应使用 `global!` 或 `inject_css!` 的静态入口；如果需要
响应式主题变量，优先考虑 `set_global_theme`，因为主题 runtime 已经提供
变量 diff 和 cleanup。

## 维护与测试

重点检查 `generate_static_global` 与动态 global 分支是否保持相同的：

- `static_id`/`style_id` 和 `base` layer 语义；
- `$(static ...)` 的值序号与 replacements 对应关系；
- `CssPart::Lit`、`CssPart::SelectorVal` 的边界净化；
- `GlobalStyleView` 的初始化失败清理和 owner dispose。

UI 契约在 `pass_macro_scoped_global.rs`、
`fail_macro_global_dynamic_without_args.rs`、`fail_macro_global_missing_name.rs`
和 `fail_macro_inject_css_dynamic_*.rs` 附近；实际 manager/fallback 行为由
`silex_css/tests/owner.rs` 和 `tests/fallback.rs` 覆盖。
