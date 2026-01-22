use crate::dom::Element;
use crate::dom::attribute::AttributeValue;
use crate::dom::core::tags::*;
use crate::dom::element::TypedElement;
use crate::dom::view::View;

/// 任何可以设置属性的类型都需要实现此 Trait。
pub trait AttributeManager: Sized {
    /// 基础的属性设置方法。
    fn attr_any(self, name: &str, value: impl AttributeValue) -> Self;
}

// 为 Element 实现 AttributeManager
impl AttributeManager for Element {
    fn attr_any(self, name: &str, value: impl AttributeValue) -> Self {
        self.attr(name, value)
    }
}

// 为 TypedElement 实现 AttributeManager
impl<T> AttributeManager for TypedElement<T> {
    fn attr_any(self, name: &str, value: impl AttributeValue) -> Self {
        self.attr(name, value)
    }
}

// --- Content Traits ---

/// Trait for elements that can contain text (non-void elements).
pub trait WithText: Sized {
    fn text<V: View>(self, content: V) -> Self;
}

impl WithText for Element {
    fn text<V: View>(self, content: V) -> Self {
        self.child(content)
    }
}

impl<T: TextTag> WithText for TypedElement<T> {
    fn text<V: View>(self, content: V) -> Self {
        self.child(content)
    }
}

// --- 分层 Trait 定义 ---

/// 全局属性：所有 HTML 元素都支持的属性
pub trait GlobalAttributes: AttributeManager {
    fn id(self, value: impl AttributeValue) -> Self {
        self.attr_any("id", value)
    }

    fn class(self, value: impl AttributeValue) -> Self {
        self.attr_any("class", value)
    }

    fn style(self, value: impl AttributeValue) -> Self {
        self.attr_any("style", value)
    }

    fn title(self, value: impl AttributeValue) -> Self {
        self.attr_any("title", value)
    }

    fn lang(self, value: impl AttributeValue) -> Self {
        self.attr_any("lang", value)
    }

    fn dir(self, value: impl AttributeValue) -> Self {
        self.attr_any("dir", value)
    }

    fn tabindex(self, value: impl AttributeValue) -> Self {
        self.attr_any("tabindex", value)
    }

    fn draggable(self, value: impl AttributeValue) -> Self {
        self.attr_any("draggable", value)
    }

    fn hidden(self, value: impl AttributeValue) -> Self {
        self.attr_any("hidden", value)
    }
}

// 自动为所有 TypedElement<T> 实现 GlobalAttributes
impl<T> GlobalAttributes for TypedElement<T> {}
impl GlobalAttributes for Element {}

/// 表单与输入属性：主要用于 input, select, textarea, button, form
pub trait FormAttributes: AttributeManager {
    // ...
    /// 设置 input 的 type 属性 (注意：使用 type_ 以避免关键字冲突)
    fn type_(self, value: impl AttributeValue) -> Self {
        self.attr_any("type", value)
    }

    fn value(self, value: impl AttributeValue) -> Self {
        self.attr_any("value", value)
    }

    fn checked(self, value: impl AttributeValue) -> Self {
        self.attr_any("checked", value)
    }

    fn disabled(self, value: impl AttributeValue) -> Self {
        self.attr_any("disabled", value)
    }

    fn readonly(self, value: impl AttributeValue) -> Self {
        self.attr_any("readonly", value)
    }

    fn required(self, value: impl AttributeValue) -> Self {
        self.attr_any("required", value)
    }

    fn placeholder(self, value: impl AttributeValue) -> Self {
        self.attr_any("placeholder", value)
    }

    fn name(self, value: impl AttributeValue) -> Self {
        self.attr_any("name", value)
    }

    fn autocomplete(self, value: impl AttributeValue) -> Self {
        self.attr_any("autocomplete", value)
    }

    fn autofocus(self, value: impl AttributeValue) -> Self {
        self.attr_any("autofocus", value)
    }

    fn min(self, value: impl AttributeValue) -> Self {
        self.attr_any("min", value)
    }

    fn max(self, value: impl AttributeValue) -> Self {
        self.attr_any("max", value)
    }

    fn step(self, value: impl AttributeValue) -> Self {
        self.attr_any("step", value)
    }

    fn pattern(self, value: impl AttributeValue) -> Self {
        self.attr_any("pattern", value)
    }

    fn multiple(self, value: impl AttributeValue) -> Self {
        self.attr_any("multiple", value)
    }

    fn accept(self, value: impl AttributeValue) -> Self {
        self.attr_any("accept", value)
    }

    fn selected(self, value: impl AttributeValue) -> Self {
        self.attr_any("selected", value)
    }

    fn rows(self, value: impl AttributeValue) -> Self {
        self.attr_any("rows", value)
    }

    fn cols(self, value: impl AttributeValue) -> Self {
        self.attr_any("cols", value)
    }

    fn action(self, value: impl AttributeValue) -> Self {
        self.attr_any("action", value)
    }

    fn method(self, value: impl AttributeValue) -> Self {
        self.attr_any("method", value)
    }
}

impl<T: FormTag> FormAttributes for TypedElement<T> {}

/// 标签属性：主要用于 label
pub trait LabelAttributes: AttributeManager {
    /// label 的 for 属性 (使用 for_)
    fn for_(self, value: impl AttributeValue) -> Self {
        self.attr_any("for", value)
    }
}

impl<T: LabelTag> LabelAttributes for TypedElement<T> {}

/// 链接属性：主要用于 a, link, area
pub trait AnchorAttributes: AttributeManager {
    fn href(self, value: impl AttributeValue) -> Self {
        self.attr_any("href", value)
    }

    fn target(self, value: impl AttributeValue) -> Self {
        self.attr_any("target", value)
    }

    fn rel(self, value: impl AttributeValue) -> Self {
        self.attr_any("rel", value)
    }
}

impl<T: AnchorTag> AnchorAttributes for TypedElement<T> {}

/// 媒体属性：主要用于 img, video, audio, source, iframe
pub trait MediaAttributes: AttributeManager {
    fn src(self, value: impl AttributeValue) -> Self {
        self.attr_any("src", value)
    }

    fn alt(self, value: impl AttributeValue) -> Self {
        self.attr_any("alt", value)
    }

    fn width(self, value: impl AttributeValue) -> Self {
        self.attr_any("width", value)
    }

    fn height(self, value: impl AttributeValue) -> Self {
        self.attr_any("height", value)
    }
}

impl<T: MediaTag> MediaAttributes for TypedElement<T> {}

/// ARIA 无障碍属性：提供给所有元素使用
pub trait AriaAttributes: AttributeManager {
    fn role(self, value: impl AttributeValue) -> Self {
        self.attr_any("role", value)
    }

    fn aria_label(self, value: impl AttributeValue) -> Self {
        self.attr_any("aria-label", value)
    }

    fn aria_hidden(self, value: impl AttributeValue) -> Self {
        self.attr_any("aria-hidden", value)
    }
}

impl<T> AriaAttributes for TypedElement<T> {}
impl AriaAttributes for Element {}
