use super::{model::ApplyTarget, operation::AttrOp, storage::IntoStorable};
use crate::events::{
    DomEvent, EventDescriptor, EventHandler, change, click, input, mouseenter, mouseleave,
    pointercancel, pointerdown, pointermove, pointerup,
};
use silex_core::{RxGet, RxWrite, SilexError};
use silex_dom::lifecycle::node_ref::NodeRef;
use std::borrow::Cow;
pub trait AttributeBuilder<'scope>: Sized {
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>;
    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, M> + Clone + 'scope;
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
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(event, callback)
    }

    fn apply<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Apply, value)
    }
}

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

    fn hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("hidden", value)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> GlobalAttributes<'scope> for T {}

pub trait AriaAttributes<'scope>: AttributeBuilder<'scope> {
    fn role(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("role", value)
    }

    fn aria_label(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-label", value)
    }

    fn aria_hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-hidden", value)
    }

    fn aria_expanded(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-expanded", value)
    }

    fn aria_controls(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-controls", value)
    }

    fn aria_disabled(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-disabled", value)
    }

    fn aria_checked(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-checked", value)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> AriaAttributes<'scope> for T {}

pub trait GlobalEventAttributes<'scope>: AttributeBuilder<'scope> {
    fn classes<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, value)
    }

    fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, (name.to_string(), condition))
    }

    fn node_ref(self, node_ref: NodeRef<'scope>) -> Self {
        self.apply(AttrOp::new_scoped(move |element, context| {
            let owner = context.owner();
            let binding = node_ref
                .bind_for_mount(element.node().clone())
                .map_err(SilexError::from)?;
            owner.on_cleanup(
                Box::new(move || binding.clear_if_current().map(|_| ()).map_err(Into::into)),
                context.error_handler(),
            )
        }))
    }

    fn on_click<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(click, callback)
    }

    fn on_input<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(input, callback)
    }

    /// 将表单控件的当前 value 与可写响应式值建立双向绑定。
    fn bind_value<T, S>(self, signal: S) -> Self
    where
        T: AsRef<str> + From<String> + Clone + PartialEq + 'scope,
        S: IntoStorable<'scope> + RxGet<Owned = T> + RxWrite<Owned = T> + Clone + 'scope,
    {
        let signal_for_input = signal.clone();
        self.on_input(move |event: DomEvent| {
            if let Some(value) = event.input_value() {
                signal_for_input.set(T::from(value))?;
            }
            Ok(())
        })
        .prop("value", signal)
    }

    fn on_change<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(change, callback)
    }

    fn on_pointer_down<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(pointerdown, callback)
    }

    fn on_pointer_move<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(pointermove, callback)
    }

    fn on_pointer_up<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(pointerup, callback)
    }

    fn on_pointer_cancel<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(pointercancel, callback)
    }

    fn on_mouse_enter<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(mouseenter, callback)
    }

    fn on_mouse_leave<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(mouseleave, callback)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> GlobalEventAttributes<'scope> for T {}
