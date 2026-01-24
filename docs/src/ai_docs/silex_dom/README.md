# Crate: `silex_dom`

**Type-safe, fine-grained DOM manipulation and attribute management.**

此 Crate 提供了对 `web_sys` DOM API 的轻量级封装，利用 `silex_core` 的响应式系统实现细粒度更新。它是 Silex 框架的渲染引擎核心。

## 模块: `element` (DOM 元素)

源码路径: `silex_dom/src/element.rs`

### `Element`
*   **Struct**: `pub struct Element { pub dom_element: web_sys::Element }`
*   **Semantics**: 基础的 DOM 元素包装器，无类型标签约束。
*   **Methods**:
    *   `new(tag: &str) -> Self`: 创建 HTML 元素 (`document.createElement`).
    *   `new_svg(tag: &str) -> Self`: 创建 SVG 元素 (`document.createElementNS`).
    *   `as_web_element(&self) -> web_sys::Element`: 获取底层 raw element。
    *   **Builder Methods**: `attr`, `id`, `class`, `style`, `prop`, `on`, `on_click`, `on_input`, `node_ref`.

### `NodeRef<T>` (已移至 `silex_core`)

> **注意**: `NodeRef` 已从 `silex_dom` 移动到 `silex_core::node_ref`。此处保留重导出以保持向后兼容。
>
> 详见 [`silex_core` 文档](../silex_core/README.md)。

*   **Struct**: `pub struct NodeRef<T = ()> { id: NodeId, marker: PhantomData<T> }`
*   **Traits**: **`Copy`**, `Clone`, `Debug`, `Default`.
*   **Semantics**: 使用 `NodeId` 句柄的轻量级 DOM 引用，数据存储在响应式运行时。
*   **Usage**: 传递给 `Element::node_ref(ref)`。无需 `.clone()`，直接复制即可。

### `TypedElement<T>`
*   **Struct**: `pub struct TypedElement<T> { pub element: Element, _marker: PhantomData<T> }`
*   **Semantics**: 带有 Phantom 类型标记的元素包装器，用于实现特定的属性 Trait (如 `FormAttributes` 仅适用于 `Input` 标签)。
*   **Traits**: `Deref<Target=Element>`, `Into<Element>`.

### Common Builder API
所有元素都支持以下链式方法：

#### Attributes & Props
*   `attr(self, name: &str, value: impl ApplyToDom)`: 设置/更新属性。
*   `prop(self, name: &str, value: impl ApplyToDom)`: 设置/更新 JS 属性 (`JsValue`).
*   `id(self, value: impl ApplyToDom)`
*   `class(self, value: impl ApplyToDom)`: 添加 class (支持多类名字符串).
*   `classes(self, value: impl ApplyToDom)`: 同 `class`.
*   `style(self, value: impl ApplyToDom)`: 设置内联样式.
*   `node_ref<N>(self, node_ref: NodeRef<N>)`: 绑定 DOM 引用。`N` 必须实现 `JsCast`。**`NodeRef` 是 `Copy` 的，无需 clone**。

> **注意**: 布尔属性（如 `required`, `checked`）需要显式传递值。
> *   静态: `.required(true)`
> *   动态: `.required(signal)`

#### Events
*   `on<E, F, M>(self, event: E, callback: F)`: **强类型事件监听** (推荐)。
    *   `E`: 实现 `EventDescriptor` (例如 `silex_dom::event::click`).
    *   `F`: `EventHandler` (接受 `E::EventType`).
*   `on_untyped<E, F>(self, event_type: &str, callback: F)`: **字符串健名事件监听**。
    *   `E`: 实现 `FromWasmAbi` (例如 `web_sys::Event`), 通常需要显式指定 (Turbofish).
    *   `event_type`: 事件名称字符串.
*   `on_click<F, M>(self, callback: F)`: 绑定点击事件 (基于 `on(event::click, ...)`).
*   `on_input<F, M>(self, callback: F)`: 绑定输入事件。
*   `bind_value(self, signal: RwSignal<String>)`: 双向绑定 `value` 属性。

---

## 模块: `attribute` (属性系统)

源码路径: `silex_dom/src/attribute.rs`

### `ApplyTarget`
*   **Enum**: 指定值应用的目标位置。
    *   `Attr(&'a str)`: `setAttribute`.
    *   `Prop(&'a str)`: `js_sys::Reflect::set`.
    *   `Class`: `classList.add/remove`.
    *   `Style`: `style.setProperty`.

### `ApplyToDom` Trait
*   **Definition**: `pub trait ApplyToDom { fn apply(self, el: &WebElem, target: ApplyTarget); }`
*   **Implementors**:
    *   **Static**: `&str`, `String`, `bool` (Boolean Attribute toggle), `Option<T>`.
    *   **Reactive**: `impl Fn() -> T` (自动创建 `Effect` 进行细粒度更新).
    *   **Signals**: `Signal<T>`, `ReadSignal<T>`, `RwSignal<T>`, `Memo<T>`.
    *   **Collections**: `Vec<V>`, `[V; N]`.
    *   **Tuples**:
        *   `(Key, Value)`: 用于 Style (e.g., `("color", "red")`).
        *   `(Key, bool)`: 用于 Conditional Class (e.g., `("active", true)`).
        *   `(Key, Signal<bool>)`: 响应式 Conditional Class.

### `AttributeGroup`
*   **Macro**: `group!(...)`
*   **Semantics**: 允许将多个属性打包成一个元组，批量应用。
*   **Impl**: `impl ApplyToDom for AttributeGroup<(T1, T2, ...)>`.

---

## 模块: `view` (渲染系统)

源码路径: `silex_dom/src/view.rs`

### `View` Trait
*   **Definition**: `pub trait View { fn mount(self, parent: &web_sys::Node); }`
*   **Semantics**: 定义对象如何挂载到 DOM 树中。
*   **Implementors**:
    *   **DOM**: `Element`, `TypedElement`.
    *   **Text**: `&str`, `String`, Primitives (`i32`, `bool`, etc.).
    *   **Reactive**: `Fn() -> V` (细粒度更新，使用 Comment 节点作为锚点进行 Range Cleaning).
    *   **Signals**: `ReadSignal<T>`, `RwSignal<T>` (文本节点更新).
    *   **Collections**: `Vec<V>`, `[V; N]`, `Option<V>`, `Result<V, E>`.
    *   **Fragments**: `(A, B, C...)` 元组。

### `AnyView` (Type Erasure)
*   **Struct**: `pub struct AnyView(Box<dyn Render>);`
*   **Semantics**: 动态分发的 View 容器，用于异构列表或条件渲染分支。
*   **Usage**: `match ... { A => a.into_any(), B => b.into_any() }`.

---

## 模块: `props` (属性特征)

源码路径: `silex_dom/src/attribute/props.rs`

利用 Rust 的 Trait 系统实现 HTML 属性的类型约束。

### Traits
*   `GlobalAttributes`: `id`, `class`, `style`, `hidden`, etc. (Applicable to `Element`).
*   `FormAttributes`: `type_`, `value`, `checked`, `disabled`, `placeholder`. (For `Input`, `Button`, etc.).
*   `LabelAttributes`: `for_`.
*   `AnchorAttributes`: `href`, `target`.
*   `MediaAttributes`: `src`, `alt`, `width`, `height`.
*   `AriaAttributes`: `role`, `aria-*`.

---

## 模块: `tags` (标签标记)

源码路径: `silex_dom/src/element/tags.rs`

*   **Traits**: Empty marker traits usually used to bound `TypedElement<T>`.
    *   `Tag`: Base trait.
    *   `FormTag`, `LabelTag`, `AnchorTag`, `MediaTag`, `TextTag`, `SvgTag`.

---

## 模块: `event` (事件描述符)

源码路径: `silex_dom/src/event.rs`

### `EventDescriptor` Trait
*   **Definition**:
    ```rust
    pub trait EventDescriptor: Copy + Clone + 'static {
        type EventType: FromWasmAbi + JsCast + 'static;
        fn name(&self) -> Cow<'static, str>;
        fn bubbles(&self) -> bool { true }
    }
    ```
*   **Semantics**: 定义 DOM 事件的元数据，将事件名称 (String) 与 `web_sys` 事件类型 (Type) 关联起来。

---

## 模块: `ev` (预定义事件)

源码路径: `silex_dom/src/event/types.rs`

此模块包含了一系列实现了 `EventDescriptor` 的空结构体，用于类型安全的事件绑定。使用宏 `generate_events!` 生成。

### Supported Events
*   **Mouse**: `click`, `dblclick`, `mousedown`, `mouseup`, `mousemove`, `mouseover`, `mouseout`, `mouseenter`, `mouseleave`, `contextmenu` (`web_sys::MouseEvent`)
*   **Keyboard**: `keydown`, `keypress`, `keyup` (`web_sys::KeyboardEvent`)
*   **Form**:
    *   `change`, `reset`, `invalid` (`web_sys::Event`)
    *   `input` (`web_sys::InputEvent`)
    *   `submit` (`web_sys::SubmitEvent`)
*   **Focus**: `focus`, `blur`, `focusin`, `focusout` (`web_sys::FocusEvent`)
*   **UI**: `scroll`, `load`, `unload`, `select` (`web_sys::Event`); `resize`, `abort` (`web_sys::UiEvent`); `error` (`web_sys::ErrorEvent`)
*   **Pointer**: `pointerdown`, `pointermove`, `pointerup`, `pointercancel`, `pointerenter`, `pointerleave`, `pointerover`, `pointerout`, `gotpointercapture`, `lostpointercapture` (`web_sys::PointerEvent`)
*   **Drag**: `drag`, `dragend`, `dragenter`, `dragexit`, `dragleave`, `dragover`, `dragstart`, `drop` (`web_sys::DragEvent`)
*   **Touch**: `touchstart`, `touchend`, `touchmove`, `touchcancel` (`web_sys::TouchEvent`)
*   **Wheel**: `wheel` (`web_sys::WheelEvent`)
*   **Animation**: `animationstart`, `animationend`, `animationiteration` (`web_sys::AnimationEvent`); `transitionend` (`web_sys::TransitionEvent`)
*   **Composition**: `compositionstart`, `compositionupdate`, `compositionend` (`web_sys::CompositionEvent`)

---

## 模块: `helpers` (工具函数)

源码路径: `silex_dom/src/helpers.rs`

提供了一系列用于 DOM 操作、事件处理和定时器的辅助函数。**注意：本模块假定运行在纯 CSR（客户端渲染）且单线程的 WASM 环境中。**

### Window & Document
*   `window() -> Window`: 获取线程局部缓存的 `window` 对象。
*   `document() -> Document`: 获取线程局部缓存的 `document` 对象。
*   `location()`, `location_hash()`, `location_pathname()`: 简化的 URL/Location 获取。

### Property Helpers
*   `set_property(el, prop_name, value)`: 使用 `js_sys::Reflect::set` 设置属性。
*   `get_property(el, prop_name)`: 使用 `js_sys::Reflect::get` 获取属性。

### Event Helpers
*   `event_target<T>(event)`: 泛型获取事件目标。
*   `event_target_value(event)`:以此获取 Input/Textarea/Select 的值。
*   `event_target_checked(event)`: 获取 Checkbox 的选中状态。
*   `window_event_listener(event_descriptor, cb) -> Handle`: **强类型**监听 Window 事件。
*   `window_event_listener_untyped(name, cb) -> Handle`: 字符串类型监听 Window 事件。

### Timers & Scheduler
所有定时器函数均提供返回 `Handle` 的版本和直接调用的版本，并会自动处理清理逻辑（如果使用了 `on_cleanup`）。

*   `request_animation_frame(cb)` / `request_animation_frame_with_handle(cb)`
*   `request_idle_callback(cb)` / `request_idle_callback_with_handle(cb)`
*   `set_timeout(cb, duration)` / `set_timeout_with_handle(cb, duration)`
*   `set_interval(cb, duration)` / `set_interval_with_handle(cb, duration)`
*   `queue_microtask(cb)`

### Utilities
*   `debounce(duration, cb)`: 防抖函数，自动绑定到当前组件的生命周期（组件卸载时自动清理 Timer）。
