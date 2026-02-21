use crate::event::{EventDescriptor, EventHandler};

mod apply;
mod into_storable;
mod typed;

pub use apply::*;
pub use into_storable::*;
pub use typed::*;

// --- Attribute Builder Trait ---

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
