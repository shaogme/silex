use silex_dom::prelude::*;

/// 表单与输入属性：主要用于 input, select, textarea, button, form
pub trait FormAttributes: AttributeBuilder {
    /// 设置 input 的 type 属性 (注意：使用 type_ 以避免关键字冲突)
    fn type_(self, value: impl IntoStorable) -> Self {
        self.attr("type", value)
    }

    fn value(self, value: impl IntoStorable) -> Self {
        self.prop("value", value)
    }

    fn checked(self, value: impl IntoStorable) -> Self {
        self.prop("checked", value)
    }

    fn disabled(self, value: impl IntoStorable) -> Self {
        self.prop("disabled", value)
    }

    fn readonly(self, value: impl IntoStorable) -> Self {
        self.prop("readOnly", value)
    }

    fn required(self, value: impl IntoStorable) -> Self {
        self.prop("required", value)
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
        self.prop("multiple", value)
    }

    fn accept(self, value: impl IntoStorable) -> Self {
        self.attr("accept", value)
    }

    fn selected(self, value: impl IntoStorable) -> Self {
        self.prop("selected", value)
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

/// 标签属性：主要用于 label
pub trait LabelAttributes: AttributeBuilder {
    /// label 的 for 属性 (使用 for_)
    fn for_(self, value: impl IntoStorable) -> Self {
        self.attr("for", value)
    }
}

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

    fn download(self, value: impl IntoStorable) -> Self {
        self.attr("download", value)
    }
}

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

    fn autoplay(self, value: impl IntoStorable) -> Self {
        self.prop("autoplay", value)
    }

    fn controls(self, value: impl IntoStorable) -> Self {
        self.prop("controls", value)
    }

    fn loop_(self, value: impl IntoStorable) -> Self {
        self.prop("loop", value)
    }

    fn muted(self, value: impl IntoStorable) -> Self {
        self.prop("muted", value)
    }

    fn poster(self, value: impl IntoStorable) -> Self {
        self.attr("poster", value)
    }

    fn preload(self, value: impl IntoStorable) -> Self {
        self.attr("preload", value)
    }
}

/// 交互属性：主要用于 dialog, details
pub trait OpenAttributes: AttributeBuilder {
    fn open(self, value: impl IntoStorable) -> Self {
        self.prop("open", value)
    }
}

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

/// 表头属性：主要用于 th
pub trait TableHeaderAttributes: AttributeBuilder {
    fn scope(self, value: impl IntoStorable) -> Self {
        self.attr("scope", value)
    }

    fn abbr(self, value: impl IntoStorable) -> Self {
        self.attr("abbr", value)
    }
}

// --- Blanket Implementations for TypedElement<T> ---

impl<T: FormTag> FormAttributes for TypedElement<T> {}
impl<T: LabelTag> LabelAttributes for TypedElement<T> {}
impl<T: AnchorTag> AnchorAttributes for TypedElement<T> {}
impl<T: MediaTag> MediaAttributes for TypedElement<T> {}
impl<T: OpenTag> OpenAttributes for TypedElement<T> {}
impl<T: TableCellTag> TableCellAttributes for TypedElement<T> {}
impl<T: TableHeaderTag> TableHeaderAttributes for TypedElement<T> {}
