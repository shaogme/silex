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
*   **信号 (Signals)**: `Signal<T>` 会创建一个响应式的文本节点，当信号更新时仅更新该文本内容。
*   **闭包 (Closures)**: `move || { ... }` 形式的闭包被视为动态视图。Silex 会自动建立副作用 (`Effect`)，并在数据变化时智能比对并更新 DOM 范围。
*   **集合**: `Vec<V>`, Slice `[V]`, 元组 `(A, B)` 都会按顺序渲染其内容。

### 3. Attributes (属性)
Silex 提供了一套统一且强大的属性设置 API。

#### 静态设置
```rust
div.id("app")
   .class("container text-center")
   .style("color: red;")
```

#### 响应式设置
任何接受属性值的地方，都可以传入一个闭包！Silex 会自动将其转化为副作用。

```rust
let (count, set_count) = signal(0);

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

Silex 提供了多种方式来绑定事件。

### 1. 强类型事件 (推荐)
使用 `.on()` 方法配合 `silex_dom::event` 模块中预定义的事件类型。这能保证回调函数的参数类型被自动正确推断，且无需手动指定泛型。

```rust
use silex_dom::event;

button.on(event::click, |e| {
    // e 被自动推断为 web_sys::MouseEvent
    console_log(&format!("Clicked at: {}, {}", e.client_x(), e.client_y()));
})
```

### 2. 快捷方法
对于常用事件（如 `click`, `input`），可以直接使用快捷方法：

```rust
button.on_click(|e| { ... })
      .on_input(|e| { ... })
```

### 3. 弱类型/自定义事件 (Untyped)
如果你需要监听自定义事件，或者不想引入 `ev` 模块，可以使用 `.on_untyped`。需要手动指定事件类型泛型。

```rust
// 必须显式指定事件类型泛型，例如 <web_sys::CustomEvent, _>
div.on_untyped::<web_sys::CustomEvent, _>("my-custom-event", |e| {
    console_log(&format!("Custom detail: {:?}", e.detail()));
})
```

双向绑定：

```rust
input.bind_value(rw_signal)
```

## 直接 DOM 访问

虽然声明式 API 可以覆盖绝大多数需求，但有时你必须访问底层的 HTML 元素（例如调用 `.focus()`, `.scrollIntoView()`, `.showModal()`，或集成第三方非 Rust 库）。

Silex 提供了 `NodeRef` 类型来实现这一需求。

### 使用 `NodeRef`

1.  创建一个 `NodeRef`。
2.  通过 `.node_ref()` 方法将其绑定到元素。
3.  在组件挂载后（例如在 `on_click` 或 `Effect` 中），通过 `.get()` 获取原生 DOM 节点。

> **提示**: `NodeRef` 实现了 `Copy`，可以直接复制，无需调用 `.clone()`。

```rust
use web_sys::HtmlInputElement;

// 1. 创建引用 (强类型)
let input_ref = NodeRef::<HtmlInputElement>::new();

div![
    input()
        // 2. 绑定引用 (NodeRef 是 Copy 的，无需 clone)
        .node_ref(input_ref) 
        .placeholder("Wait for focus..."),
        
    button("Focus").on_click(move |_| {
        // 3. 安全获取并调用原生 API
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    })
]
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

## 类型化事件 (Typed Events)

`silex_dom::event` 模块定义了一系列实现了 `EventDescriptor` 的空结构体，用于提供类型安全的事件名称和类型映射。

例如：
*   `event::click`: 对应 `web_sys::MouseEvent`，名称 "click"。
*   `event::input`: 对应 `web_sys::InputEvent`，名称 "input"。
*   `event::keydown`: 对应 `web_sys::KeyboardEvent`，名称 "keydown"。

这些类型主要配合 `window_event_listener` 等强类型 API 使用。

## 实用工具 (Helpers)

`silex_dom::helpers` 模块提供了一系列常用的 DOM 操作辅助函数：

*   **Window/Document**: `window()`, `document()` (线程局部缓存，无需反复 unwrap)
*   **属性操作**:
    *   `set_property(el, "prop_name", &value)`: 直接设置 DOM 属性 (Property) 而不是 Attribute。
    *   `get_property(el, "prop_name")`: 获取 DOM 属性值。
*   **定时器**: `set_timeout`, `set_interval`, `request_animation_frame`, `request_idle_callback` (包含自动清理机制)
*   **事件辅助**:
    *   `event_target_value(&event)`: 便捷获取 input 值
    *   `event_target_checked(&event)`: 便捷获取 checkbox 状态
    *   `window_event_listener(event::resize, |e| ...)`: **强类型**全局事件监听，自动推导事件参数类型。
    *   `window_event_listener_untyped("resize", ...)`: 字符串类型的全局事件监听。
*   **其他**: `debounce` (防抖), `queue_microtask`
