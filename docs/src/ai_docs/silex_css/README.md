# Crate: silex_css

`silex_css` 是 Silex 框架抽离的 CSS 核心验证逻辑、类型定义及运行时 Style Builder Crate。

## 1. 基础注入与动态管理 (CSS Injection & Management)
*   **inject_style(id, content)**: 
    *   检查 `<head>` 中是否存在 `id`。
    *   若不存在，创建 `<style id="...">` 并注入 CSS 内容。
    *   **Idempotent**: 多次调用无副作用。
*   **DynamicStyleManager**: 
    *   负责管理带生命周期的动态 `<style>` 标签。
    *   基于 `thread_local!` 使用 `DYNAMIC_STYLE_REGISTRY`（引用计数 `ref_count`）和 `RETIRED_STYLES` (LRU 队列，Limit=128)。确保组件挂载/卸载时，精确回收或复用 CSSOM 资源，防止长期运行 SPA 导致的 `<style>` 泄漏。
*   **DynamicCss (运行时响应态)**: `css!` 宏的实际产物。
    1.  **静态类名**: 提前在编译期就确立的 `.slx-[hash]`。
    2.  **局部变量 (vars)**: 多条利用 CSS 变量（如 `color: var(--...);`）构建的属性。由单一 `Effect` 聚合并在变化时仅调用 DOM `style.set_ property`，不造成布局抖动。
    3.  **局部规则 (rules)**: 对于带有插值的伪类或子选择器（如 `&:hover { width: $(w); }`)，在独立的 `Effect` 中评估插值并在执行时求取哈希，由 `DynamicStyleManager` 推送形如 `.slx-1234abcd-dyn-e5f6` 的新类，并自动变更元素的 `classList`。

## 2. 强类型系统 (silex_css::types & registry)
*   **全量属性生成**: `silex_css/src/registry.rs` 中通过 `for_all_properties!` 宏定义了全部标准 CSS 属性及对应安全类型标识。这为整个运行时的类型边界验证提供了标准化字典支持。
*   **类型安全**: 提供基于包裹原语（如 `px`, `pct`）的强类型约束机制组合。结合底层泛型方法如 `make_dynamic_val_for<P, S>` 在编译运行时阶段实施 `ValidFor` Trait 的安全性校准。
*   **复合类型工厂**: 利用如 `border()` 返回专属 `BorderValue`、或 `margin::all()` 创建多维值，剥离宏对于杂糅属性拆解的负担。
*   **显式逃逸 (`UnsafeCss`)**: 废弃泛用 `&str` 的随意通行，通过 `UnsafeCss::new("calc(...)")` 显式标识未知或高级组合边界。

## 3. Type-Safe Builder API (Style Builder)
`silex_css/src/builder.rs` -> `struct Style`

为追求零宏开销 (Zero Macro Overhead) 和极致类型安全设计的纯 Rust API。

*   **API 形态**: 链式调用，通过 `Style::new()` 或 `sty()` 初始化。
*   **属性实现**: 通过 `implement_css_properties!` 宏（声明宏而非过程宏，无解析开销）生成强类型 Setter，自动关联 `ValidFor<T>` 约束。
*   **静态与动态提取 (Static & Dynamic Extraction)**:
    1.  **静态规则**: 收集所有常量值，计算 `DefaultHasher` 指纹，生成 `.slx-bldr-{hash}` 类名并调用 `inject_style` 提升至 `<head>`。
    2.  **动态规则 (CSS 变量优化)**: 收集闭包/信号值，在渲染时为属性分配 CSS 变量名（如 `--sb-{hash}-{index}`）。在 `Effect` 中通过 `style.set_property` 原子化更新该变量。该方案保证了在高频场景下（如拖拽、动画）最低的渲染负载。
    3.  **伪类处理**: 支持 `on_hover`, `on_active`, `on_focus`。若伪类中包含动态插值，会自动利用 `DynamicStyleManager` 创建实例级类名并由运行时更新该类的 CSS 内容（以在内联样式无法表达伪类的情况下实现响应式）。
*   **DOM 集成**: 实现了 `ApplyToDom` trait，可直接在 `html::div().style(Style::new())` 中使用。

## 4. 主题系统 (Theme System)
`silex_css/src/theme.rs`
*   **ThemeVariables**: 零开销插入机制。不再引入额外包裹 DOM，而是通过扩展方法 `div(...).apply(theme_variables(theme_signal))` 直接监听信号变化并将主题变量转换后注入 `element.style`。
*   **全局模式**: `set_global_theme(theme_signal)` 可将主题挂载到 `:root` 上。
*   **Context**: 内部自动注入或查询 `use_theme<T>()` 获取。
