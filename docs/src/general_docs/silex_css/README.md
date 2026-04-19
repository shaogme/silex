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

// 使用 css! 宏 (现在支持代码块语法)
let base_cls = css! {
    width: $(width);
    background-color: $(color);
    padding: 1rem;
    &:hover { 
        filter: brightness(1.1);
        transform: scale(1.02);
    }
};

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

传统的样式框架在实现主题切换时，通常依赖外层类名切换或 JS 环境注入。Silex 提供了一个高度原生的、基于 **CSS 变量注入** 的强类型主题系统，它不仅性能极高，而且支持极致的代码补全和类型校验。

### 6.1 定义主题
使用 `theme!` 宏定义主题结构。通过 `#[theme(main)]` 标记主主题，宏会自动生成 `Theme` 类型别名，供其他样式宏自动识别。

```rust
theme! {
    #[theme(main, prefix = "slx")]
    pub struct AppTheme {
        pub primary: Hex,     // 颜色类型
        pub radius: Px,       // 尺寸类型
        pub surface: Hex,
    }
}
```

> **提示**：Silex 现已全面转向基于路径的变量引用语法。你可以通过 `$AppTheme::PRIMARY` 等语法安全地引用由 `theme!` 生成的常量。

### 6.2 强类型变量引用 (推荐)
宏生成的常量（如 `AppTheme::PRIMARY`）具有 `CssVar<Hex>` 类型，并在编译期继承 `Hex` 的校验规则。

```rust
// ✅ 合法：primary 是颜色，可以传给 color()
sty().color(AppTheme::PRIMARY)

// ❌ 编译报错：无法将颜色传给 width()
// 错误信息：类型 `CssVar<Hex>` 无法作为有效的 CSS `Width` 属性值使用
sty().width(AppTheme::PRIMARY) 

// ✅ 合法：radius 是尺寸，支持算术运算
sty().border_radius(AppTheme::RADIUS + px(4))
```

### 6.3 应用主题
Silex 支持全局挂载和局部补丁，所有操作均通过 `.apply()` 注入，不产生额外的 DOM 包装层。

```rust
// 1. 全局主题 (应用于 :root)
// 支持信号、常量或 rx! 闭包
set_global_theme(rx!(move || {
    if is_dark.get() { default_dark_theme() } else { default_light_theme() }
}));

// 2. 局部补丁 (增量覆盖)
// 仅修改 primary 变量，其余变量自动从环境继承 (CSS Inheritance)
let patch = rx!(|| AppThemePatch::default().primary(hex("#ff69b4")));
div("粉色主题区域").apply(theme_patch(patch))
```

### 6.4 获取主题状态
如果你需要在 Rust 逻辑中直接访问当前的变量数值（而非仅仅引用变量名）：

```rust
let theme = use_theme::<AppTheme>();
let is_dark = theme.map(|t| t.surface == "#111827");
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

