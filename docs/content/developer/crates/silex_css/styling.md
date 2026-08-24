+++
title = "类型安全样式与 Style"
description = "silex_css 的 Style builder、CSS 值能力、嵌套规则、class 组合和响应式声明。"
weight = 10
+++

# 类型安全样式与 `Style`

`Style<'scope>` 把一组 CSS 声明表示为可应用到 `silex_view::Element` 的
attribute operation。它用 `ValidFor<Property>` 在 Rust 编译期限制值类型，用
`CssSource<'scope, T>` 把静态值和 owner 绑定的响应式值统一到 builder 中。
最终结果不是直接写入 `style="..."` 的整段字符串，而是一个稳定 class、
静态 CSS 规则和必要的动态绑定。

## 创建与应用

`sty(ctx)` 和 `Style::new(ctx)` 等价；`ctx` 必须能提供当前 scope 的
`ErrorReporter`。每个 builder 方法都返回 `SilexResult<Style>`，因此链式
调用要在每一步使用 `?`：

```rust
let style = sty(ctx)
    .display(DisplayKeyword::Flex)?
    .gap(px(8))?
    .color(rgb(29, 78, 216))?
    .on_hover(|style| style.color(rgb(30, 64, 175)))?;
```

`Style` 实现 `silex_view::attribute::ApplyToDom`，所以可以作为 View 元素的
`.style(...)` 或 `.apply(...)` 属性操作：

```rust
let view = Element::with_child("button", "styled").style(style);
context.mount_unit(view, error_handler)?;
```

上面两个片段依赖外层的 mount scope，属于 API 关系说明，不是独立的 CI
示例；完整 owner 创建流程见总览页的 `basic.rs`。

## CSS 值能力

属性方法由 `for_all_properties!` 注册表生成。例如 `width` 接受长度、
百分比和长度计算，`color` 接受颜色类型，`rotate` 接受角度；不兼容的
组合会在编译期失败，而不是等浏览器丢弃声明。

| 类型 | 代表 API | 约束/用途 |
| --- | --- | --- |
| 长度/百分比 | `px(8)`、`rem(1.25)`、`pct(50)` | 实现 `CssLength` 或 `CssLengthPercentage`。 |
| 角度/时间 | `deg(90)`、`sec(0.3)`、`ms(150)` | 分别只进入 angle/time 属性和对应计算。 |
| 颜色 | `rgb(...)`、`rgba(...)`、`hex(...)`、`hsl(...)` | 通过 `CssColor` 进入颜色语法。 |
| 关键字 | `DisplayKeyword::Flex`、`AUTO`、`INHERIT` | 由属性注册表生成或全局复用。 |
| 计算 | `px(100) - px(16)`、`css_min!(...)` | 同一量纲才能相加减；长度百分比允许合法混合。 |
| 简写/复杂值 | `border(...)`、`margin::block_inline(...)`、`transform()` | 构造器内部再次校验组成部分。 |
| CSS 变量 | `css_var("--brand")`、`CssVar<Hex>` | `CssVar<T>` 可保留属性能力；`CssVar<()>` 是无类型入口。 |
| 可取消值 | `css_some(px(4))`、`css_none::<Px>()` | `None` 的 CSS 表示是 `unset`，不是空声明。 |

`CssUnsafe`/`css_unsafe(...)` 和 `Style::raw(...)` 是明确的 escape hatch。
它们允许注册表尚未覆盖的厂商属性、较新的 CSS 语法或复杂字符串；调用方
仍需负责语义正确性。`raw` 的属性名和值会通过 `escape` 模块净化，不能
借助 `;`、括号或 selector 字符打开另一条声明。

## 静态值、动态值和生命周期

`IntoCssSource<'scope>` 的静态实现覆盖内置 CSS 值、数字、字符串和
`CssVar`。`Rx`、`ReadSignal`、`Signal`、`Computed`、
`StoredValue` 会转换成 `CssSource::Reactive`，其值必须实现 `Display +
Clone + 'scope`：

```rust
let size = owner.signal(px(8))?;
let style = sty(ctx).width(size.read_signal().into_rx())?;
size.set(px(12))?;
```

动态值不会在 `sty` 中创建隐式 runtime。它只携带当前 owner 的 `Rx`，并由
应用样式时注册 effect；source 属于 foreign runtime、owner 已关闭或读取
失败时，错误交给 `ErrorReporter`/mount error handler。

局部动态声明的生成过程是：

```text
Style::width(rx)
    │
    ├── class rule: width: var(--sb-<hash>-0)
    └── owner effect: element.style.setProperty("--sb-<hash>-0", value)
```

effect 只在值发生变化时更新对应 property。cleanup 移除它拥有的变量和
class；它不会清理其他样式源写入的同名变量，因此多个 attribute operation
可以共存时必须避免使用相同的自定义属性名。

## 嵌套选择器和伪类

`Style::media(query, f)` 生成 `@media` 包裹规则。`nest(selector, f)` 遵守
CSS Nesting：带 `&` 时替换为基础 class，不带 `&` 时按后代关系展开：

```rust
let style = sty(ctx)
    .nest("& > span", |style| style.color(rgb(15, 23, 42)))?
    .nest(".icon", |style| style.width(px(16)))?
    .pseudo(":focus-visible", |style| style.outline("2px solid currentColor"))?;
```

结果关系分别类似 `.slx-x > span`、`.slx-x .icon` 和
`.slx-x:focus-visible`。因此 `nest(":hover", ...)` 不等价于自身伪类；
`on_hover`、`on_active`、`on_focus`、`on_focus_visible`、`on_disabled` 是
自身附着伪类的便捷方法。

每个静态或动态结构都会参与 class hash，动态值本身用占位符而不参与基础
class 的结构哈希。这样同一调用点的结构可以复用 class，值更新只更新
inline 变量或动态规则 class。

## class 组合

`IntoClass` 支持 `&str`、`String`、`Cow<str>`、`Option<T>`、条件 tuple
以及可嵌套的数组/切片形式；`cx!` 将它们写成空格分隔的 class 字符串：

```rust
let classes = cx!(
    "button",
    (is_primary, "button-primary"),
    (is_disabled.then_some("opacity-50")),
);
```

`cx!` 只负责 class 文本组合，不创建 CSS，也不拥有 cleanup。响应式 class
应使用 `silex_view` 的 reactive attribute 或 `tw!`/宏生成的绑定，确保更新
和移除属于同一个 owner。

## 维护边界

- 新 CSS 属性应先更新 `silex_codegen` 使用的 MDN 数据/生成链，再检查
  `ValidFor` 的能力集合和 `css_type_safety` UI 测试；不要手写一个与注册表
  冲突的实现。
- 新的复杂值必须实现 `Display`、对应属性的 `ValidFor`，并为字符串、引号、
  括号或数值边界添加测试。`hex` 对字面量错误会 panic；不可信输入应使用
  `Hex::try_new`。
- 修改嵌套选择器、动态值变量名或 hash 输入时，要同时检查静态 CSS、
  `DynamicCss` 和 owner dispose 后的 class/inline style 清理。
