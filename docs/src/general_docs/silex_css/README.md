# Silex CSS：极致性能的类型安全样式库

`silex_css` 是 Silex 框架的核心组件之一，它为 Rust Web 开发提供了**原生的类型安全 CSS 体系**。

在 Silex 中，CSS 不再是脆弱的字符串拼接，而是具有强类型保障、自动性能优化和零运行时损耗（Zero-runtime overhead）的现代化基础设施。

## 为什么选择 Silex CSS？

*   **编译期类型校验**：通过 `px(10)`, `rem(1.2)`, `hex("#fff")` 等包装类型，彻底杜绝了单位遗漏或属性写错等低级错误。
*   **极致性能更新**：动态样式优先转化为 **CSS 变量**，通过响应式信号实现原子化更新，避开复杂的 DOM 重新解析。
*   **零 DOM 损耗**：采用最先进的 **Adopted StyleSheets** 技术，样式完全驻留在内存中，不再向 `head` 注入成堆的 `<style>` 标签。
*   **智能自动回收**：内置 LRU 缓存与弱引用机制，自动清理不再使用的样式规则，保障长效运行下的内存安全。
*   **显式空值处理**：所有属性类型内部均使用 `Option` 包装，支持 `Default` 生成“未设置”状态，避免了强制赋予默认数值导致的冲突。

---

## 1. 快速上手

在 Silex 中，你可以使用多种方式编写样式。最简单的方法是使用 `css!` 宏或纯 Rust API `sty()`。

### 类型安全的属性值
Silex 要求显式指定单位，这不仅能获得 IDE 的自动补全，还能在编译阶段拦截错误。

```rust
use silex::css::prelude::*;

// 声明响应式变量
let width = Signal::pair(px(200));
let color = Signal::pair(hex("#4f46e5"));

// 使用 css! 宏
let base_cls = css!("
    width: $(width);
    background-color: $(color);
    padding: 1rem;
    &:hover { filter: brightness(1.1); }
");

div("Hello Silex").class(base_cls)
```

> **原理说明**：当 `width` 信号变化时，Silex 不会修改 `.class` 里的规则，而是仅调用一次 `style.setProperty("--v-width", "200px")`。这种变量级的更新几乎是浏览器能达到的最高效率。

---

## 2. 纯 Rust 样式构建器 (Style Builder)

如果你更喜欢纯粹的 Rust 语法，或者希望获得更极致的类型提示，可以使用 `sty()`（`Style::new()` 的简写）。

```rust
use silex::css::prelude::*;

div("用 Builder 构建的样式")
    .style(
        sty().display(DisplayKeyword::Flex)
            .justify_content(JustifyContentKeyword::Center)
            .background_color(hex("#f3f4f6"))
            .on_hover(|s| s.background_color(hex("#e5e7eb")))
    )
```

**全面对齐宏的功能：**
*   **IDE 友好**：每一个方法都有明确的参数类型要求。
*   **复杂嵌套**：使用 `.nest("& > div", |s| ...)` 支持任意选择器嵌套。
*   **响应式设计**：使用 `.media("@media (max-width: 600px)", |s| ...)` 直接定义断点样式。
*   **零损耗更新**：即使是深层嵌套中的信号，依然通过原子级的 CSS 变量进行更新。

```rust
sty().width(px(200))
    .on_hover(|s| s
        .background_color(hex("#f3f4f6"))
        .nest("& > .icon", |s| s.opacity(0.8)) // 复杂嵌套
    )
    .media("@media (max-width: 768px)", |s| s // 媒体查询
        .width(pct(100))
    )
```

---

## 3. 复合属性工厂

为了简化繁琐的组合属性（如 `margin`, `border`），`silex_css` 提供了工厂函数：

```rust
use silex::css::prelude::*;

let border_val = border(px(2), BorderStyleKeyword::Solid, hex("#3b82f6"));
let pad_val = padding::x_y(px(16), px(32)); // 水平 16px, 垂直 32px

styled! {
    pub MyBox<div> {
        border: $(border_val);
        padding: $(pad_val);
    }
}
```

---

## 4. 复杂属性 DSL (Complex Properties)

对于 `transform` 或 `grid-template-areas` 等语法极其复杂的属性，Silex 提供了专门的 DSL（领域专用语言）来确保输入的正确性。

### 变换 (Transform)
支持链式调用，无需手动拼接字符串，且会自动验证单位。

```rust
sty().transform(
    transform()
        .translate(px(10), px(20))
        .rotate(deg(45))
        .scale(1.2)
)
```

### 网格区域 (Grid Template Areas)
通过 Rust 数组/向量声明布局，自动处理引号包裹。

```rust
sty().grid_template_areas(
    grid_template_areas(["header header", "main sidebar"])
)
// 生成: grid-template-areas: "header header" "main sidebar";
```

### 字体变体 (Font Variation Settings)
为变体字体（Variable Fonts）提供结构化输入。

```rust
sty().font_variation_settings(
    font_variation_settings([("wght", 700.0), ("ital", 0.5)])
)
```

---

## 5. 计算属性与运算符重载

Silex CSS 允许你像编写原生 CSS 一样进行数值计算。通过重载算术运算符，你可以直接组合不同的单位。

### 算术运算
```rust
use silex::css::prelude::*;

let width = px(100) + rem(2); // 自动生成 (100px + 2rem)
let half = width / 2.0;       // 自动生成 ((100px + 2rem) / 2)
```

### 现代 CSS 函数
完全支持 `calc()`, `min()`, `max()` 和 `clamp()`，且具有编译时类型检查。

```rust
use silex::css::prelude::*;

sty().width(clamp(px(200), pct(50), px(800)))
     .font_size(min(vec![rem(2), vw(5)]))
     .margin_top(calc(px(100) - rem(1)));
```

---

## 6. 主题系统 (Theme System)

传统的样式框架在实现主题方案时，往往需要包裹一层 `<div>` 主题容器，这会破坏 Flex/Grid 布局。Silex 的主题系统通过 **CSS 变量注入** 巧妙解决了这个问题。

### 定义与应用主题
使用 `define_theme!` 宏定义主题，它会自动生成配套的补丁结构体用于局部微调。

```rust
define_theme! {
    #[theme(prefix = "slx")]
    pub struct AppTheme {
        pub primary: Hex,
        pub surface: Hex,
    }
}

// 方式 A：全局生效（应用于 :root）
// 接收实现 IntoSignal 的任意类型（信号、常量、rx! 闭包）
set_global_theme(theme_sig);

// 方式 B：局部补丁（增量覆盖）
// 仅修改 primary 颜色，其他变量自动从环境继承
let patch = rx!(|| AppThemePatch::default().primary(hex("#ff69b4")));
Stack(children)
    .apply(theme_patch(patch)) 
```

### 在组件中使用主题
```rust
// 获取全局主题信号
let t = use_theme::<AppTheme>();

div("主题文字").style(sty().color(t.map(|v| v.primary.clone())))
```

---

## 7. 核心引擎与架构

`silex_css` 的高性能离不开其底层的**中心化注册机制**：

1.  **静态提升**：所有纯静态的 CSS 规则会被自动提取，合并到一个全局唯一的 `CSSStyleSheet` 中，避免重复解析。
2.  **异步同步 (Async Sync)**：样式注入操作通过微任务队列进行批处理，确保即使在一帧内创建大量组件，也只触发一次浏览器的样式重计算。
3.  **内存在管理**：不使用 `<style>` 标签，意味着样式表对 DOM 树不可见且无法直接通过字符串检索，减少了大型应用中 DOM 树的压力。

## 小结

`silex_css` 的设计哲学是将 CSS 的灵活性与 Rust 的安全性深度融合。无论你是追求极致开发体验（使用 `css!`），还是极致性能提示（使用 `sty()`），它都能在保障类型安全的同时，为你提供行业一流的渲染性能。

建议下一步阅读：[silex_macros 宏指南](../silex_macros/README.md) 或 [深入组件样式化](../chapter_styling.md)。

