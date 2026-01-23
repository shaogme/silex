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
*   **信号 (Signals)**: `ReadSignal<T>` 会创建一个响应式的文本节点，当信号更新时仅更新该文本内容。
*   **闭包 (Closures)**: `move || { ... }` 形式的闭包被视为动态视图。Silex 会自动建立副作用 (`create_effect`)，并在数据变化时智能比对并更新 DOM 范围。
*   **集合**: `Vec<V>`, Slice `[V]`, 元组 `(A, B)` 都会按顺序渲染其内容。

### 3. Attributes (属性)
Silex 提供了一套统一且强大的属性设置 API。

#### 静态设置
```rust
div.attr("id", "app")
   .class("container text-center")
   .style("color: red;")
```

#### 响应式设置
任何接受属性值的地方，都可以传入一个闭包！Silex 会自动将其转化为副作用。

```rust
let (count, set_count) = create_signal(0);

// class 会随着 count 的奇偶性自动切换
div.class(move || if count.get() % 2 == 0 { "even" } else { "odd" })
```

#### 专用的 Property API
HTML Attribute 和 DOM Property 是不同的。例如 `input.value` 是 Property，而 `input.getAttribute('value')` 是初始值 Attribute。

使用 `.prop()` 方法直接操作 DOM 对象属性：

```rust
input.prop("value", signal) // 绑定实时值
     .prop("checked", true)
```

## 事件处理

使用 `.on_event` 系列方法绑定事件。

```rust
button.on_click(|e| {
    console_log(&format!("Clicked: {:?}", e));
})
```

双向绑定：

```rust
input.bind_value(rw_signal)
```

## 扩展：自定义 View
你可以为自己的结构体实现 `View` trait，使其可以直接嵌入到组件树中。

```rust
struct MyComponent;

impl View for MyComponent {
    fn mount(self, parent: &web_sys::Node) {
        let el = document().create_element("div").unwrap();
        el.set_text_content(Some("Custom Component"));
        parent.append_child(&el).unwrap();
    }
}
```
