# Crate: `silex_dom`

**高性能、细粒度的 DOM 渲染引擎。**

`silex_dom` 是 Silex 框架的核心渲染层，它将 `silex_core` 的响应式系统与 `web_sys` 原生 DOM API 直接结合。其设计的核心目标是 **零 VDOM (Virtual DOM)**，通过 **Rx 委托 (Rx Delegate)** 模式实现极其精准的局部更新。

---

## 1. 核心架构与设计哲学 (Architecture & Design Philosophy)

### 1.1 零 VDOM 细粒度更新
与 React 等传统框架不同，Silex 不维护内存中的虚拟树。视图的逻辑直接编译/映射为 DOM 操作：
*   **静态部分**：仅在 `mount` 时执行一次。
*   **动态部分**：自动包装在 `silex_core::Effect` 中，仅监听其依赖的信号，变化时直接修改对应的 DOM 节点。

### 1.2 渲染管线 (Rendering Pipeline)
1.  **构造**：通过链式 API (`AttributeBuilder`) 或宏构造视图树。
2.  **归一化**：利用 `IntoStorable` 将引用和不同类型的响应式对象转换为 `'static` 容器。
3.  **挂载**：调用 `View::mount`，创建物理 DOM 节点并建立响应式绑定。
4.  **透传**：属性通过 `PendingAttribute` 在组件间转发，通过 `consolidate_attributes` 进行合并压缩。

---

## 2. 视图系统 (View System)

源码路径: `silex_dom/src/view.rs`, `src/view/any.rs`, `src/view/reactive.rs`

### 2.1 `View` Trait
核心接口，定义一个底层实体如何进入 DOM 树及如何接收属性。
```rust
pub trait View {
    fn mount(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>);
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>);
    fn into_any(self) -> AnyView;
}
```

### 2.2 类型擦除 (Type Erasure) 优化
为了支持异构视图（如 `match` 不同分支返回不同类型）和组件 `Children`，项目提供了一个优化的枚举类型：

| 类型 | 特点 | 适用场景 |
| :--- | :--- | :--- |
| **`AnyView`** | 支持 `Clone`，克隆外壳共享底层视图句柄。内部对常见类型仍然使用枚举变体，避免额外分配。 | 普通视图擦除、可重复挂载的组件边界、`Children` 属性。 |

**优化点**：对于 `String`, `Element`, `List` 等常见类型，仍然保留为枚举变体，**零堆分配**。

### 2.3 响应式视图内核 (`ReactiveView`)
当视图为 `Rx<V>` 时，系统使用 **双锚点策略 (Double-Anchor Strategy)**：
*   **内部机制**：在 DOM 中插入 `<!--dyn-start-->` 和 `<!--dyn-end-->` 注释节点。
*   **清理逻辑**：更新前，系统会遍历两个锚点之间的所有节点并进行 `remove_child`。同时调用 `silex_core::reactivity::dispose` 销毁旧视图关联的所有响应式 Effect。
*   **高性能文本**：如果 `Rx` 包装的是 `Display` 类型，则省略锚点，直接更新单个 `TextNode` 的 `nodeValue`。

### 2.4 实现者列表
*   **文本**：`String`, `&str`, 基础数字, `bool`, `char`。
*   **信号**：`Signal`, `ReadSignal`, `RwSignal`, `Memo` (要求内容实现 `Display`)。
*   **集合**：`Vec<V>`, `[V; N]`, `Option<V>`, `Result<V, E>`。
*   **元组**：`(A, B, ...)` (最大支持 12 元)，按照顺序依次挂载。

---

## 3. 属性与绑定 (Attribute & Binding)

源码路径: `silex_dom/src/attribute/`, `src/attribute/apply/`

### 3.1 `AttributeBuilder`
所有 DOM 构建器的统一个接口，提供链式调用：
*   `attr(name, value)`: 设置属性。
*   `prop(name, value)`: 设置 JS 属性 (Property)。
*   `on(event, handler)`: 绑定事件。
*   `apply(op)`: 应用自定义指令或 `AttrOp`。

### 3.2 属性存储与生命周期 (`IntoStorable`)
由于链式调用通常发生在栈上，但响应式 Effect 可能存活很久，`IntoStorable` 特征负责将非 `'static` 生命周期“擦除”：
*   `&str` -> `String`
*   `&String` -> `String`
*   信号与闭包 -> 保持原样 (它们本身已是 `'static` 的智能指针)。

### 3.3 统一指令集: `AttrOp`
`AttrOp` 是为了减少 Wasm 体积和运行时 Effect 数量的核心优化：
```rust
pub enum AttrOp {
    Update { name: Cow<'static, str>, target: AttrTarget, data: AttrData },
    SetStaticClasses(Vec<Cow<'static, str>>),
    CombinedClasses { ... }, // 专项合并优化
    Sequence(Vec<AttrOp>),   // 指令平铺
    Custom(Rc<dyn Fn(&Element)>), // 逃逸舱
    ...
}
```
**合并策略 (`CombinedClasses` / `CombinedStyles`)**：
*   当一个元素有多个 class 绑定（如一个静态 class 列表 + 多个 `class_toggle` + 一个响应式字符串）时，系统会将它们**合并为一个单 Effect**。
*   **Diff 算法**：内置基于 `HashSet` 的 Token Diff。仅移除不再需要的类，添加新类，避免全量重写 `className` 导致的闪烁或样式重置。

### 3.4 属性透传与合并 (`PendingAttribute`)
`PendingAttribute` 用于存储尚未应用到具体 DOM 节点的属性。
*   **一次性消费模式**：属性会在其遇到的第一个 `Element` 或组件边界处被“消费”。
*   **Consolidation**：通过 `consolidate_attributes` 函数将大量的 `Update` 指令合并为高效的 `Combined` 指令。

---

## 4. 元素与事件 (Element & Event)

### 4.1 元素包装器
*   **`Element`**：持有 `web_sys::Element` 的基础结构。实现 `Deref<Target = web_sys::Element>`。
*   **`TypedElement<T>`**：基于 `PhantomData` 的强类型包装。通过 `T` 指定具体标签（如 `FormTag`），从而启用标签特有的扩展属性（如 `.action()`, `.method()`）。

### 4.2 事件系统优化
为了避免为每个闭包生成重复的 JS 互操作代码，`silex_dom` 实施了**单态化归并**策略：
*   **`bind_event_impl<E>`**：此内部函数仅对事件类型 `E`（如 `MouseEvent`）进行单态化。
*   **闭包处理**：具体的处理器闭包被擦除为 `Box<dyn FnMut(E)>`，从而显著减小 Wasm 二进制体积。
*   **自动清理**：所有事件监听器都会自动注册 `on_cleanup`，在绑定的响应式作用域销毁时自动调用 `removeEventListener`。

---

## 5. 关键内部机制 (Internal Mechanics)

### 5.1 递归属性组 (`AttributeGroup`)
为了支持无限个数的元组属性绑定（如 `( (attr1, val1), (attr2, val2) )`），引入了 HList (Heterogeneous List) 模式：
*   `AttrCons(Head, Tail)`
*   `AttrNil`
这种递归打平技术允许用户传入深层嵌套的属性组，而系统在运行时会将其打平为 `Vec<AttrOp>`，避免了宏生成的硬编码限制。

### 5.2 全局助手 (`helpers.rs`)
*   **JS Reflection**：`set_property` / `get_property` 提供对 JS 对象属性的低层访问。
*   **双向绑定**：`bind_value(signal)` 宏/方法自动处理 `on_input` 追踪和 `signal` 更新后的视图反向同步，并包含防止 Cursor 跳动的逻辑。

---

## 6. 使用示例 (Usage)

```rust
use silex_dom::prelude::*;

let count = create_rw_signal(0);

// 构建强类型元素
h::div()
    .id("app")
    .class("container")
    .class_toggle("is-active", count.map(|c| c > 0)) // 合并 Effect
    .on_click(move |_| count.update(|c| *c += 1))
    .children((
        h::h1().text("Counting..."),
        h::p().text(rx!(count)), // 细粒度 Text 节点更新
    ))
    .mount_to_body();
```
