# Silex CSS 核心实现分析

`silex_css` 是 Silex 框架中负责样式处理的核心 crate。它不仅提供了一套强类型的 CSS 构建系统，还包含了一个高性能的 CSSOM 运行时，旨在解决传统 Web 框架在处理样式时面临的性能瓶颈、类型安全缺失以及 DOM 压力问题。

## 1. 概要 (Overview)

*   **定义**：一个集成了强类型验证、零宏构建器 (Zero-macro Builder) 以及现代 CSSOM 运行时的 CSS 管理引擎。
*   **作用**：在 Silex 架构中，`silex_css` 为 `silex_macros` 提供的 `styled!` 宏以及用户直接使用的 `sty()` 构建器提供底层支持。它负责将 Rust 代码中的样式描述转换为高效的浏览器指令（如 `adoptedStyleSheets` 和 `setProperty`）。
*   **目标受众**：本文档主要面向希望了解 Silex 样式系统底层优化机制的贡献者。阅读前建议了解现代浏览器的 `Constructable StyleSheets` API 以及 Rust 的响应式系统基础。

## 2. 理念和思路 (Philosophy and Design)

*   **设计背景**：传统的样式更新方案（如修改 `className` 或内联 `style` 字符串）会触发大规模的重计算（Recalculate Style）和解析压力。同时，动态样式的生命周期管理一直是前端框架的难点，容易导致内存泄漏。
*   **核心思想**：
    *   **零 DOM 压力**：样式注入走 `adoptedStyleSheets`，不往 DOM 里塞 `<style>` 标签。仅当 `new CSSStyleSheet()` 不可用时才退回一个 `<style>` 兜底——那条路只保证「样式不丢」，不保证性能。
    *   **极简更新路径**：对于动态样式，优先使用 CSS 变量（CSS Variables）进行占位，更新时仅触发轻量级的 `element.style.setProperty`。
    *   **编译时安全**：利用 Rust 的 ZST (Zero-Sized Types) 和 Trait 系统，在编译期拦截非法的属性赋值（如将 `Color` 传给 `Width`）。每个属性允许哪些值类型由 MDN 的值定义语法解析得出，而不是靠人工分组——细节见 §4.1。这条保证覆盖两条路：`sty()` 的每个方法（`V::Value: ValidFor<props::X>`），以及 `css!` / `styled!` 里的每一条声明——属性名过注册表白名单（`colr: red` 会报错并给出 `color` 的建议），`$(expr)` 过 `ValidFor`，**静态取值**在「一眼能定型」时也会生成一条编译期断言（`color: 10px` → `Px` vs `props::Color`，编译失败）。已知缺口：定不了型的静态取值（`color: rgb(0 0 0)`、裸关键字）不生成断言——宁可漏报也不能把合法 CSS 拒之门外。
    *   **自动化生命周期**：结合弱引用（Weak References）和 LRU 缓存，实现样式的自动注入与销毁。
*   **方案取舍 (Trade-offs)**：
    *   **为什么不使用内联样式？** 内联样式无法处理伪类（`:hover`）、伪元素（`::before`）和媒体查询。
    *   **为什么不使用 CSS-in-JS 常见的 Hash 方案？** 传统的 Hash 方案在属性变更时需要生成新的类名并注入新的样式表，开销巨大。Silex 通过“静态结构 Hash + 动态变量注入”的组合方案，兼顾了功能完整性和性能。

## 3. 模块内结构 (Internal Structure)

```text
src/
├── builder.rs          // 零宏 Style 构建器系统
├── types/              // 强类型属性与单位系统
│   ├── units.rs        // CSS 单位（长度/角度/时间/网格）与颜色类型
│   ├── calc.rs         // 计算属性 (calc, min, max, clamp)、量纲标记及运算符重载
│   ├── shorthands.rs   // 基础复合属性工厂 (Border, Margin等)
│   ├── gradients.rs    // 渐变生成器 DSL
│   └── complex.rs      // 复杂属性 DSL (Transform, GridAreas等)
├── types.rs            // 类型系统入口，定义 ValidFor trait 并整合响应式绑定
├── theme.rs            // 主题上下文集成、增量补丁 (Partial Patching) 与变量同步逻辑
├── layers.rs           // 级联层 (@layer) 的层序——优先级的唯一约定处
├── escape.rs           // 属性名与值写入 CSS 文本前的净化
├── runtime/
│   ├── backend.rs      // 「对样式表做什么」与「样式表是什么」的分界（SheetBackend）
│   ├── platform.rs     // 与宿主的另两处接缝：诊断输出与微任务调度
│   ├── registry.rs     // 全局样式表注册表 (Static & Document Registry)
│   ├── sheet.rs        // 浏览器后端：构造式样式表 + <style> 兜底（仅 wasm）
│   ├── fake.rs         // 非 wasm 后端：状态机测试用的观察窗
│   ├── template.rs     // 动态规则的结构化模板（编译期切片，运行时拼接）
│   └── dynamic.rs      // 动态样式状态管理与弱引用 GC
└── codegen.rs          // 自动生成的代码产物入口 (codegen/ 子模块)
```

### 核心组件关系
1.  **`Style` (Builder)**：用户接口，收集 `StaticRule` 和 `DynamicRule`。
2.  **`DocumentStyleRegistry`**：单一事实来源，管理整个 `document` 的 `adoptedStyleSheets` 列表。
3.  **`StaticStyleRegistry`**：负责将所有组件共用的静态 CSS 规则合并到一个共享的 StyleSheet 中。
4.  **`DynamicStyleManager`**：负责管理那些无法用简单变量解决的复杂动态规则（如随状态变化的伪类），通过引用计数和 LRU 确保不发生泄露。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 强类型验证机制 (`ValidFor<P>`)
在 `src/types.rs` 中，我们为每一个 CSS 属性定义了一个 ZST 结构体（如 `props::Width`）。

**值类型能力集来自真实的语法解析。**
每个属性允许哪些 Rust 值类型，由 `silex_codegen::css::syntax` 解析 MDN 的
「值定义语法」得出，回答的是一个具体问题：**哪些值可以单独构成这个属性的完整
取值**。产物是 `properties.rs` 里每项的第四栏，例如：

```text
(width,   "width",   Width,  [LenCalc Length Percent])
(color,   "color",   Color,  [Color])
(z_index, "z-index", ZIndex, [Int])
(margin,  "margin",  Margin, [LenCalc Length Percent Str])
```

关键字（`auto`、`center` …）不在这一栏里：它们按「**关键字集合**」去重后共用同一个
枚举，由 `keywords_gen.rs` 单独生成。`width` / `height` / `min-width` / … 八个属性
的关键字集合完全相同，于是共用 `BlockSizeKeyword`（其余七个是它的类型别名）；
`align-items` 的 `center` 走 `AlignItemsKeyword`。集合恰好只有一个 `auto` 或一个
`none` 的属性直接复用全局的 `Auto` / `NoneValue`，不再各生成一个枚举。

能力位与 Rust 类型的对应关系（`define_props!` 的 `@cap` 分支）：

| 能力 | 允许的类型 |
| --- | --- |
| `Length` | `Px` / `Rem` / `Em` / `Ch` / `Ex` / `Vw` / `Vh` / `Vmin` / `Vmax` / `Dvw` / `Dvh` / `Svw` / `Svh` / `Lvw` / `Lvh` / `Pt` / `Pc` / `Cm` / `Mm` / `In` / `Qmm` |
| `Percent` | `Percent` |
| `LenCalc` | `CalcValue<LengthMark>` |
| `Angle` | `Deg` / `Rad` / `Turn` / `CalcValue<AngleMark>` |
| `Time` | `Sec` / `Ms` / `CalcValue<TimeMark>` |
| `Flex` | `Fr`（只在网格轨道尺寸里合法，不与长度互通） |
| `Color` | `Rgba` / `Hex` / `Hsl` / `ColorFn` / `ColorKeyword` |
| `Url` | `Url` |
| `Num` / `Int` | `f64` / `f32` / 各整数类型 |
| `Str` | `String` / `&'static str`——取值可能由多个分量拼成，或含 Rust 侧没有对应类型的东西 |

此外，**每个**属性都无条件接受 `CssWide`（`inherit` / `initial` / `unset` /
`revert` / `revert-layer`，规范规定它们对任何属性都合法）、`CssUnsafe`、
`CssVar<()>`，以及该属性关键字集合对应的枚举。

**验证转发与 `CssVar<T>`**：
为了让主题变量具备原始类型的校验能力，我们通过泛型转发实现了约束继承：
```rust
impl<T, P> ValidFor<P> for CssVar<T> where T: ValidFor<P> {}
```
这保证了 `AppTheme::PRIMARY`（类型为 `CssVar<Hex>`）只能在接收 `Color` 相关的
属性方法（如 `color()`、`background_color()`）中使用。

> 这条保证**依赖字段类型是 `Hex` 而不是 `String`**。`theme!` 从 `silex.toml`
> 派生字段时曾把类型一律硬编码成 `String`，生成 `CssVar<String>`——而 `String`
> 不是 `ValidFor<props::Color>`，于是配置驱动的主题颜色恰恰不能用在 `color()`
> 上。现在配置里可以声明字段类型，颜色默认为 `Hex`。

**错误信息**：`ValidFor` 的所有具体值实现都带 `#[diagnostic::do_not_recommend]`。
不加它，一条 `align_items(hex(…))` 会让 rustc 附上一张「`Hex` 还实现了
`ValidFor<BackgroundColor>` / `ValidFor<Border>` / …」的清单——40 多行，讲的全是
**别的**属性；trybuild 快照曾因此长到 455 行、其中 433 行是这张清单。真正有用的
「`AlignItems` 能收什么」由关键字枚举那一侧给出，它没有被抑制。

### 4.2 宏驱动的主题系统 (`theme!`)
主题系统不仅是运行时的变量同步，更是编译期的强约束：
1. **常量生成**：宏通过 `pub const NAME: CssVar<T>` 为每个字段生成常量。
2. **零开销引用**：这些常量内部使用 `CssVarValue::Static(&'static str)`，在 `sty()` 中直接作为字符串片段嵌入 CSS 哈希和样式表中。
3. **Patch 系统**：自动生成 `ThemePatch` 结构体，用于局部覆盖。**配置驱动的主题
   同样有 Patch 字段**——`theme! { struct T {} }` + `silex.toml` 配色曾生成一个零
   字段的 `TPatch`（补字段的是 `fields`，生成 Patch 用的却是 `def.fields`），于是
   `theme_patch()` 静默无效。
4. **字段类型**：在 `[theme.field_types]` 里指定；不指定时按取值猜——像颜色的值
   给 `Hex`，其余给 `String`。这一步决定了 §4.1 那条转发保证是否成立。

```toml
[theme]
prefix = "app"

[theme.colors]
primary = "#6366f1"
radius  = "8px"

[theme.field_types]
radius = "Px"      # 不写的话 `radius` 会被猜成 String
```

### 4.3 单位、颜色与空值

**单位。** 每个单位都是一个独立的 newtype，清单集中在
`units::for_all_length_units!` / `for_all_angle_units!` / `for_all_time_units!`
三个宏里——`CssLength`、算术运算符、`calc()` 操作数、`ValidFor` 展开、响应式登记
全都由它们驱动，加一个新单位只要改一处。

| 量纲 | 单位 |
| --- | --- |
| 长度 | `px` `rem` `em` `ch` `ex` `vw` `vh` `vmin` `vmax` `dvw` `dvh` `svw` `svh` `lvw` `lvh` `pt` `pc` `cm` `mm` `inch` `qmm` |
| 百分比 | `pct` |
| 角度 | `deg` `rad` `turn` |
| 时间 | `sec` `ms` |
| 网格 | `fr` |

`in` 是 Rust 关键字，所以英寸的工厂函数叫 `inch`；`Q` 与 HTML 的 `<q>` 标签在
prelude 里同名，所以叫 `qmm`。

**颜色。** `rgb()` / `rgba()` / `hex()` / `hsl()` / `hsla()`，加上 CSS Color 4 的
`oklch` / `oklab` / `lch` / `lab` / `hwb`（各有一个带 alpha 的 `*a` 变体）与
`color_mix` / `color_mix_weighted`。现代颜色函数的分量可以是数值、百分比或
`NONE`（规范的 `none`，表示该通道缺省）。

`Rgba` 与 `Hsl` 用**同一条**规则决定输出形式：alpha 为 1 时写 `rgb()` / `hsl()`，
否则写 `rgba()` / `hsla()`。此前 `Rgba` 恒定输出 `rgba(…)` 而 `Hsl` 按 alpha 切换，
同一份语义在两个类型上写出两种文本，静态哈希的稳定性判断也跟着分叉。

**空值。** `CssOption::None` 渲染成 CSS 宽关键字 `unset`。它此前渲染成**空串**，
而两条路的行为既不一致也都不是注释说的「不输出」：静态路径产出 `prop: ;`
（无效声明，浏览器丢弃），动态路径产出 `prop: var(--x)` 且 `--x` 为空串，触发
*invalid at computed-value time*。真正的「不输出」在动态路径上做不到——声明写在类
规则里，能改的只有变量的值——所以两边统一到 `unset`。

### 4.4 运算符重载与计算表达式 (`calc.rs`)
为了提供原生的 CSS 计算体验，`silex_css` 针对标量类型（如 `Px`, `Rem`）重载了算术运算符：
- **运算符重载**：`px(100) + rem(2)` 生成一个 `CalcValue<LengthMark>`，渲染为
  `calc(100px + 2rem)`。算术结果**自带 `calc()` 外壳**——`width: (100px + 2rem)`
  不是合法 CSS；嵌套算式补括号而不是套第二层 `calc()`。
- **计算函数**：支持 `calc()`, `min()`, `max()`, `clamp()`。
    - `clamp(px(100), pct(50), px(500))` → `clamp(100px, 50%, 500px)`。三个参数
      各有各的类型参数，只要同量纲即可。
    - `min()` / `max()` 有**两个形态**，分工明确：
        - 函数版收迭代器，元素必须同型——`min(vec_of_px)` 这种参数本来就来自
          运行时集合的场合用它；
        - 宏版 `css_min!` / `css_max!` 收变长参数，**每个参数各自过一次
          `IntoCalc`**，所以类型可以不同：`css_min!(px(10), pct(50))` →
          `min(10px, 50%)`。参数在编译期就写死时用它。
        - `css_clamp!` 与函数版 `clamp` 完全等价，只为让三个数学函数写法一致。
        - 三个宏都在 prelude 里。命名带 `css_` 前缀是为了不和 `std::cmp::min`
          撞心智，与 `css_unsafe` / `css_some` / `css_none` 一致。
- **数学安全**：`LengthMark` / `AngleMark` / `TimeMark` 三个量纲标记挡住跨量纲运算
  （`px(1) + sec(1)` 编译失败，`css_min!(px(1), sec(1))` 同样失败）。`<length>` 与 `<length-percentage>` 是两个不同的
  trait：`translateZ()` 与 `perspective()` 只收前者，而算术运算符收后者
  （`calc(100% - 10px)` 本来就合法）。

### 4.5 递归构建器逻辑 (`builder.rs`)
`Style` 构建器在执行 `apply_to_element` 时采用递归处理模式：
1.  **递归哈希**：递归遍历所有静态规则、嵌套选择器和媒体查询。这意味着即使是深层嵌套的样式变化，只要属性结构稳定，生成的类名就保持稳定。
2.  **CSS 生成与变量展平**：
    *   **选择器处理**：`nest()` 按 **CSS Nesting** 语义展开——含 `&` 则替换，不含
        则补一个**后代**关系（`.nest(":hover")` → `.cls :hover`）。想把伪类贴在
        自身上用 `pseudo()` 或 `on_hover()` / `on_active()` / `on_focus()` /
        `on_focus_visible()` / `on_disabled()`（`.cls:hover`）。
        这两条此前挤在同一个方法里且与宏相反：builder 无 `&` 时直接拼接，而
        `css!` 里裸写 `:hover { … }` 走 CSS Nesting——**同一个字符串，builder 当
        伪类、宏当后代选择器**，匹配的是完全不同的元素集。
    *   **媒体查询**：自动包裹生成的 CSS 块。
    *   **变量分配**：所有的动态值（信号）在生成的类定义中被展平为全局唯一的变量索引（`--sb-<hash>-<n>`）。
3.  **原子更新 Effect**：为每个动态值启动一个极轻量的 `Effect`，该 Effect **不触碰** CSSOM 树，只调用 `style.set_property` 修改当前元素的变量值。这种“树状定义，扁平更新”的设计实现了表达力与性能的平衡。
4.  **逃生舱**：注册表覆盖不到的东西走 `var()` 与 `raw()`。
    *   `.var("--brand", hex("#09f"))` 写自定义属性（不带 `--` 会自动补上）。整个
        主题系统都建立在 CSS 变量之上，而 `sty()` 此前根本写不出 `--my-var: red`。
    *   `.raw("-webkit-font-smoothing", "antialiased")` 写一条不经类型系统的声明。
        MDN 的 `properties.json` 里根本没有 `-webkit-font-smoothing` /
        `-moz-osx-font-smoothing` / `-webkit-backdrop-filter` 这类属性，注册表也就
        生成不出方法；能进白名单的厂商前缀属性（`-webkit-appearance`、
        `-webkit-mask-*`、`-webkit-tap-highlight-color` 等）仍有强类型方法。
    *   两条路的**属性名与值都会过净化**（`escape::property_name` /
        `escape::declaration_value`），写不出越界的声明；语义正确与否由调用方负责。

### 4.6 文档注册表同步 (`runtime/registry.rs`)
为了避免高频插入样式表导致的布局抖动（Layout Thrashing），`DocumentStyleRegistry` 采用了微任务同步机制：
```rust
fn sync(&mut self) {
    if self.is_pending { return; }
    self.is_pending = true;
    wasm_bindgen_futures::spawn_local(async {
        // 在微任务中合并所有变更，一次性调用 set_adopted_style_sheets
        with_document_registry(|dr| dr.perform_sync());
    });
}
```
微任务里再比一次表的清单：只有当**样式表的 JS 对象标识**这一批真的变了，才调用
`set_adopted_style_sheets`。同一微任务内增删抵消（组件卸载又立刻挂载同一份样式）
时，这一步能整个跳过。

> 这里一定要按 JS 对象标识比，而不是 Rust 侧那个 `CssStyleSheet` 值的内存地址。
> 后者是 `Vec` 元素的地址：扩容会让它整批变化（明明没换表却重同步），而增删数量
> 相等时新元素又可能落回同一批槽位（明明换了表却判定没变，于是**新样式永不生效、
> 被移除的永不摘除**）。

**静态表是增量写入的。** `StaticStyleRegistry` 只把新增的 chunk 切成顶层规则、
`insertRule` 到表尾，而不是每次 flush 都把所有 chunk 重新拼一遍再 `replaceSync`
——后者会让浏览器整表重新解析，组件在不同 tick 陆续挂载时总成本是 O(n²)。
只有 `<style>` 兜底或某条 `insertRule` 抛错时才退回整表重建。

**失败不再静默。** 借用冲突时注入与摘除都会排进延迟队列、在下一个微任务补做；
建表失败会退到 `<style>`；debug 构建下这些情况都会打到 `console.error`。

### 4.7 级联层 (`layers.rs`)
优先级从低到高共四层，声明为 `@layer base, components, utilities, overrides;`
（静态样式表的第一条规则）：

| 层 | 谁写进来 | 用途 |
| --- | --- | --- |
| `base` | `global!` | 全局重置、元素默认样式 |
| `components` | `styled!` / `declare_variants!` | 组件自身的样式 |
| `utilities` | `css!` / `tw!` | 工具类，按设计就该压过组件默认值 |
| `overrides` | `sty()` | 针对单个元素实例的就地覆盖，优先级最高 |

从组件里提升出来的 `@font-face` / `@keyframes` 不套任何层——它们不属于那个组件。
`set_global_theme` 注入的 `:root{}` 也不套层（无层规则优先级最高），这样主题变量
总能压住 `base` 里的默认值。

### 4.8 浏览器基线
编译产物的降级目标默认对齐运行时真正需要的能力：

| 依赖 | 最低版本 |
| --- | --- |
| `adoptedStyleSheets` + `new CSSStyleSheet()`（主注入路径） | Chrome 73 / Safari 16.4 / Firefox 101 |
| `@layer` | Chrome 99 / Safari 15.4 / Firefox 97 |
| `color-mix()`（`CssVar::alpha`） | Chrome 111 / Safari 16.2 / Firefox 113 |

取上界即 **Chrome 111 / Safari 16.4 / Firefox 113**。可以在 `silex.toml` 里改：

```toml
[css.targets]
chrome = "111"
safari = "16.4"
firefox = "113"
```

可用的键：`android`、`chrome`、`edge`、`firefox`、`ie`、`ios_saf`、`opera`、
`safari`、`samsung`。写错浏览器名或版本号会直接编译报错，不会静默退回默认值。

### 4.9 动态样式 GC 策略 (`runtime/dynamic.rs`)
`DynamicStyleState` 实现了 `Drop`：
- 一个样式不再被任何组件引用时立即**退休**：从 `document.adoptedStyleSheets` 摘出去，
  但保留已解析好的样式表对象。退休表不再参与样式匹配，复用时也不必重新解析 CSS。
- 退休队列是 LRU，上限 32；超出后最老的那个才真正 `Drop`。
- `DYNAMIC_STYLE_REGISTRY` 内部维护 `Weak<DynamicStyleState>`，确保如果同一个组件或
  相同样式的组件重新挂载，可以立即复用现有的 StyleSheet 对象。

### 4.10 动态规则的结构化模板 (`runtime/template.rs`)
带运行时片段的规则（`.x $theme { … }`、`& $sel { color: $c; }`）在**编译期**就被切成
`CssPart::{Lit, Class, Val}` 的序列，运行时只做拼接。

这样做是因为事后文本替换会误伤：`res.replace(基类名, 动态类名)` 会把
`.foo-bar` 一起改成 `.foo-dyn-h-bar`；按顺序逐个 `String::replace` 会让前一个替换
写进去的**值内容**被后一个 pattern 二次命中；`{}` 占位符则与 CSS 里真实出现的 `{}` 冲突。

唯一还靠文本替换的是全局样式里的 `var(--slx-dyn-N)`：那段模板要先过一遍
lightningcss，位置信息在那之后不复存在。但替换是一遍扫描完成的，写进去的值不会
再被当成占位符。

### 4.11 静态取值的校验

`css!` / `styled!` 里**不含插值**的静态声明会过五道判据，全部在宏展开期完成，
按下表的先后顺序：

| # | 判据 | 拦住的写法 | 实现 |
| --- | --- | --- | --- |
| 1 | 属性名在注册表里 | `colr: red` | `css::table::resolve_property_type` |
| 2 | 分量个数属性吃得下 | `color: 1px solid red`、`z-index: 1 2` | `css::value_check`（A-3） |
| 3 | 裸关键字在该属性的关键字表里 | `align-items: centre`、`color: reed` | 同上（A-1） |
| 4 | 函数产出的能力属性收得下 | `align-items: rgb(0 0 0)` | 同上（A-2） |
| 5 | 取值定得了型 | `color: 10px`、`width: #fff` | `compiler::classify_static_value` + `ValidFor` |

第 2 道必须排在第 5 道之前：`width: 1 0px` 的分量个数是错的，但
`classify_static_value` 会先把空白折掉、再把它认成一个合法的 `10px`。

第 2～4 道的判据表由 `silex_codegen` 从 MDN 值定义语法生成，与 `silex_css` 的
`for_all_properties!` / `keywords_gen.rs` 同源：

*   `silex_macros/src/css/property_keywords.rs` —— 每个属性可以**单独**取的关键字，
    外加全局具名颜色表与 CSS 全局关键字。
*   `silex_macros/src/css/property_caps.rs` —— 每个属性的能力位掩码，另带
    `MULTI`（可由多个顶层分量拼成）与 `OPEN`（语法里有裸标识符/字符串，能取什么
    无法穷举）两位。

**不误报优先**：`OPEN` 的属性（`animation-name`、`font-family`、`grid-area` …）、
关键字表为空的属性、以 `-` 开头的厂商关键字、不认识的函数一律放行。

逃生口三层：

1.  `unsafe { … }` 块 —— 单条声明原样透传，不改配置。
2.  `silex.toml` 逐层降级：

    ```toml
    [css.validation]
    keywords  = "error"   # error | warn | off，默认 error
    functions = "error"
    arity     = "error"
    ```

    `warn` 走 `CssWarning` 通道，展开成 `#[deprecated]` 触发的警告，不中断编译。
3.  `Style::raw(name, value)` —— 完全绕开注册表。

`@apply` 展开出来的声明是机器生成的（含 `--tw-*` 与厂商前缀），不走这套判据。

### 4.12 运行时的三处宿主接缝 (`runtime/backend.rs`、`runtime/platform.rs`)

运行时被切成三层，只有最下面一层认识浏览器：

| 层 | 内容 | 认识 web-sys 吗 |
| --- | --- | --- |
| 1 纯计算 | `builder.rs::render`、`types/*`、`escape.rs`、`layers.rs`、`runtime/template.rs` | 否 |
| 2 状态机 | `runtime/registry.rs`、`runtime/dynamic.rs`：谁进文档、何时进、退休与复用 | 否 |
| 3 后端 | `runtime/sheet.rs`（wasm）、`runtime/fake.rs`（其它） | 是 |

接缝有三处，都是 type alias + 静态分发，**没有 `dyn`、没有运行时开销**：

*   `SheetBackend`——建表、整表替换、追加顶层规则、拿 `adoptedStyleSheets` 句柄、摘除。
    句柄的 `PartialEq` 必须是**对象标识**：`DocumentStyleRegistry` 靠它判断这一批
    表和上一批是不是同一批，此前拿 Rust 侧内存地址当身份，`Vec` 一扩容身份就全变，
    反过来同一微任务内增删数量相等时新元素又可能落回原槽位，于是「没变化」被误判。
*   `DocumentBackend`——`document.adoptedStyleSheets = [...]` 那一次写入。
*   `platform::schedule_microtask`——「借不到注册表 → 排队 → 下一个微任务补做」里的
    那个微任务，wasm 下是 `spawn_local`。

分层不是为了多态（运行时只有一个实现在用），而是为了**第 2 层能脱离浏览器被断言**：
退休 LRU、延迟队列、增删时序这些最容易改坏的判断，没有一条需要浏览器在场。
`runtime/tests.rs` 里的 15 个用例覆盖的就是这一层，`cargo test` 一把跑完，不需要
headless 浏览器。「我们对 CSSOM API 的理解对不对」不由它们保证——那是另一回事。

同一个接缝也是 SSR 的地基：第 1、2 层已经与平台无关，服务端形态缺的是第 3 层的一个
新后端（以及框架其余部分的服务端形态，见 §5）。

### 4.13 借不到注册表时的补做

`DOCUMENT_REGISTRY` / `STATIC_REGISTRY` / `DYNAMIC_STYLE_REGISTRY` 都是
`RefCell`，而注入过程中触发注入、`Drop` 里回头动注册表都会撞上重入借用。
这类冲突一律**排队 + 约一个微任务补做**，不再「借不到就算了」：

| 场景 | 队列 | 此前的后果 |
| --- | --- | --- |
| 静态样式注入 | `DEFERRED_INJECTIONS` | 那段 CSS 静默消失 |
| 文档增删（挂载/摘除/注册静态表） | `PENDING_OPS`（增删共用一个队列以保序） | 摘不掉的表永久留在文档上；挂不上的表要等退休复用才重试 |
| 动态表注销 | `PENDING_UNREGISTER` | 注册表里留下悬空条目 |

`DynamicStyleState` 的 `attached` 记的是**意图**而不是结果：排进队列的增删一定会发生，
所以先记状态再排队。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **已知限制**：
    *   首次注入大型复杂样式树时，在 Rust 端构建 CSS 字符串会有一定的毫秒级开销。
    *   还不支持 SSR。运行时的状态机（§4.12 的第 1、2 层）已经与平台无关，非 wasm
        目标上有一个空转后端；但 `builder.rs` 的 `apply_to_element` 与 `theme.rs`
        仍直接收 `web_sys` 类型，且 `silex_core` / `silex_dom` 都只有 wasm 形态——
        **SSR 是框架级立项，不是 CSS 模块的 TODO**，缺的是服务端后端 + 那两个 crate
        的服务端形态。
    *   静态取值的校验以 MDN 的值定义语法为判据（见 §4.11），**MDN 数据滞后的
        属性会漏报**：关键字表为空、或者语法里有 `<custom-ident>` 的属性一律放行。
*   **性能瓶颈**：当页面存在数千个不同的动态 `Style` 对象时，虽然 DOM 压力小，但 Rust 端的 `Effect` 闭包管理会有一定的内存开销。
*   **TODO**：
    1.  [ ] 实现样式的跨组件去重（目前仅在单组件多次渲染间去重）。
    2.  [ ] 打通 SSR：给 `runtime/backend.rs` 加一个累积 CSS 文本的服务端后端，并等
        `silex_core` / `silex_dom` 有服务端形态。
    3.  [ ] 补一组 `wasm-bindgen-test` 冒烟用例，验证「我们对 CSSOM API 的理解」——
        `runtime/tests.rs` 覆盖的是状态机，覆盖不到这一层。


