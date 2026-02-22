# Crate: `silex_dom`

**High-performance, fine-grained DOM rendering engine.**

此 Crate 将 `silex_core` 的响应式系统与 `web_sys` DOM API 相结合，通过 **Rx 委托 (Rx Delegate)** 模式实现无 VDOM 的细粒度渲染。

---

## 模块: `view` (视图系统)

源码路径: `silex_dom/src/view.rs`

### `View` Trait
核心渲染接口，定义对象如何挂载到 DOM 树中。
*   `fn mount(self, parent: &Node)`: 挂载视图。
*   `fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>)`: 应用透传属性（默认为空操作，`Element` 重写此方法）。
*   `fn into_any(self) -> AnyView`: 类型擦除优化点。

### `AnyView` (Enum Optimization)
为了避免 `Box<dyn View>` 的分配开销，使用枚举分发：
*   **Variants**: `Empty`, `Text(String)`, `Element(Element)`, `List(Vec<AnyView>)`, `Boxed(Box<dyn Render>)`.
*   **Optimization**: 常见类型通过 `into_any` 直接转换为 Enum 变体，**零堆分配**。
*   **`Render` Trait**: 内部辅助特征，用于支持 Boxed 会话的移动语义。

### Implementors (视图实现者)
*   **Text**: `String`, `&str`, 基础数字类型, `bool`, `char` (渲染为文本节点)。
*   **Reactive (Dynamic)**:
    *   `Fn() -> V where V: View`: 动态视图。使用 **双锚点策略 (Double-Anchor)**：`<!--dyn-start-->` 和 `<!--dyn-end-->` 标记范围，更新时进行 **Range Cleaning**。
    *   `Rx<F, RxValue>`: 包装计算闭包的视图。兼容 `Move` 闭包和类型擦除闭包 `Rc<dyn Fn() -> V>`。
*   **Signals**: `ReadSignal<T>`, `RwSignal<T>`, `Signal<T>`, `Memo<T>` (其中 `T: Display`)。信号更新时仅更新特定的文本节点内容。
*   **Collections**: `Vec<V>`, `[V; N]`, `Option<V>`, `Result<V, E>`, 元组 `(A, B, ...)` (元组最大支持 12 个元素)。

---

## 模块: `attribute` (属性与绑定)

源码路径: `silex_dom/src/attribute.rs`, `src/attribute/apply.rs` 等。

### `AttributeBuilder` Trait
所有 DOM 构建器的基础，提供链式调用方法。
*   `build_attribute<V>(self, target: ApplyTarget, value: V)`: 核心钩子。
*   `build_event<E, F, M>(self, event: E, callback: F)`: 事件绑定钩子。
*   **Convenience API**: `attr`, `prop`, `on`, `apply`.
*   **Global Extensions**: `GlobalAttributes` (id, class, style...), `AriaAttributes` (role, aria-*).

### `ApplyToDom` & `ApplyTarget`
*   `ApplyTarget`: 指定应用位置（`Attr`, `Prop`, `Class`, `Style`, `Apply`）。
*   `ApplyToDom`: 定义值如何作用于 DOM。
    *   实现了 **Rx 委托**: 接受 `Rx<F, M>`。
    *   **重要**: 由于属性设置受 `IntoStorable` 约束，动态属性闭包**必须**使用 `rx!(...)` 宏包裹以转换为 `Rx` 类型。直接传入 `move || {}` 将导致编译错误。
    *   **`RxValue`**: (通过 `rx!(expr)`) 执行闭包并创建一个 `Effect` 追踪依赖更新。
    *   **`RxEffect`**: (通过 `rx!(|el| body)`) 执行 `FnOnce(&WebElem)` 进行一次性或持续性元素修改。
    *   **Pairs**: 支持 `(Key, Value)` 形式，用于条件类名 (`"active", signal`) 或特定样式 (`"color", "red"`)。
*   **Diffing Logic**: `class` 和 `style` 属性在动态更新时内置了基于 `HashSet` 的简单 Diff，仅更新变化的类名或样式项。

### `IntoStorable` Trait
生命周期抹除机制。
*   将 `&str` 转换为 `String`，将信号和闭包保持为 `'static` 容器。
*   这是 `AttributeBuilder` 允许用户在链式调用中直接传引用（如 `.id("foo")`）的关键。

### `PendingAttribute` (属性透传)
*   存储待处理的应用操作，用于从组件向子元素转发属性。
*   **一次性消费语义**: 内部利用 `Rc<RefCell<Option<V>>>` 的 **Take-out** 模式，确保属性只应用到第一个匹配的 `Element`。

---

## 模块: `element` (DOM 元素)

源码路径: `silex_dom/src/element.rs`

### `Element` & `TypedElement<T>`
*   `Element`: 基础包装器，直接持有 `web_sys::Element`。
*   `TypedElement<T>`: 带有 `PhantomData<T>` 的强类型包装器。
    *   `T` 实现自 `tags.rs` (如 `FormTag`, `MediaTag`)。
    *   这使得 Codegen 生成的扩展 Trait (如 `FormAttributes`) 能够进行类型约束。
    *   **Deref**: `TypedElement` 到 `Element` 的 Deref 允许直接调用全局属性。

### `mount_to_body<V: View>(view: V)`
挂载入口。在内部创建一个根响应式作用域 (`create_scope`)，确保应用内部的 `Effect` 和上下文正常工作。

---

## 模块: `event` (事件系统)

源码路径: `silex_dom/src/event.rs`

### `EventDescriptor`
元数据特征，将事件名称（字符串）与 `web_sys` 类型关联。
*   `EventType`: 绑定的事件参数类型（如 `web_sys::MouseEvent`）。

### `EventHandler<E, M>`
灵活的回调处理器，通过 `Marker` 区分参数模式：
*   **`WithEventArg`**: 对应 `FnMut(E)`。
*   **`WithoutEventArg`**: 对应 `FnMut()`。
*   支持直接传入 `Rx` 包装的闭包。

---

## 模块: `helpers` (工具集)

源码路径: `silex_dom/src/helpers.rs`

*   **Window/Document**: 提供线程局部缓存的 `window()` 和 `document()` 访问。
*   **Hooks**: `use_interval`, `use_timeout`, `debounce`。自动集成 `on_cleanup`，在组件卸载时自动取消。
*   **Property Access**: `set_property`, `get_property` (基于 JS Reflection)。
*   **Event Helpers**: 
    *   `event_target_value`: 获取 Input/Select 值。
    *   `window_event_listener`: 强类型全局监听。

---

## 编译时优化 (Codegen)
`silex_dom/src/attribute/typed.rs` 提供 `ApplyStringAttribute` 和 `ApplyBoolAttribute`。这些 Trait 允许 Codegen 工具为各个标签生成高效的、针对特定类型的属性设置代码，避免了运行时的动态字符串转换开销。
