+++
title = "theme!"
description = "theme! 的主题类型、CSS 变量映射、silex.toml 配置补全和 Patch 展开契约。"
weight = 60
+++

# `theme!`

`theme!` 在 `crates/silex_macros/src/css/theme.rs` 中把 Rust struct 声明
转换为 `silex_css` 的主题类型。它生成主题字段、CSS 变量名称、
`ThemeType`/`ThemeToCss` 实现、变量常量和对应的 Patch 类型；真正把主题
应用到 `:root` 或元素的 effect 由 `silex_css::theme` 运行时负责。

## 显式字段

```rust
theme! {
    #[theme(prefix = "app", main)]
    pub struct AppTheme {
        #[theme(var = "--brand-primary")]
        pub primary: Hex,
        pub radius: Px,
    }
}
```

上面是依赖 `Hex`、`Px` 和 CSS facade 的语法示意，不是独立 CI example。
字段必须是 named field。字段上的 `#[theme(var = ...)]`
只改变 CSS variable 名，其他字段属性会保留到生成的 struct；struct 上的
`#[theme(...)]` 会被宏消费，不会原样复制。

变量名的默认优先级为：

1. 字段的 `#[theme(var = "...")]`；
2. struct 的 `#[theme(prefix = "...")]`；
3. `silex.toml` 的 `[theme].prefix`；
4. 内置 `slx-theme`。

默认变量名把字段下划线转成 CSS 连字符，例如 `brand_primary` 对应
`--slx-theme-brand-primary`。`#[theme(main)]` 还会生成当前主题类型的
`Theme` 类型别名。

## 生成物

对于 `AppTheme`，宏会生成以下公开契约：

- `AppTheme`：`Clone`/`Debug`，无配置补全时还实现 `Default`；
- `AppTheme::PRIMARY` 等大写常量：类型为 `CssVar<FieldType>`，值引用
  `var(--...)`，可以参与类型安全的 CSS 声明；
- `AppThemeFields`：把字段映射为相同的类型关联项；
- `ThemeType` 和 `ThemeToCss`：提供变量名数组和值数组；
- `AppThemePatch`：字段为 `Option<FieldType>`，并生成链式字段 setter；
- `ThemePatchToCss` 和 `Display`：把完整主题或 patch 渲染为 CSS 声明。

`ThemeToCss::get_variable_names()` 与 `get_variable_values()` 按字段声明
顺序一一对应。运行时会检查二者长度；维护宏时不能改变其中一个列表的
顺序而不同时改变另一个。

## `silex.toml` 驱动的字段

当 `theme!` 的 struct 没有字段且项目配置存在 `[theme.colors]` 时，宏会
按排序后的配置 key 补出字段，`-` 会转换为 Rust 字段名中的 `_`：

```toml
[theme.colors]
brand-primary = "#6366f1"
radius = "8px"

[theme.field_types]
radius = "Px"
```

配置字段的类型规则：

- 没有 `[theme.field_types]` 时，形如 `#RGB`、`#RGBA`、`#RRGGBB` 或
  `#RRGGBBAA` 的值推断为 `Hex`，其它值推断为 `String`；
- 显式类型名如 `Hex`、`Px` 解析到 `silex::css::types`，`String` 解析到
  `std::string::String`，带 `::` 或泛型的类型路径原样解析；
- 量纲类型会从配置初值构造，例如 `8px` + `Px` 变为 `px(8)`；单位后缀
  写错会编译失败，不会静默当成另一个单位；
- 未知类型没有统一字符串构造器时，字段默认值回退到该 Rust 类型的
  `Default`。

宏通过 `include_bytes!` 生成 `silex.toml` 依赖 token，使配置改变时 Cargo
能重新运行过程宏。配置解析错误也不会被静默忽略。

## 与 runtime 的配合

- 完整主题使用 `theme_variables(theme)` 应用到元素，runtime 首次写入全部
  变量，后续只 diff 变化项；owner cleanup 会移除它写过的变量。
- Patch 使用 `theme_patch(patch)` 只覆盖选中的变量。Patch 中的 `None` 和
  上一轮不再出现的变量都走 `removeProperty`，恢复 CSS inheritance。
- `Theme::PRIMARY` 这种 `CssVar<T>` 可以放进 `css!`、`styled!` 的静态
  插值；它不是把完整主题对象直接塞进 `inject_css!` 的动态通道。

## 维护与测试

修改主题宏时必须同时检查字段补全、默认值、prefix 优先级、变量顺序、
Patch 字段和配置依赖 token。实现单元测试位于
`crates/silex_macros/src/css/theme.rs` 的 `tests` 模块；
`pass_macro_theme_patch.rs`、`pass_macro_static_interpolation.rs` 和
`docs/content/developer/crates/silex_css/theme.md` 对应 runtime 契约。

主题变量的 diff、`None` 移除和 owner cleanup 由
`crates/silex_css/src/theme.rs` 及 `tests/owner.rs` 验证；修改宏生成的 trait
签名时不要只运行过程宏本身的 token string 测试。
