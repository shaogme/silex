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
    *   **Builder Methods**: `attr`, `id`, `class`, `style`, `prop`, `on_click`, `on_input`.

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
*   `child<V: View>(self, view: V)`: 挂载子节点。

#### Events
*   `on_click<F, M>(self, callback: F)`: 绑定点击事件。
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
    *   **Reactive**: `impl Fn() -> T` (自动创建 `create_effect` 进行细粒度更新).
    *   **Signals**: `ReadSignal<T>`, `RwSignal<T>`.
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

源码路径: `silex_dom/src/props.rs`

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

源码路径: `silex_dom/src/tags.rs`

*   **Traits**: Empty marker traits usually used to bound `TypedElement<T>`.
    *   `Tag`: Base trait.
    *   `FormTag`, `LabelTag`, `AnchorTag`, `MediaTag`, `TextTag`, `SvgTag`.
