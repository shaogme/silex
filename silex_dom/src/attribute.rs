use crate::event::{EventDescriptor, EventHandler};

mod apply;
mod into_storable;
mod op;

pub use apply::*;
pub use into_storable::*;
pub use op::*;

/// 指令组宏：将多个异构属性/事件平铺为一个 AttributeGroup。
/// 这在创建自定义 Mixin 或组件透传属性时非常有用。
#[macro_export]
macro_rules! group {
    ($($attr:expr),* $(,)?) => {
        $crate::attribute::AttributeGroup(vec![
            $( $crate::attribute::ApplyToDom::into_op($attr, $crate::attribute::OwnedApplyTarget::Apply) ),*
        ])
    };
}

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
        self.apply(AttrOp::Custom(std::rc::Rc::new(
            move |el: &web_sys::Element| {
                use wasm_bindgen::JsCast;
                if let Ok(typed) = el.clone().dyn_into::<N>() {
                    node_ref.load(typed);
                } else {
                    silex_core::log::console_error("NodeRef type mismatch: failed to cast element");
                }
            },
        )))
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
        self.apply(PendingAttribute::new_listener(
            move |el: &web_sys::Element| {
                crate::element::bind_event_impl(
                    el,
                    "input".to_string(),
                    Box::new({
                        let mut handler = callback.clone().into_handler();
                        move |e: web_sys::InputEvent| {
                            match crate::helpers::event_target_value_result(&e) {
                                Ok(value) => handler(value),
                                Err(err) => silex_core::error::handle_error(err),
                            }
                        }
                    }),
                );
            },
        ))
    }

    fn bind_value<S>(self, signal: S) -> Self
    where
        S: Into<silex_core::reactivity::RwSignal<String>>,
    {
        use silex_core::traits::RxWrite;
        let signal = signal.into();
        let this = self.on_input(move |value| {
            signal.set(value);
        });

        this.apply(PendingAttribute::new_listener(
            move |el: &web_sys::Element| {
                let dom_element = el.clone();
                silex_core::reactivity::Effect::new(move |_| {
                    use silex_core::traits::RxGet;
                    use wasm_bindgen::JsCast;
                    let value = signal.get();
                    if let Some(input) = dom_element.dyn_ref::<web_sys::HtmlInputElement>() {
                        if input.value() != value {
                            input.set_value(&value);
                        }
                    } else if let Some(area) = dom_element.dyn_ref::<web_sys::HtmlTextAreaElement>()
                    {
                        if area.value() != value {
                            area.set_value(&value);
                        }
                    } else if let Some(select) = dom_element.dyn_ref::<web_sys::HtmlSelectElement>()
                    {
                        if select.value() != value {
                            select.set_value(&value);
                        }
                    } else {
                        let _ = dom_element.set_attribute("value", &value);
                    }
                });
            },
        ))
    }

    fn on_untyped<E, F>(self, event_type: &str, callback: F) -> Self
    where
        E: wasm_bindgen::convert::FromWasmAbi + 'static,
        F: FnMut(E) + 'static + Clone,
    {
        let event_type_str = event_type.to_string();
        let cb_template = callback.clone();
        self.apply(PendingAttribute::new_listener(
            move |el: &web_sys::Element| {
                crate::element::bind_event_impl(
                    el,
                    event_type_str.clone(),
                    Box::new(cb_template.clone()),
                );
            },
        ))
    }
}

// 自动实现全局事件属性
impl<T: AttributeBuilder> GlobalEventAttributes for T {}

// --- AttributeBuilder Implementations for Erasure Types ---

impl AttributeBuilder for crate::view::AnyView {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable,
    {
        use crate::view::ApplyAttributes;
        self.apply_attributes(vec![PendingAttribute::build(
            value.into_storable(),
            OwnedApplyTarget::from(target),
        )]);
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: crate::event::EventDescriptor + 'static,
        F: crate::event::EventHandler<E::EventType, M> + Clone + 'static,
    {
        use crate::view::ApplyAttributes;
        self.apply_attributes(vec![PendingAttribute::new_listener(move |el| {
            crate::element::bind_event(el, event, callback.clone());
        })]);
        self
    }
}

impl AttributeBuilder for crate::view::SharedView {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable,
    {
        use crate::view::ApplyAttributes;
        self.apply_attributes(vec![PendingAttribute::build(
            value.into_storable(),
            OwnedApplyTarget::from(target),
        )]);
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: crate::event::EventDescriptor + 'static,
        F: crate::event::EventHandler<E::EventType, M> + Clone + 'static,
    {
        use crate::view::ApplyAttributes;
        self.apply_attributes(vec![PendingAttribute::new_listener(move |el| {
            crate::element::bind_event(el, event, callback.clone());
        })]);
        self
    }
}
