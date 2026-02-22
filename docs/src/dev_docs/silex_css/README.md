# Silex CSS 核心实现分析

`silex_css` 是 Silex 框架中负责样式处理的核心 crate。它不仅提供了一套强类型的 CSS 构建系统，还包含了一个高性能的 CSSOM 运行时，旨在解决传统 Web 框架在处理样式时面临的性能瓶颈、类型安全缺失以及 DOM 压力问题。

## 1. 概要 (Overview)

*   **定义**：一个集成了强类型验证、零宏构建器 (Zero-macro Builder) 以及现代 CSSOM 运行时的 CSS 管理引擎。
*   **作用**：在 Silex 架构中，`silex_css` 为 `silex_macros` 提供的 `styled!` 宏以及用户直接使用的 `sty()` 构建器提供底层支持。它负责将 Rust 代码中的样式描述转换为高效的浏览器指令（如 `adoptedStyleSheets` 和 `setProperty`）。
*   **目标受众**：本文档主要面向希望了解 Silex 样式系统底层优化机制的贡献者。阅读前建议了解现代浏览器的 `Constructable StyleSheets` API 以及 Rust 的响应式系统基础。

## 2. 理念和思路 (Philosophy and Design)

*   **设计背景**：传统的样式更新方案（如修改 `className` 或内联 `style` 字符串）会触发大规模的重计算（Recalculate Style）和解析压力。同时，动态样式的生命周期管理一直是前端框架的难点，容易导致内存泄漏。
*   **核心思想**：
    *   **零 DOM 压力**：彻底放弃 `<style>` 标签，完全基于 `adoptedStyleSheets` 实现样式注入。
    *   **极简更新路径**：对于动态样式，优先使用 CSS 变量（CSS Variables）进行占位，更新时仅触发轻量级的 `element.style.setProperty`。
    *   **编译时安全**：利用 Rust 的 ZST (Zero-Sized Types) 和 Trait 系统，在编译期拦截非法的属性赋值（如将 `Color` 传给 `Width`）。
    *   **自动化生命周期**：结合弱引用（Weak References）和 LRU 缓存，实现样式的自动注入与销毁。
*   **方案取舍 (Trade-offs)**：
    *   **为什么不使用内联样式？** 内联样式无法处理伪类（`:hover`）、伪元素（`::before`）和媒体查询。
    *   **为什么不使用 CSS-in-JS 常见的 Hash 方案？** 传统的 Hash 方案在属性变更时需要生成新的类名并注入新的样式表，开销巨大。Silex 通过“静态结构 Hash + 动态变量注入”的组合方案，兼顾了功能完整性和性能。

## 3. 模块内结构 (Internal Structure)

```text
src/
├── builder.rs          // 零宏 Style 构建器系统
├── types.rs            // 强类型属性 (props)、单位 (units) 及 ValidFor 验证 trait
├── theme.rs            // 主题上下文集成与变量同步逻辑
├── runtime/
│   ├── registry.rs     // 全局样式表注册表 (Static & Document Registry)
│   └── dynamic.rs      // 动态样式状态管理与弱引用 GC
└── properties.rs       // 自动生成的属性宏定义 (由编译工具产出)
```

### 核心组件关系
1.  **`Style` (Builder)**：用户接口，收集 `StaticRule` 和 `DynamicRule`。
2.  **`DocumentStyleRegistry`**：单一事实来源，管理整个 `document` 的 `adoptedStyleSheets` 列表。
3.  **`StaticStyleRegistry`**：负责将所有组件共用的静态 CSS 规则合并到一个共享的 StyleSheet 中。
4.  **`DynamicStyleManager`**：负责管理那些无法用简单变量解决的复杂动态规则（如随状态变化的伪类），通过引用计数和 LRU 确保不发生泄露。

## 4. 代码详细分析 (Detailed Analysis)

### 4.1 强类型验证机制 (`ValidFor<P>`)
在 `src/types.rs` 中，我们为每一个 CSS 属性定义了一个 ZST 结构体（如 `props::Width`）。
```rust
pub trait ValidFor<Prop> {}

impl ValidFor<props::Width> for Px {}
impl ValidFor<props::Width> for Percent {}
// 编译期会拦截：Style::new().width(rgba(0,0,0,1))
```
这种设计利用了 Rust 的类型系统实现了“非法状态不可表示”。

### 4.2 零宏构建器逻辑 (`builder.rs`)
`Style` 构建器在执行 `apply_to_element` 时会经历以下步骤：
1.  **稳定哈希**：对所有的属性名和静态值进行哈希，生成唯一的 `class_base`。
2.  **CSS 生成与变量占位**：
    *   静态值直接写入 CSS。
    *   动态值（信号）被替换为 `--sb-<hash>-<index>` 格式的 CSS 变量名。
3.  **原子更新 Effect**：为每个动态值启动一个极轻量的 `Effect`，该 Effect **不触碰** CSSOM 树，只调用 `style.set_property` 修改当前元素的变量值。

### 4.3 文档注册表同步 (`runtime/registry.rs`)
为了避免高频插入样式表导致的布局抖动（Layout Thrashing），`DocumentStyleRegistry` 采用了微任务同步机制：
```rust
fn sync(&mut self) {
    if self.is_pending { return; }
    self.is_pending = true;
    wasm_bindgen_futures::spawn_local(async {
        // 在微任务中合并所有变更，一次性调用 set_adopted_style_sheets
        DOCUMENT_REGISTRY.with(|dr| dr.borrow_mut().perform_sync());
    });
}
```
通过比较 `last_sync_ids`（记录样式表指针地址），我们能跳过 99% 的冗余同步调用。

### 4.4 动态样式 GC 策略 (`runtime/dynamic.rs`)
`DynamicStyleState` 实现了 `Drop`：
- 当一个样式不再被任何组件引用，且超出 `RETIRED_STYLES` 的 LRU 限制（当前为 128）时，它会从全局 `DocumentStyleRegistry` 中自动移除。
- `DYNAMIC_STYLE_REGISTRY` 内部维护 `Weak<DynamicStyleState>`，确保如果同一个组件或相同样式的组件重新挂载，可以立即复用现有的 StyleSheet 对象，避免重复解析。

## 5. 存在的问题和 TODO (Issues and TODOs)

*   **已知限制**：目前 `Style` 构建器在哈希时未考虑伪类内部的层级细化，复杂的嵌套伪类可能会导致哈希冲突（概率极低但理论存在）。
*   **性能瓶颈**：当页面存在数千个不同的动态 `Style` 对象时，虽然 DOM 压力小，但 Rust 端的 `Effect` 闭包管理会有一定的内存开销。
*   **TODO**：
    1.  [ ] 实现样式的跨组件去重（目前仅在单组件多次渲染间去重）。
    2.  [ ] 支持更复杂的 CSS 简写属性（Shorthand Properties）的智能解析。
    3.  [ ] 进一步优化 `split_rules` 的性能。

