# Silex CSS 工具库

`silex_css` 提供了 Silex 框架强大且极具性能优化的的 CSS 类型安全体系、样式构建器以及主题注入系统。它旨在替代容易导致错误的传统字符串写法的 CSS。
您可以直接通过 `silex::prelude::*` 或 `silex::css::*` 获取本库提供的方法。

## 1. CSS 编写 (`css!` 与 `styled!`)
Silex 拥有极为强大的**原生态类型安全 CSS 体系**！避免了一般框架所面临的字符串拼接引发的各类不安全 CSS Bug。由于其杜绝了通配符隐式字符串转化逃逸，您需要显式地通过我们提供的 Builder 或 Enum 类型组合属性。

**基础插值：**
```rust
use silex::css::types::{px, pct, hex};

let w = signal(px(100));
let c = signal(hex("#ff0000"));

let cls = css!("
    color: $(c); 
    width: $(w); /* 编译期类型校验，保障不会错写成单纯数字或者错用其他强单位 */
    &:hover { color: blue; }
");
div("Hello").class(cls)
```
**性能注记：** 所有的动态插值 $(...) 现在大部分都通过 **CSS 变量 (CSS Variables)** 进行高效更新。这意味着当信号变化时，框架仅调用一次极轻量的 `style.setProperty`，而无需操作 DOM 结构，在高频更新场景下性能表现极其卓越。
对于无法用内联变量表示的插值（例如嵌套伪类中的动态值），Silex 内置了拥有**引用计数 (Reference Counting)** 及 **LRU 缓存回收**机制的 `DynamicStyleManager`，它会在后台自动计算哈希生成独特类名并复用 `<style>` 标签，既实现了无死角的完全响应式，又使得长期运行的应用不至于出现 `<style>` DOM 节点污染与内存溢出。

**复杂复合类型（工厂与 Builders）：**
使用专用模块工厂快速安全打包例如 `margin`，`border` 等复合元素。
```rust
use silex::css::types::{border, padding, BorderStyleKeyword};

let border_style = signal(border(px(1), BorderStyleKeyword::Solid, hex("#ccc")));
let pad = signal(padding::x_y(px(8), px(16)));

styled! {
    pub StyledDiv<div>(
        #[prop(into)] p_val: Signal<UnsafeCss>, // 如果确实需要越过系统拦截
    ) {
        border: $(border_style);
        padding: $(pad);
        margin: $(p_val);

        variants: {
            size: {
                small: { font-size: 12px; }
                large: { font-size: 20px; }
            }
        }
    }
}
```

## 2. 样式构建器 (Style Builder)

除了宏，`silex_css` 还提供了一套纯 Rust 的样式构建 API，适用于希望完全避免过程宏开销、或需要极致类型安全提示的场景。

```rust
use silex::css::builder::Style;
use silex::css::types::{px, hex, DisplayKeyword};

let (width, _) = signal(px(200));

div("I am styled by Builder")
    .style(
        Style::new()
            .display(DisplayKeyword::Flex)
            .width(width) // 支持响应式信号
            .background_color(hex("#f0f0f0"))
            .padding(px(20))
            .on_hover(|s| { // 支持伪类
                s.background_color(hex("#e0e0e0"))
                 .color(hex("#00bfff"))
            })
    )
```

**对比优势：**
*   **零开销**：不使用过程宏进行字符串解析，纯泛型展开，编译速度极快。
*   **强类型提示**：Rust Analyzer 可以准确提示每一个属性的合法参数（如 `Display` 只能传 `DisplayKeyword` 枚举）。
*   **CSS 变量级优化**：静态样式自动提升到 `<head>` 共享；动态属性自动绑定为 CSS 变量，通过响应式 Effect 进行原子化更新，避免重绘抖动。

## 3. 主题注入 (Theme System)
为了解决深层组件组件主题透传问题且杜绝包裹产生多余的 `<div class="theme-provider">` DOM 节点致使 `Flex/Grid` 失效：
```rust
// 通过宏预先构建具有强类型校验保障的系统 （细节见 silex_macros 文档）
#[theme(MyTheme)]
struct AppTheme { ... }

let my_theme_signal = signal(AppTheme { ... });

// 1. 全局生效方案:
set_global_theme(my_theme_signal); 

// 2. 将主题直接应用（注入 CSS Vars）到已经建立在流里的组件上进行范围挂载:
Stack(...)
    .apply(theme_variables(my_theme_signal))
```
