use super::ApplyToDom;
use crate::element::Element;
use crate::element::TypedElement;
use crate::element::tags::*;

/// 任何可以设置属性的类型都需要实现此 Trait。
pub trait AttributeManager: Sized {
    /// 基础的属性设置方法。
    fn attr_any(self, name: &str, value: impl ApplyToDom) -> Self;
}

// 为 Element 实现 AttributeManager
impl AttributeManager for Element {
    fn attr_any(self, name: &str, value: impl ApplyToDom) -> Self {
        self.attr(name, value)
    }
}

// 为 TypedElement 实现 AttributeManager
impl<T> AttributeManager for TypedElement<T> {
    fn attr_any(self, name: &str, value: impl ApplyToDom) -> Self {
        self.attr(name, value)
    }
}

// --- 分层 Trait 定义 ---

/// 全局属性：所有 HTML 元素都支持的属性
pub trait GlobalAttributes: AttributeManager {
    fn id(self, value: impl ApplyToDom) -> Self {
        self.attr_any("id", value)
    }

    fn class(self, value: impl ApplyToDom) -> Self {
        self.attr_any("class", value)
    }

    fn style(self, value: impl ApplyToDom) -> Self {
        self.attr_any("style", value)
    }

    fn title(self, value: impl ApplyToDom) -> Self {
        self.attr_any("title", value)
    }

    fn lang(self, value: impl ApplyToDom) -> Self {
        self.attr_any("lang", value)
    }

    fn dir(self, value: impl ApplyToDom) -> Self {
        self.attr_any("dir", value)
    }

    fn tabindex(self, value: impl ApplyToDom) -> Self {
        self.attr_any("tabindex", value)
    }

    fn draggable(self, value: impl ApplyToDom) -> Self {
        self.attr_any("draggable", value)
    }

    fn hidden(self, value: impl ApplyToDom) -> Self {
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
    fn type_(self, value: impl ApplyToDom) -> Self {
        self.attr_any("type", value)
    }

    fn value(self, value: impl ApplyToDom) -> Self {
        self.attr_any("value", value)
    }

    fn checked(self, value: impl ApplyToDom) -> Self {
        self.attr_any("checked", value)
    }

    fn disabled(self, value: impl ApplyToDom) -> Self {
        self.attr_any("disabled", value)
    }

    fn readonly(self, value: impl ApplyToDom) -> Self {
        self.attr_any("readonly", value)
    }

    fn required(self, value: impl ApplyToDom) -> Self {
        self.attr_any("required", value)
    }

    fn placeholder(self, value: impl ApplyToDom) -> Self {
        self.attr_any("placeholder", value)
    }

    fn name(self, value: impl ApplyToDom) -> Self {
        self.attr_any("name", value)
    }

    fn autocomplete(self, value: impl ApplyToDom) -> Self {
        self.attr_any("autocomplete", value)
    }

    fn autofocus(self, value: impl ApplyToDom) -> Self {
        self.attr_any("autofocus", value)
    }

    fn min(self, value: impl ApplyToDom) -> Self {
        self.attr_any("min", value)
    }

    fn max(self, value: impl ApplyToDom) -> Self {
        self.attr_any("max", value)
    }

    fn step(self, value: impl ApplyToDom) -> Self {
        self.attr_any("step", value)
    }

    fn pattern(self, value: impl ApplyToDom) -> Self {
        self.attr_any("pattern", value)
    }

    fn multiple(self, value: impl ApplyToDom) -> Self {
        self.attr_any("multiple", value)
    }

    fn accept(self, value: impl ApplyToDom) -> Self {
        self.attr_any("accept", value)
    }

    fn selected(self, value: impl ApplyToDom) -> Self {
        self.attr_any("selected", value)
    }

    fn rows(self, value: impl ApplyToDom) -> Self {
        self.attr_any("rows", value)
    }

    fn cols(self, value: impl ApplyToDom) -> Self {
        self.attr_any("cols", value)
    }

    fn action(self, value: impl ApplyToDom) -> Self {
        self.attr_any("action", value)
    }

    fn method(self, value: impl ApplyToDom) -> Self {
        self.attr_any("method", value)
    }
}

impl<T: FormTag> FormAttributes for TypedElement<T> {}

/// 标签属性：主要用于 label
pub trait LabelAttributes: AttributeManager {
    /// label 的 for 属性 (使用 for_)
    fn for_(self, value: impl ApplyToDom) -> Self {
        self.attr_any("for", value)
    }
}

impl<T: LabelTag> LabelAttributes for TypedElement<T> {}

/// 链接属性：主要用于 a, link, area
pub trait AnchorAttributes: AttributeManager {
    fn href(self, value: impl ApplyToDom) -> Self {
        self.attr_any("href", value)
    }

    fn target(self, value: impl ApplyToDom) -> Self {
        self.attr_any("target", value)
    }

    fn rel(self, value: impl ApplyToDom) -> Self {
        self.attr_any("rel", value)
    }
}

impl<T: AnchorTag> AnchorAttributes for TypedElement<T> {}

/// 媒体属性：主要用于 img, video, audio, source, iframe
pub trait MediaAttributes: AttributeManager {
    fn src(self, value: impl ApplyToDom) -> Self {
        self.attr_any("src", value)
    }

    fn alt(self, value: impl ApplyToDom) -> Self {
        self.attr_any("alt", value)
    }

    fn width(self, value: impl ApplyToDom) -> Self {
        self.attr_any("width", value)
    }

    fn height(self, value: impl ApplyToDom) -> Self {
        self.attr_any("height", value)
    }
}

impl<T: MediaTag> MediaAttributes for TypedElement<T> {}

/// 交互属性：主要用于 dialog, details
pub trait OpenAttributes: AttributeManager {
    fn open(self, value: impl ApplyToDom) -> Self {
        self.attr_any("open", value)
    }
}

impl<T: OpenTag> OpenAttributes for TypedElement<T> {}

/// 表格单元格属性：主要用于 td, th
pub trait TableCellAttributes: AttributeManager {
    fn colspan(self, value: impl ApplyToDom) -> Self {
        self.attr_any("colspan", value)
    }

    fn rowspan(self, value: impl ApplyToDom) -> Self {
        self.attr_any("rowspan", value)
    }

    fn headers(self, value: impl ApplyToDom) -> Self {
        self.attr_any("headers", value)
    }
}

impl<T: TableCellTag> TableCellAttributes for TypedElement<T> {}

/// 表头属性：主要用于 th
pub trait TableHeaderAttributes: AttributeManager {
    fn scope(self, value: impl ApplyToDom) -> Self {
        self.attr_any("scope", value)
    }

    fn abbr(self, value: impl ApplyToDom) -> Self {
        self.attr_any("abbr", value)
    }
}

impl<T: TableHeaderTag> TableHeaderAttributes for TypedElement<T> {}

/// ARIA 无障碍属性：提供给所有元素使用
pub trait AriaAttributes: AttributeManager {
    fn role(self, value: impl ApplyToDom) -> Self {
        self.attr_any("role", value)
    }

    fn aria_label(self, value: impl ApplyToDom) -> Self {
        self.attr_any("aria-label", value)
    }

    fn aria_hidden(self, value: impl ApplyToDom) -> Self {
        self.attr_any("aria-hidden", value)
    }
}

impl<T> AriaAttributes for TypedElement<T> {}
impl AriaAttributes for Element {}
