# Silex CSS 核心文档

`silex_css` 是框架中隔离的 CSS 核心包，专责处理 CSS 的运行时注入、强类型验证和无宏 Builder 构建系统。它通过独立出来，大幅提高了宏系统和主体库变更时的重新编译速度。

## 1. 强类型 CSS 运行时 (Type-Safe CSS Runtime)
位于 `silex_css/src/lib.rs` 、 `silex_css/src/registry.rs` 与 `silex_css/src/types.rs`。
*   **架构设计**：它不单纯是传统意义上的 CSS Runtime 工具链，而是与 `silex_macros` 协同构筑的前后端一体化防线。抛弃单纯接受一切 `Display` 给字符串 `+` 的行为。
*   **自动化注册表 (registry.rs)**: 基于 `for_all_properties!` 自动化生产出了标准 CSS 属性名同 Rust `struct` ZST 标签的枚举绑定映射，为工具链构筑底层完备属性字典。
*   **Property Tags (属性感知)**：基于上述注册表，系统生成了内建 Trait Bounds Tag 结构体（诸如 `props::Width`，`props::Color` 等）。验证流伴随 `DynamicCss`，使得 `ValidFor<P>` 这个 Trait 得以在运行时构建前提前在编译期就成功实施阻断语法错误。
*   **封锁隐式逃逸与 `UnsafeCss`**：彻底废除对 `&str`、`String` 及 `Any` 属性的泛用 `ValidFor` 实现，转而要求开发者在需要越过类型检查时显式声明使用 `UnsafeCss::new(...)`。
*   **复合复合与工厂函数 (Factory Functions)**：对于 `border` 这类需要多种类型排版的复杂属性，采用 Rust `const fn border(width, style, color)` 工厂函数及其对应类型 `BorderValue` 处理。

## 2. 现代 CSSOM 引擎 (Adopted StyleSheets & Weak GC)
*   **混合挂载与 `DynamicCss`**: Silex 将样式打散。静态规则通过全局 `StaticStyleRegistry` 合并注入。直接属性能使用 CSS Var 替换的则抽取为 `vars`，在同个组件内聚合并使用单个 `Effect` 配合 `style.set_property` 原子性更新；如果涉及无法以 CSS Var 处理的伪类或内嵌插值结构（`rules`），运行时将其分发给一个个独立的 `Effect`，它们会伴随信号变化动态重哈希类名，由 `DynamicStyleManager` 托管。
*   **基于弱引用的自动化 GC (Weak Reference Lifecycle)**: 彻底废弃了手动 `ref_count` 维护。
    *   **零 DOM 模式**：所有样式通过 `document.adoptedStyleSheets` 挂载，不再在 HTML 中产生任何 `<style>` 节点。
    *   **自动化清理**：通过 Rust 的 `Rc<DynamicStyleState>` 维持活跃引用。一旦组件卸载且该样式不在 LRU 缓存池（`RETIRED_STYLES`）中，`Drop` 协议会自动触发。
    *   **原子更新逻辑**：`DocumentStyleRegistry` 作为单一事实来源，持有样式表镜像，并在微任务中合并 `sync` 请求。这确保了无论组件树如何复杂，对 `adoptedStyleSheets` 的赋值操作始终是确定且高性能的。该设计不仅终结了节点泄漏，还大幅降低了浏览器重新计算样式的总开销。

## 3. 类型安全构建器 (Type-Safe Builder: Style)
位于 `silex_css/src/builder.rs`。
*   **设计动机**：为了满足对极致编译性能（零宏路径）和 100% Rust 原生补全极致追求的场景。
*   **核心逻辑**：
    *   **链式 API**：提供一系列强类型方法如 `.width(px(100))`。它不仅仅是字符串拼接，而是利用 `IntoSignal` 和 `ValidFor` trait 在编译期拦截类型不匹配的属性赋值。
    *   **智能分配**：
        *   当属性是**常量**时，合并进 `static_rules`。多处使用相同 `Style` 的静态部分会被哈希成同名 Class，共享样式注入到 `<head>`。
        *   当属性是**信号/闭包**时，进入 `dynamic_rules`。在 DOM 挂载时通过响应式 Effect 绑定 **CSS 变量**。
    *   **高频更新优化**：这是 Silex 的核心优化点。对于动态属性（如 `width: $(w)`），系统会为对应的 class 生成一个唯一的 CSS 变量占位（例如 `--sb-hash-0`）。当信号更新时，Effect 只执行轻量的 `element.style.setProperty('--sb-hash-0', val)`。这避免了修改内联 style 字符串导致的浏览器样式重计算压力，且能与静态 Class 完美配合。
    *   **伪类响应式支持**：通过 `on_hover(|s| ...)` 定义的样式。如果其中包含动态部分，Style 引擎会为该元素分配唯一的类名，并由 `DynamicStyleManager` 管理对应的 CSSOM 对象。该样式表会自动同步到全局 `Adopted StyleSheets` 列表中，无需在 DOM 中创建任何物理节点，解决了内联样式（inline-style）无法覆盖伪类的局限。

## 4. 主题上下文注入 (Theme System)
位于 `silex_css/src/theme.rs`。
*   **去包裹设计**: 为了保证类似 Flex/Grid 的嵌套层级不受不必要的父容器 DOM 打扰，Silex 没有提供一个 `ThemeProvider` 标签组件，而是采用 `Apply` Trait 机制：`div(..).apply(theme_variables(theme))` 进行挂载。或者通过 `set_global_theme(theme)` 将变量挂靠在 `<style>` 内为 `:root` 共享。
*   **索引化更新 (Performance Indexing)**: 最新的主题系统彻底废弃了 `to_css_variables()` 产生的长字符串方案。现在通过 `ThemeToCss` trait 预导出的 `get_variable_names()` 和 `get_variable_values()`，在 `Effect` 中通过索引对比每一个分量，仅对变动的变量调用 `style.set_property`。

## 5. 架构与哈希改进
*   **哈希解耦**: 为了支持更复杂的哈希策略（如未来可能的缩写混淆），`silex_hash` 引入了 `css` 专用模块。所有 CSS 相关的哈希逻辑（如 `Normalized` 包装器）均迁入此处。
*   **缓存对比策略**: 在 `DynamicCss` 和 `Style` Builder 内部，通过 `Option<Vec<String>>` 在 `Effect` 中保存上一轮的计算结果。这确保了即便信号频繁触发，只要最终生成的 CSS 属性值未变，就不会触发昂贵的字符串哈希和 `<style>` 内容更新。
