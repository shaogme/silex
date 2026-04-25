# Silex DOM

`silex_dom` 是框架的渲染层核心。它不仅仅是一个简单的 DOM 包装器，而是深度集成了 `silex_core` 的响应式系统，实现了**细粒度更新 (Fine-Grained DOM Updates)**。

## 核心概念

### 1. Element (元素)
所有的 DOM 节点在 Rust 中被表示为 `Element` 或强类型的 `TypedElement<T>`。它们是对 `web_sys::Element` 的轻量级包装。

```rust
let div = Element::new("div");
```

### 2. View (视图)
`View` 是一个 Trait，定义了如何将内容挂载渲染到屏幕上。
Silex 的视图系统非常灵活，支持多种类型直接作为视图：

*   **基本类型**: 数字、字符串、布尔值直接渲染为文本节点。
*   **信号 (Signals)**: `ReadSignal<T>`, `Signal<T>`, `Memo<T>` 等响应式类型。它们会创建一个响应式的文本节点，当信号更新时仅更新该文本内容。
*   **计算闭包**: 
    - **`rx!(...)` (推荐)**: 统一的响应式入口，将表达式转化为响应式视图。
    - **`move || { ... }`**: 传统的闭包形式，同样被视为动态视图。
    Silex 会自动建立副作用 (`Effect`)，并在数据变化时通过双锚点 (Double-Anchor) 机制智能清理并更新 DOM。

*   **集合**: `Vec<V>`, Slice `[V]`, 元组 `(A, B)` 都会按顺序渲染其内容。

### 3. Attributes (属性)
Silex 提供了一套统一且强大的属性设置 API。所有设置属性的方法（`.attr()`, `.prop()`, `.class()`, `.style()`, `.apply()` 等）都支持泛型 `V: IntoStorable`。

**重要规则**：
- **静态值**：可以直接传入 `&str`, `String`, `bool` 等。
- **动态值**：**必须**使用 `rx!(...)` 宏包裹闭包。直接传入 `move || ...` 无法通过编译。

#### 静态设置
```rust
div.id("app")
   .class("container text-center")
   .style("color: red;")
```

#### 响应式设置
任何接受属性值的地方，都可以传入一个 `rx!` 包装的计算单元。

```rust
let (count, set_count) = Signal::pair(0);

// class 会随着 count 的奇偶性自动切换
div.class(rx!(if count.get() % 2 == 0 { "even" } else { "odd" }))

// 禁用状态根据信号自动切换
button().disabled(rx!(count.get() > 10))
```

#### 通用应用 (Apply)
如果需要应用一段通用的逻辑、主题变量或 Mixins，可以使用 `.apply()` 方法：

```rust
// rx!(|el| ...) 创建一个 RxEffect，允许直接操作元素
element.apply(rx!(|el: &web_sys::Element| {
    el.set_attribute("data-custom", "value").unwrap();
}))
```

### 4. Attribute Forwarding (属性透传)

Silex 支持多根节点组件（view chain），通常通过 view_chain! 宏实现。

当你在一个返回 view chain 的组件上设置属性（如 `.class("foo")`）时，Silex 采用**首个匹配 (First-Match)** 策略：

*   属性会被向下传递给容器的所有子节点。
*   **第一个**能够实际消费属性的真实 DOM 节点（`Element`）会应用这些属性并将该应用操作“取走”。
*   后续的节点将不再接收到该属性。

这确保了在组件外部设置的 `class` 或 `id` 能够符合直觉地应用到组件的“主”元素上。

## 事件处理

### 1. 强类型事件 (推荐)
使用 `.on()` 方法配合 `silex_dom::event` 模块。这能提供完美的类型推断。

```rust
use silex_dom::event;

button.on(event::click, |e| {
    // e 自动推断为 web_sys::MouseEvent
    log!("Clicked at: {}, {}", e.client_x(), e.client_y());
})
```

### 2. 快捷方法
对于常用事件，可以直接使用快捷方法：

```rust
button.on_click(|e| { ... })
      .on_input(|value| { ... }) // input 事件会自动提取 value 字符串
```

### 3. 双向绑定
```rust
let text = rw_signal("".to_string());
input().bind_value(text)
```

## 直接 DOM 访问 (`NodeRef`)

有时你必须访问底层的 HTML 元素（例如调用 `.focus()`）。

```rust
use web_sys::HtmlInputElement;

// 1. 创建引用 (NodeRef 实现了 Copy)
let input_ref = NodeRef::<HtmlInputElement>::new();

div![
    input()
        .node_ref(input_ref) // 2. 绑定引用
        .placeholder("Wait for focus..."),
        
    button("Focus").on_click(move |_| {
        // 3. 安全获取
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    })
]
```

## 实用工具 (Helpers)

`silex_dom::helpers` 模块提供了一系列常用的辅助函数：

*   **Window/Document**: `window()`, `document()` (线程局部缓存)。
*   **定时器 (Hooks)**: `use_interval(duration, cb)`, `use_timeout(duration, cb)`。它们会自动集成 `on_cleanup`，在组件卸载时自动销毁，避免内存泄漏。
*   **事件辅助**: `event_target_value(&event)`, `window_event_listener(event::resize, |e| ...)`。
*   **调度**: `debounce` (防抖), `request_animation_frame`。

```rust
// 每秒执行一次，组件卸载时自动停止
use_interval(Duration::from_secs(1), || {
    log!("Tick!");
});
```
