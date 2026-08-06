use std::borrow::Cow;

use wasm_bindgen::JsCast;
use wasm_bindgen::convert::FromWasmAbi;
use web_sys::{Element, Event, InputEvent, MouseEvent, PointerEvent};

use silex_core::{
    ReactiveError,
    log::console_error,
    node_ref::NodeRef,
    reactivity::{ReactiveSource, runtime_inputs_of},
    traits::{RxGet, RxWrite},
};

use crate::{
    element::bind_event_impl,
    event::{
        EventDescriptor, EventHandler, click, pointercancel, pointerdown, pointermove, pointerup,
    },
    helpers::event_target_value_result,
    view::{AnyView, ApplyAttributes},
};

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
            $( $crate::attribute::ApplyToDom::into_op($attr, $crate::attribute::ApplyTarget::Apply) ),*
        ])
    };
}

pub trait AttributeBuilder<'scope>: Sized {
    /// Core hook: Apply or store a generic attribute/property directly using ApplyTarget mechanism.
    /// Accepts any type that implements IntoStorable, allowing both static references (&str, &String)
    /// and owned/reactive types (String, Signal, closures).
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>;

    /// Core hook: Apply or store an event listener.
    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope;

    // === Unified Mixins (Default Implementation) ===

    fn attr<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::attr(name), value)
    }

    fn prop<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::prop(name), value)
    }

    fn on<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.build_event(event, callback)
    }

    /// Generic application of a value that knows how to apply itself to the DOM.
    /// Useful for mixins, theme variables, or complex reactive logic.
    fn apply<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        // Wrap in a storable type and build
        self.build_attribute(ApplyTarget::Apply, value)
    }
}

// --- 分层 Trait 定义 (from props.rs) ---

/// 全局属性：所有 HTML 元素都支持的属性
pub trait GlobalAttributes<'scope>: AttributeBuilder<'scope> {
    fn id(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("id", value)
    }

    fn class(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("class", value)
    }

    fn style(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("style", value)
    }

    fn title(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("title", value)
    }

    fn lang(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("lang", value)
    }

    fn dir(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("dir", value)
    }

    fn tabindex(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("tabindex", value)
    }

    fn draggable(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("draggable", value)
    }

    fn hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("hidden", value)
    }
}

// 自动为所有实现 AttributeBuilder 的类型实现 GlobalAttributes
impl<'scope, T: AttributeBuilder<'scope>> GlobalAttributes<'scope> for T {}

/// ARIA 无障碍属性：提供给所有元素使用
pub trait AriaAttributes<'scope>: AttributeBuilder<'scope> {
    fn role(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("role", value)
    }

    fn aria_label(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-label", value)
    }

    fn aria_labelledby(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-labelledby", value)
    }

    fn aria_describedby(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-describedby", value)
    }

    fn aria_hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-hidden", value)
    }

    fn aria_expanded(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-expanded", value)
    }

    fn aria_checked(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-checked", value)
    }

    fn aria_selected(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-selected", value)
    }

    fn aria_controls(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-controls", value)
    }

    fn aria_disabled(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-disabled", value)
    }

    fn aria_invalid(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-invalid", value)
    }

    fn aria_required(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-required", value)
    }

    fn aria_valuenow(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-valuenow", value)
    }

    fn aria_valuemin(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-valuemin", value)
    }

    fn aria_valuemax(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-valuemax", value)
    }

    fn aria_orientation(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-orientation", value)
    }

    fn aria_haspopup(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-haspopup", value)
    }

    fn aria_live(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-live", value)
    }

    fn aria_atomic(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-atomic", value)
    }

    fn aria_modal(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-modal", value)
    }
}

// 自动为所有实现 AttributeBuilder 的类型实现 AriaAttributes
impl<'scope, T: AttributeBuilder<'scope>> AriaAttributes<'scope> for T {}

/// 全局事件与通用组件方法：提供诸如 on_click, class_toggle, bind_value 等常用操作
pub trait GlobalEventAttributes<'scope>: AttributeBuilder<'scope> {
    fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, (name.to_string(), condition))
    }

    fn classes<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, value)
    }

    fn node_ref<N>(self, node_ref: NodeRef<'scope, N>) -> Self
    where
        N: JsCast + Clone + 'scope,
    {
        let node_ref_for_cleanup = node_ref;
        self.apply(PendingAttribute::new_scoped(move |el: &Element, owner| {
            if let Ok(typed) = el.clone().dyn_into::<N>() {
                if let Err(error) = node_ref.try_load(typed) {
                    owner.report_error(error.into());
                }
            } else {
                console_error("NodeRef type mismatch: failed to cast element");
            }
        }))
        .apply(PendingAttribute::new_scoped(move |_el, owner| {
            let owner_for_cleanup = owner.clone();
            if let Err(error) = owner.on_cleanup(Box::new(move || {
                if let Err(error) = node_ref_for_cleanup.try_clear()
                    && !matches!(error, ReactiveError::NoSuchNode)
                {
                    owner_for_cleanup.report_error(error.into());
                }
            })) {
                owner.report_error(error);
            }
        }))
    }

    // --- Event API ---

    fn on_click<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, MouseEvent, M> + Clone + 'scope,
    {
        self.build_event(click, callback)
    }

    fn on_pointer_down<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, PointerEvent, M> + Clone + 'scope,
    {
        self.build_event(pointerdown, callback)
    }

    fn on_pointer_move<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, PointerEvent, M> + Clone + 'scope,
    {
        self.build_event(pointermove, callback)
    }

    fn on_pointer_up<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, PointerEvent, M> + Clone + 'scope,
    {
        self.build_event(pointerup, callback)
    }

    fn on_pointer_cancel<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, PointerEvent, M> + Clone + 'scope,
    {
        self.build_event(pointercancel, callback)
    }

    fn on_input<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, String, M> + Clone + 'scope,
    {
        self.apply(PendingAttribute::new_scoped(move |el: &Element, owner| {
            let owner_for_handler = owner.clone();
            bind_event_impl(
                el,
                "input".to_string(),
                Box::new({
                    let mut handler = callback.clone().into_handler();
                    move |e: InputEvent| match event_target_value_result(&e) {
                        Ok(value) => handler(value),
                        Err(err) => owner_for_handler.report_error(err),
                    }
                }),
                owner,
            );
        }))
    }

    fn on_change<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, String, M> + Clone + 'scope,
    {
        self.apply(PendingAttribute::new_scoped(move |el: &Element, owner| {
            let owner_for_handler = owner.clone();
            bind_event_impl(
                el,
                "change".to_string(),
                Box::new({
                    let mut handler = callback.clone().into_handler();
                    move |e: Event| match event_target_value_result(&e) {
                        Ok(value) => handler(value),
                        Err(err) => owner_for_handler.report_error(err),
                    }
                }),
                owner,
            );
        }))
    }

    fn bind_value<T, S>(self, signal: S) -> Self
    where
        T: AsRef<str> + From<String> + Clone + PartialEq + 'scope,
        S: RxGet<Value = T> + RxWrite + ReactiveSource<'scope> + Clone + 'scope,
    {
        let s = signal.clone();
        let this = self.on_input(move |value| {
            s.set(T::from(value));
        });

        this.apply(PendingAttribute::new_scoped(move |el: &Element, owner| {
            let dom_element = el.clone();
            let signal = signal.clone();
            let owner = owner.clone();
            if let Err(error) = owner.effect_from(
                runtime_inputs_of(signal.clone()),
                Box::new(move || {
                    let value = signal.get();
                    let str_val = value.as_ref();
                    apply_attr_with_target_internal(
                        &dom_element,
                        "value",
                        ApplyTarget::Known(KnownProp::Value),
                        &Attr::from(str_val.to_string()),
                    );
                }),
            ) {
                owner.report_error(error);
            }
        }))
    }

    fn on_untyped<E, F>(self, event_type: &str, callback: F) -> Self
    where
        E: FromWasmAbi + JsCast + 'static,
        F: FnMut(E) + 'scope + Clone,
    {
        let event_type_str = event_type.to_string();
        let cb_template = callback.clone();
        self.apply(PendingAttribute::new_scoped(move |el: &Element, owner| {
            bind_event_impl(
                el,
                event_type_str.clone(),
                Box::new(cb_template.clone()),
                owner,
            );
        }))
    }
}

// 自动实现全局事件属性
impl<'scope, T: AttributeBuilder<'scope>> GlobalEventAttributes<'scope> for T {}

// --- AttributeBuilder Implementations for Erasure Types ---

impl<'scope> AttributeBuilder<'scope> for AnyView<'scope> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.apply_attributes(vec![PendingAttribute::build(value.into_storable(), target)]);
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.apply_attributes(vec![PendingAttribute::new_scoped(move |el, owner| {
            crate::element::bind_event(el, event, callback.clone(), owner);
        })]);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::Runtime;

    #[test]
    fn test_known_prop_reactive_bool_into_op() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let signal = scope.rw_signal(true);
            let target = ApplyTarget::Known(KnownProp::Disabled);
            let pending = PendingAttribute::build(signal.into_storable(), target);
            match pending {
                AttrOp::Update(AttrUpdate { target, .. }) => {
                    assert_eq!(target, ApplyTarget::Known(KnownProp::Disabled));
                }
                _ => panic!("Expected AttrOp::Update for KnownProp reactive bool"),
            }
        });
    }

    #[test]
    fn test_tuple_bool_class_into_op() {
        let op_true = ("active", true).into_op(ApplyTarget::Class);
        assert_eq!(
            op_true,
            AttrOp::static_class(std::borrow::Cow::Borrowed("active"))
        );

        let op_false = ("active", false).into_op(ApplyTarget::Class);
        assert_eq!(op_false, AttrOp::Noop);
    }

    #[test]
    fn test_static_str_class_into_op_splits_tokens() {
        let op = "first second".into_op(ApplyTarget::Class);
        assert_eq!(
            op,
            AttrOp::static_classes(vec![
                std::borrow::Cow::Borrowed("first"),
                std::borrow::Cow::Borrowed("second"),
            ])
        );
    }

    #[test]
    fn test_tuple_reactive_bool_class_into_op() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let signal = scope.rw_signal(true);
            let rx = signal.into_rx();
            let op = ("active", rx).into_op(ApplyTarget::Class);
            assert_eq!(op, AttrOp::class_toggle(Cow::Borrowed("active"), rx));
        });
    }

    #[test]
    fn test_tuple_reactive_string_style_into_op() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let signal = scope.rw_signal("10px".to_string());
            let rx = signal.into_rx();
            let op = ("margin", rx).into_op(ApplyTarget::Style);
            assert_eq!(op, AttrOp::style_property(Cow::Borrowed("margin"), rx));
        });
    }

    #[test]
    fn test_tuple_static_string_style_into_op() {
        let op = ("color", "red").into_op(ApplyTarget::Style);
        assert_eq!(
            op,
            AttrOp::static_styles(vec![(Cow::Borrowed("color"), Cow::Borrowed("red"))])
        );
    }

    #[test]
    fn test_consolidate_attributes_dedup_and_combine() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let signal = scope.rw_signal(true);

            let attrs = vec![
                PendingAttribute::build("btn", ApplyTarget::Class),
                PendingAttribute::build("active", ApplyTarget::Class),
                PendingAttribute::build("btn", ApplyTarget::Class), // 重复项 (非相邻)
                PendingAttribute::build(("highlight", signal.into_rx()), ApplyTarget::Class),
            ];

            let consolidated = consolidate_attributes(attrs);
            assert_eq!(consolidated.len(), 1);

            match &consolidated[0] {
                AttrOp::CombinedClasses(cc) => {
                    assert_eq!(
                        cc.statics,
                        vec![Cow::Borrowed("btn"), Cow::Borrowed("active")]
                    );
                    assert_eq!(cc.toggles.len(), 1);
                    assert_eq!(cc.toggles[0].0, "highlight");
                }
                _ => panic!("Expected AttrOp::CombinedClasses"),
            }
        })
    }

    #[test]
    fn custom_with_inputs_keeps_declared_runtime_sources() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let source = scope.rw_signal(1i32).into_rx();
            let op = AttrOp::custom_with_inputs(source.runtime_inputs(), |_, _| {});

            assert_eq!(op.runtime_inputs().len(), 1);
            assert!(format!("{op:?}").contains("CustomWithInputs"));
        });
    }
}
