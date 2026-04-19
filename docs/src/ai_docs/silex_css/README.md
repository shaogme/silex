# Crate: silex_css

`silex_css` 是 Silex 框架的 CSS 核心基础设施，提供类型安全的样式构建 (`Style` Builder)、高性能现代浏览器运行时 (Adopted StyleSheets)、以及响应式主题系统。

## 1. 核心架构映射 (Core Architecture)

| 模块/文件 | 职责说明 |
| :--- | :--- |
| `src/builder.rs` | 提供递归式 `Style` 构建器，支持媒体查询 (`@media`)、复杂嵌套及其生成的递归哈希提取。 |
| `src/runtime/registry.rs` | **DocumentStyleRegistry**: 样式单一事实来源，管理 `adoptedStyleSheets`。 |
| `src/runtime/registry.rs` | **StaticStyleRegistry**: 全局静态样式合并与原子的 `insert_rule` 注入。 |
| `src/runtime/dynamic.rs` | **DynamicStyleManager**: 实例级样式管理，支持基于弱引用的 GC 与 LRU 缓存。 |
| `src/theme.rs` | 提供主题变量注入、全局主题管理及亚毫秒级增量更新。 |
| `src/types/` | 模块化的类型系统，包含单位 (`units.rs`)、计算 (`calc.rs`)、复合属性 (`shorthands.rs`)、渐变 (`gradients.rs`) 及**复杂 DSL (`complex.rs`)**。 |
| `src/types.rs` | 类型系统入口，定义核心验证 Trait `ValidFor` 并整合属性注册。**内联集成了 IntoSignal 响应式绑定**。 |
| `src/properties.rs` | 自动生成的 CSS 全量属性定义与方法宏映射。 |

## 2. 核心验证系统 (Type-Safe Validation)
`src/types.rs`

### 2.1 `ValidFor<Prop>` Trait
Silex CSS 的类型安全基石。通过为不同属性定义专有的零大小类型 (ZST)，并在编译期检查 `ValidFor` 实现。
- **属性命名空间**: 位于 `crate::types::props` 下，如 `props::Width`, `props::Color`。
- **验证组 (Groups)**:
    - **Dimension**: 支持 `Px`, `Percent`, `Rem`, `Em`, `Vw`, `Vh`。
    - **Color**: 支持 `Rgba`, `Hex`, `Hsl`。
    - **Number**: 支持各类型数字 (`i32`, `f64` 等)。
    - **Calculation**: 支持 `CalcValue` 及其相关的 `calc()`, `min()`, `max()`, `clamp()` 函数。
    - **Keyword**: 自动生成的枚举类 (如 `TextAlignKeyword`) 会自动实现对应属性的 `ValidFor`。
    - **Complex**: 专为 `transform`, `grid-template-areas` 等属性设计的强类型 DSL (参见 `complex.rs`)。

### 2.2 复合属性工厂 (Shorthand Factories)
为避免手写易错的字符串，提供强类型工厂函数：
- `border(width, style, color)` -> `BorderValue`
- `margin::all(v)`, `margin::x_y(x, y)` -> `MarginValue`
- `padding::all(v)` -> `PaddingValue`
- `flex(grow, shrink, basis)` -> `FlexValue`
- `transition(prop, duration, timing, delay)` -> `TransitionValue`
- **Transform**: `transform().translate(x, y).rotate(a).build()` -> `TransformValue`
- **Grid Areas**: `grid_template_areas(["header header", "main side"])` -> `GridTemplateAreasValue`
- **Font Variations**: `font_variation_settings([("wght", 700), ("ital", 1)])` -> `FontVariationSettingsValue`

## 3. 现代 CSSOM 运行时 (Adopted StyleSheets)
`silex_css` 彻底抛弃了传统的 `<style>` 标签操作，完全基于现代浏览器的 **Constructable StyleSheets**。

### 3.1 批量同步机制 (Microtask Batching)
通过 `DocumentStyleRegistry::sync()` 实现。
- **逻辑**: 当任意样式变更（添加/移除）时，并不立即操作 DOM。
- **批处理**: 利用 `wasm_bindgen_futures::spawn_local` 将同步推迟到微任务队列。
- **去重更新**: 在同一帧（Event Loop 循环）中，无论发生多少次 `sync`，实际的 `document.set_adopted_style_sheets` 仅执行一次。

### 3.2 弱引用生命周期管理 (GC)
位于 `DynamicStyleManager`。内部持有 `RETIRED_STYLES` (LRU 缓存，容量 100)。
- **回收机制**: 使用 `Rc` 与 `Weak` 指针。当样式表不再被任何活跃组件引用且被强制挤出 LRU 缓存时，触发物理卸载。

## 4. 样式构建器 (Style Builder)
`builder.rs` -> `struct Style`

### 4.1 静态与动态提取逻辑 (Recursive Style)
`Style::apply_to_element` 执行流程：
1.  **递归哈希生成**: 使用 `CssHasher` 对 *属性名* + *静态值* + *选择器* + *媒体查询* 的树状组合计算唯一指纹。**动态值不参与哈希**。
2.  **变量注入**: 无论嵌套深度如何，动态信号都被分配扁平化的 CSS 变量（如 `--sb-{hash}-{index}`），写入样式表的类定义中。
3.  **递归 CSS 构建**: 遍历 `Style` 树，处理 `&` 占位符展开并包裹 `@media` 块。
4.  **原子更新**: 渲染时产生极轻量 `Effect`，直接调用 `CSSStyleDeclaration.setProperty` 更新变量值。

## 5. 关键 API 详述

### 5.1 核心 Structs

| 实体 | 路径 | 说明 |
| :--- | :--- | :--- |
| `Style` | `builder.rs` | 递归链式构建器。支持 `width()`, `on_hover()`, `nest()`, `media()` 等。 |
| `Px(Option<f64>)` | `types.rs` | 标准 CSS 单位包装。所有单位现已统一使用 `Option` 包装内容。 |
| `UnsafeCss(Option<String>)` | `types.rs` | 逃逸舱，用于绕过类型检查。 |
| `DynamicCss` | `runtime/dynamic.rs` | 用于存储带有响应式绑定的 CSS 类信息。 |

### 5.2 核心 Functions

| 实体 | 路径 | 说明 |
| :--- | :--- | :--- |
| `sty() -> Style` | `builder.rs` | `Style::new()` 的快捷入口。 |
| `inject_style(id, css)` | `runtime/registry.rs` | 增量注入静态 CSS 规则。 |
| `theme_variables(theme)` | `theme.rs` | 零开销主题变量注入。支持 `IntoSignal` 实现在不产生多余 DOM 下的变量同步。 |
| `theme_patch(patch)` | `theme.rs` | **[NEW]** 局部主题补丁。支持增量更新，利用 CSS 变量继承实现精准局部覆盖。 |
| `set_global_theme(theme)` | `theme.rs` | 全局 `:root` 主题挂载（含 Context 注入）。 |
| `clamp(min, val, max)` | `types/calc.rs` | CSS `clamp()` 函数的强类型实现。 |
| `calc(value)` | `types/calc.rs` | 将表达式包裹在 `calc()` 中的快捷函数。 |
| `linear_gradient()` | `types/gradients.rs` | 渐变构建器入口。 |

## 6. 性能规格 (Performance Specs)
- **更新延迟**: 变量级更新 < 10μs。
- **内存占用**: 静态样式全局单例化；动态样式基于 LRU 自动回收。
- **线程安全**: `!Send / !Sync` (绑定 WASM 线程)。
- **不变量**: 必须在 `wasm32` 环境且支持 `Constructable StyleSheets` 的浏览器中运行。
