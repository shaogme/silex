+++
title = "主题与 CSS 变量"
description = "silex_css 的主题 trait、全局 :root 注入、局部变量和 patch 清理语义。"
weight = 30
+++

# 主题与 CSS 变量

`silex_css` 的主题层不直接替换组件样式，而是把主题值映射为 CSS custom
properties。组件规则引用 `var(--name)`，主题变化只更新变量，因而保持
CSS 规则结构和组件 class 不变。主题对象可以是静态值，也可以是当前 owner
中的 `Signal`、`Computed` 或其他 `IntoCssSource`。

## 两个主题 trait

实现主题通常由 `theme!` 宏完成；运行时契约由两个 trait 表达：

```rust
pub trait ThemeType {}

pub trait ThemeToCss: Display {
    fn get_variable_values(&self) -> Vec<String>;
    fn get_variable_names() -> &'static [&'static str];
}
```

`ThemeType` 是宏和属性边界使用的 marker。`ThemeToCss` 要求 names 与 values
按同一顺序、一一对应；运行时会检查长度，不匹配返回 framework error。
变量名应使用 `--` 开头，并且值应只表达 CSS value，不要把整条规则放入
value。

## 局部主题变量

`theme_variables(theme)` 返回一个可作为 DOM attribute 应用的
`ThemeVariables`：

```rust
let theme = owner.signal(AppTheme::light())?;
theme_variables(theme).apply(
    &element,
    ApplyTarget::Apply,
    &owner_token,
    error_handler.view(),
)?;
```

实际调用依赖 `silex_dom` 的 attribute API；片段只说明 source 到 DOM 的
关系。首次 effect 写入全部变量，后续只对变化的 `(name, value)` 调用
`setProperty`。主题 owner 清理时移除它写入的变量，使父元素继承恢复。

`None` 不是把变量写成空字符串，而是调用 `removeProperty`。这允许变量
回到 CSS inheritance；写空字符串可能触发 computed-value invalid，而不等
于“没有局部覆盖”。

## 全局 `:root` 主题

`set_global_theme(owner, theme, error_handler)` 使用 owner 绑定的 effect，
把当前主题渲染为 `:root{--name:value;}` 并交给 `DynamicStyleManager`：

```text
Theme source
    │ get()
    ▼
theme_entries ──► :root{...}
    │
    ▼
DynamicStyleManager.update(unique_id, css)
    │ owner cleanup
    ▼
dispose / remove stylesheet
```

只有 CSS 文本发生变化时才更新样式表。初始化失败会清理已经创建的 manager，
owner 关闭也会释放全局样式表；因此全局不代表永久存在，它仍然属于传入
owner 的生命周期。

## 局部 patch 与继承

完整主题适合初始化一棵子树；需要只覆盖少数变量时实现
`ThemePatchToCss`：

```rust
pub trait ThemePatchToCss {
    fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)>;
}
```

通过 `theme_patch(patch)` 应用时，patch 的当前变量集合与上一轮按名称 diff：

- 新变量或值变化：`setProperty(name, value)`；
- `None`：`removeProperty(name)`，回到继承值；
- 上一轮存在、本轮不再返回的名字：同样移除；
- owner cleanup：移除本 patch 历史上写过的全部变量。

patch 的变量名可以随响应式值变化，因此 cleanup 需要保存运行中见过的
名字，而不能只依赖初始 patch 的字段列表。

## 宏生成与静态样式

`theme!` 位于 `silex_macros`，通常生成：

- 主题 struct 和构造/默认值；
- `ThemeType` marker；
- `Display` 与 `ThemeToCss` 实现；
- 供静态 CSS 插值和 `theme_variables` 使用的变量名和值映射。

宏生成的静态 CSS 仍通过 `inject_style` 进入静态 registry；主题本身在
runtime 中更新时则使用动态 manager 或元素 inline style。不要把运行时主题
对象直接拼到 `inject_css!`：文档级静态宏拒绝动态值，原因是它必须先经过
静态 CSS 编译和压缩。

## 安全和维护边界

`global_theme_css`、局部变量和 patch 都会对变量名和值执行声明边界净化，
防止值里的 `; } body { ...` 变成额外规则。但净化不保证业务语义正确；
主题值仍应来自受约束的 CSS 类型或可信构造器。

修改主题 trait、变量 diff 或 cleanup 时，必须覆盖：首轮全写、值不变不重写、
`None` 移除、变量集合缩短/变长、名称和值数量不一致，以及恶意声明值不能
打开新规则。对应的逻辑测试在 `src/theme.rs`，浏览器行为在
`tests/owner.rs`。
