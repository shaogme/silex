+++
title = "css!"
description = "css! 的 CSS block 解析、静态类型检查、动态声明和动态 selector 展开契约。"
weight = 10
+++

# `css!`

`css!` 是局部 CSS 的过程宏入口。它把声明编译成带稳定 class 的 CSS，
并把响应式声明值或 selector 转换为 `silex_css` 的动态绑定。宏定义在
`crates/silex_macros/src/css.rs`，解析和编译分别位于 `css/ast.rs` 与
`css/compiler/`。

## 输入形式

下面的片段展示语法形状；它依赖调用方的 `silex` facade 和 owner 上下文，
不是可单独编译的文档 example。

```rust
let class_name = css! {
    color: red;
    padding: 8px;
    &:hover { color: blue; }
};

let dynamic = css!(error_handler; {
    width: $(width);
    $selector { color: $(color); }
});
```

宏接收 CSS block，不要求最后一条声明带分号；编译器会统一补上分号，
因此是否书写尾分号不会改变最终样式的 class identity。声明属性名由
属性表解析，短别名如 `p`、`bg` 和 `text` 也由同一张表处理；自定义变量
`--name` 与厂商前缀属性走无类型透传路径。

## 插值语义

声明值支持三种写法：

| 写法 | 语义 | 约束 |
| --- | --- | --- |
| `$ident` / `$path` | 响应式 source | 表达式必须属于当前 owner，并能转换成 CSS reactive source |
| `$(expr)` | 响应式 source | 可放入值片段；整条值时还会按属性能力校验 |
| `$(static path)` | 编译期静态值 | 只接受 Rust path，值需满足 `StaticCssValue` 和当前属性的 `ValidFor` |

响应式声明值通常编译成 `.class { property: var(--class-N) }`，再在元素上
由 owner effect 更新对应 custom property。因此动态值本身不参与 class
结构哈希；同一 CSS 结构可以复用同一个静态规则。动态 selector 没有元素
可以挂 custom property，会编译成带 selector placeholder 的 `DynamicCss`
规则，由动态样式表管理器更新。

动态输出必须显式提供错误边界：`css!(error_handler; { ... })` 的第一个
参数可以是 `ErrorReporter`、`ErrorHandler` 或 `ErrorHandlerToken` 能接受的
表达式。宏不会创建隐式 `Runtime`，也不会把 foreign runtime 的 source
自动转入当前 owner。

返回形状由动态内容决定：

- 完全静态：返回 class 字符串，并注入静态 CSS。
- 只有动态声明值：返回 `DynamicCss`，错误在后续应用/读取时通过 handler
  报告。
- 含动态 selector：返回 `SilexResult<DynamicCss>`，因为动态 rule 的值
  getter 在构造时需要处理错误。

## 嵌套规则与 at-rule

- 静态嵌套 selector 直接进入组件 CSS；`&` 会被替换成生成的 class。
- `$selector` 或 `$(selector)` 使该规则成为动态 selector。selector 片段
  会经过 selector 专用净化，不能借助动态值打开新的 CSS 声明边界。
- `@media`、`@supports` 等块规则可以嵌套。语句式 at-rule（例如
  `@import`、`@charset` 和 `@layer a, b;`）会被提升到 class 外层。
- `@keyframes` 和 `@font-face` 在局部样式中也会被提升；它们不能留在
  `.class { ... }` 内。
- 动态 selector 内不能放语句式 at-rule；运行时无法把全局语句安全地放进
  一条动态 selector 规则。

启用 `silex_macros/tw` 时，`@apply flex items-center;` 会先经过 Tailwind
parser 和 codegen，再作为机器生成的声明并入当前 CSS block。`@apply` 的
生成属性不会重复走用户 CSS 的静态属性表校验，但原始 utility 仍必须能被
Tailwind parser 解析。

## 静态校验和 `unsafe`

静态 CSS 有三层编译期校验：属性名、裸关键字/函数能力和顶层分量个数。
能直接识别的 `px`、颜色等值还会生成 `ValidFor<Property>` 断言。校验失败
应在宏调用处报错，而不是把无效声明留给浏览器。

`unsafe { ... }` 是局部逃生舱：它关闭当前块的属性和值校验，并保留 CSS
文本生成和运行时边界净化。项目级 `[css.validation]` 可以分别把
`keywords`、`functions`、`arity` 设为 `error`、`warn` 或 `off`；这只调整
静态校验层级，不改变响应式 owner 约束。

## 维护与测试

修改 `css!` 时应同时检查：

- `ast.rs` 的 `$` 插值分类是否把 `$(static path)` 与响应式表达式分开；
- `compiler/parser.rs` 的 placeholder 序号是否与生成 getter 一一对应；
- `generate_css_output` 对静态、动态声明和动态 selector 的返回形状是否仍与
  `DynamicCss` API 一致；
- `escape`、`DynamicStyleManager` 和 owner cleanup 是否覆盖 selector/value
  的更新与释放。

针对宏入口的 pass/fail 用例在
`crates/tests/silex_macros_test/tests/ui/`，owner 运行时用例在
`crates/tests/silex_macros_test/tests/macro_owner.rs` 和
`scoped_css_macros.rs`。只修改本页文档不需要运行测试；修改可执行文档
example 时按 `silex_css --test docs_examples` 的约定单独验证。
