use crate::event::{EventDescriptor, EventHandler};

mod apply;
mod into_storable;
mod typed;

pub use apply::*;
pub use into_storable::*;
pub use typed::*;

pub trait AttributeBuilder: Sized {
    /// Core hook: Apply or store a generic attribute/property directly using ApplyTarget mechanism.
    /// Accepts any type that implements IntoStorable, allowing both static references (&str, &String)
    /// and owned/reactive types (String, Signal, closures).
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable;

    /// Core hook: Apply or store an event listener.
    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static;

    // === Unified Mixins (Default Implementation) ===

    fn attr<V>(self, name: &str, value: V) -> Self
    where
        V: IntoStorable,
    {
        self.build_attribute(ApplyTarget::Attr(name), value)
    }

    fn prop<V>(self, name: &str, value: V) -> Self
    where
        V: IntoStorable,
    {
        self.build_attribute(ApplyTarget::Prop(name), value)
    }

    fn on<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static,
    {
        self.build_event(event, callback)
    }

    /// Generic application of a value that knows how to apply itself to the DOM.
    /// Useful for mixins, theme variables, or complex reactive logic.
    fn apply<V>(self, value: V) -> Self
    where
        V: IntoStorable,
    {
        // Wrap in a storable type and build
        self.build_attribute(ApplyTarget::Apply, value)
    }
}

// --- 分层 Trait 定义 (from props.rs) ---

/// 全局属性：所有 HTML 元素都支持的属性
pub trait GlobalAttributes: AttributeBuilder {
    fn id(self, value: impl IntoStorable) -> Self {
        self.attr("id", value)
    }

    fn class(self, value: impl IntoStorable) -> Self {
        self.attr("class", value)
    }

    fn style(self, value: impl IntoStorable) -> Self {
        self.attr("style", value)
    }

    fn title(self, value: impl IntoStorable) -> Self {
        self.attr("title", value)
    }

    fn lang(self, value: impl IntoStorable) -> Self {
        self.attr("lang", value)
    }

    fn dir(self, value: impl IntoStorable) -> Self {
        self.attr("dir", value)
    }

    fn tabindex(self, value: impl IntoStorable) -> Self {
        self.attr("tabindex", value)
    }

    fn draggable(self, value: impl IntoStorable) -> Self {
        self.attr("draggable", value)
    }

    fn hidden(self, value: impl IntoStorable) -> Self {
        self.attr("hidden", value)
    }
}

// 自动为所有实现 AttributeBuilder 的类型实现 GlobalAttributes
impl<T: AttributeBuilder> GlobalAttributes for T {}

/// ARIA 无障碍属性：提供给所有元素使用
pub trait AriaAttributes: AttributeBuilder {
    fn role(self, value: impl IntoStorable) -> Self {
        self.attr("role", value)
    }

    fn aria_label(self, value: impl IntoStorable) -> Self {
        self.attr("aria-label", value)
    }

    fn aria_hidden(self, value: impl IntoStorable) -> Self {
        self.attr("aria-hidden", value)
    }
}

// 自动为所有实现 AttributeBuilder 的类型实现 AriaAttributes
impl<T: AttributeBuilder> AriaAttributes for T {}

/// 全局事件与通用组件方法：提供诸如 on_click, class_toggle, bind_value 等常用操作
pub trait GlobalEventAttributes: AttributeBuilder {
    fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): IntoStorable,
    {
        self.build_attribute(ApplyTarget::Class, (name.to_string(), condition))
    }

    fn classes<V>(self, value: V) -> Self
    where
        V: IntoStorable,
    {
        self.build_attribute(ApplyTarget::Class, value)
    }

    fn node_ref<N>(self, node_ref: silex_core::node_ref::NodeRef<N>) -> Self
    where
        N: wasm_bindgen::JsCast + Clone + 'static,
    {
        self.apply(silex_core::rx!(move |el: &web_sys::Element| {
            use wasm_bindgen::JsCast;
            if let Ok(typed) = el.clone().dyn_into::<N>() {
                node_ref.load(typed);
            } else {
                silex_core::log::console_error("NodeRef type mismatch: failed to cast element");
            }
        }))
    }

    // --- Event API ---

    fn on_click<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<web_sys::MouseEvent, M> + Clone + 'static,
    {
        self.build_event(crate::event::click, callback)
    }

    fn on_input<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<String, M> + Clone + 'static,
    {
        let mut handler = callback.clone().into_handler();
        self.apply(silex_core::rx!(move |el: &web_sys::Element| {
            use wasm_bindgen::JsCast;
            let closure =
                wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
                    if let Some(target) = e.target() {
                        let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                        handler(input.value());
                    } else {
                        let err =
                            silex_core::error::SilexError::Dom("Input event has no target".into());
                        silex_core::error::handle_error(err);
                    }
                }) as Box<dyn FnMut(_)>);

            let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();

            if let Err(e) = el
                .add_event_listener_with_callback("input", js_value)
                .map_err(silex_core::error::SilexError::from)
            {
                silex_core::error::handle_error(e);
                return;
            }

            let target = el.clone();
            let js_fn = js_value.clone();

            silex_core::reactivity::on_cleanup(move || {
                let _ = target.remove_event_listener_with_callback("input", &js_fn);
                drop(closure);
            });
        }))
    }

    fn bind_value(self, signal: silex_core::reactivity::RwSignal<String>) -> Self {
        use silex_core::traits::Set;
        let this = self.on_input(move |value| {
            signal.set(value);
        });

        this.apply(silex_core::rx!(move |el: &web_sys::Element| {
            let dom_element = el.clone();
            silex_core::reactivity::Effect::new(move |_| {
                use silex_core::traits::Get;
                use wasm_bindgen::JsCast;
                let value = signal.get();
                if let Some(input) = dom_element.dyn_ref::<web_sys::HtmlInputElement>() {
                    if input.value() != value {
                        input.set_value(&value);
                    }
                } else if let Some(area) = dom_element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                    if area.value() != value {
                        area.set_value(&value);
                    }
                } else if let Some(select) = dom_element.dyn_ref::<web_sys::HtmlSelectElement>() {
                    if select.value() != value {
                        select.set_value(&value);
                    }
                } else {
                    let _ = dom_element.set_attribute("value", &value);
                }
            });
        }))
    }

    fn on_untyped<E, F>(self, event_type: &str, callback: F) -> Self
    where
        E: wasm_bindgen::convert::FromWasmAbi + 'static,
        F: FnMut(E) + 'static + Clone,
    {
        let event_type_str = event_type.to_string();
        self.apply(silex_core::rx!(move |el: &web_sys::Element| {
            use wasm_bindgen::JsCast;
            let mut cb = callback.clone();
            let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |e: E| {
                cb(e);
            }) as Box<dyn FnMut(E)>);

            let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();

            if let Err(e) = el
                .add_event_listener_with_callback(&event_type_str, js_value)
                .map_err(silex_core::error::SilexError::from)
            {
                silex_core::error::handle_error(e);
                return;
            }

            let target = el.clone();
            let js_fn = js_value.clone();
            let type_clone = event_type_str.clone();

            silex_core::reactivity::on_cleanup(move || {
                let _ = target.remove_event_listener_with_callback(&type_clone, &js_fn);
                drop(closure);
            });
        }))
    }
}

// 自动实现全局事件属性
impl<T: AttributeBuilder> GlobalEventAttributes for T {}
