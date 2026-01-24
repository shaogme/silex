use super::IntoStorable;
use crate::attribute::AttributeBuilder;
use crate::element::TypedElement;
use crate::element::tags::*;

// AttributeManager removed - traits now inherit from AttributeBuilder

// --- 分层 Trait 定义 ---

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

/// 表单与输入属性：主要用于 input, select, textarea, button, form
/// 表单与输入属性：主要用于 input, select, textarea, button, form
pub trait FormAttributes: AttributeBuilder {
    // ...
    /// 设置 input 的 type 属性 (注意：使用 type_ 以避免关键字冲突)
    fn type_(self, value: impl IntoStorable) -> Self {
        self.attr("type", value)
    }

    fn value(self, value: impl IntoStorable) -> Self {
        self.attr("value", value)
    }

    fn checked(self, value: impl IntoStorable) -> Self {
        self.attr("checked", value)
    }

    fn disabled(self, value: impl IntoStorable) -> Self {
        self.attr("disabled", value)
    }

    fn readonly(self, value: impl IntoStorable) -> Self {
        self.attr("readonly", value)
    }

    fn required(self, value: impl IntoStorable) -> Self {
        self.attr("required", value)
    }

    fn placeholder(self, value: impl IntoStorable) -> Self {
        self.attr("placeholder", value)
    }

    fn name(self, value: impl IntoStorable) -> Self {
        self.attr("name", value)
    }

    fn autocomplete(self, value: impl IntoStorable) -> Self {
        self.attr("autocomplete", value)
    }

    fn autofocus(self, value: impl IntoStorable) -> Self {
        self.attr("autofocus", value)
    }

    fn min(self, value: impl IntoStorable) -> Self {
        self.attr("min", value)
    }

    fn max(self, value: impl IntoStorable) -> Self {
        self.attr("max", value)
    }

    fn step(self, value: impl IntoStorable) -> Self {
        self.attr("step", value)
    }

    fn pattern(self, value: impl IntoStorable) -> Self {
        self.attr("pattern", value)
    }

    fn multiple(self, value: impl IntoStorable) -> Self {
        self.attr("multiple", value)
    }

    fn accept(self, value: impl IntoStorable) -> Self {
        self.attr("accept", value)
    }

    fn selected(self, value: impl IntoStorable) -> Self {
        self.attr("selected", value)
    }

    fn rows(self, value: impl IntoStorable) -> Self {
        self.attr("rows", value)
    }

    fn cols(self, value: impl IntoStorable) -> Self {
        self.attr("cols", value)
    }

    fn action(self, value: impl IntoStorable) -> Self {
        self.attr("action", value)
    }

    fn method(self, value: impl IntoStorable) -> Self {
        self.attr("method", value)
    }
}

impl<T: FormTag> FormAttributes for TypedElement<T> {}

/// 标签属性：主要用于 label
/// 标签属性：主要用于 label
pub trait LabelAttributes: AttributeBuilder {
    /// label 的 for 属性 (使用 for_)
    fn for_(self, value: impl IntoStorable) -> Self {
        self.attr("for", value)
    }
}

impl<T: LabelTag> LabelAttributes for TypedElement<T> {}

/// 链接属性：主要用于 a, link, area
/// 链接属性：主要用于 a, link, area
pub trait AnchorAttributes: AttributeBuilder {
    fn href(self, value: impl IntoStorable) -> Self {
        self.attr("href", value)
    }

    fn target(self, value: impl IntoStorable) -> Self {
        self.attr("target", value)
    }

    fn rel(self, value: impl IntoStorable) -> Self {
        self.attr("rel", value)
    }
}

impl<T: AnchorTag> AnchorAttributes for TypedElement<T> {}

/// 媒体属性：主要用于 img, video, audio, source, iframe
/// 媒体属性：主要用于 img, video, audio, source, iframe
pub trait MediaAttributes: AttributeBuilder {
    fn src(self, value: impl IntoStorable) -> Self {
        self.attr("src", value)
    }

    fn alt(self, value: impl IntoStorable) -> Self {
        self.attr("alt", value)
    }

    fn width(self, value: impl IntoStorable) -> Self {
        self.attr("width", value)
    }

    fn height(self, value: impl IntoStorable) -> Self {
        self.attr("height", value)
    }
}

impl<T: MediaTag> MediaAttributes for TypedElement<T> {}

/// 交互属性：主要用于 dialog, details
/// 交互属性：主要用于 dialog, details
pub trait OpenAttributes: AttributeBuilder {
    fn open(self, value: impl IntoStorable) -> Self {
        self.attr("open", value)
    }
}

impl<T: OpenTag> OpenAttributes for TypedElement<T> {}

/// 表格单元格属性：主要用于 td, th
/// 表格单元格属性：主要用于 td, th
pub trait TableCellAttributes: AttributeBuilder {
    fn colspan(self, value: impl IntoStorable) -> Self {
        self.attr("colspan", value)
    }

    fn rowspan(self, value: impl IntoStorable) -> Self {
        self.attr("rowspan", value)
    }

    fn headers(self, value: impl IntoStorable) -> Self {
        self.attr("headers", value)
    }
}

impl<T: TableCellTag> TableCellAttributes for TypedElement<T> {}

/// 表头属性：主要用于 th
/// 表头属性：主要用于 th
pub trait TableHeaderAttributes: AttributeBuilder {
    fn scope(self, value: impl IntoStorable) -> Self {
        self.attr("scope", value)
    }

    fn abbr(self, value: impl IntoStorable) -> Self {
        self.attr("abbr", value)
    }
}

impl<T: TableHeaderTag> TableHeaderAttributes for TypedElement<T> {}

/// ARIA 无障碍属性：提供给所有元素使用
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
