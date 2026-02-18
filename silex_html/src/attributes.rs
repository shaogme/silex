use silex_dom::{ApplyBoolAttribute, ApplyStringAttribute, AttributeBuilder, IntoStorable};

/// 表单与输入属性：主要用于 input, select, textarea, button, form
pub trait FormAttributes: AttributeBuilder {
    /// 设置 input 的 type 属性 (注意：使用 type_ 以避免关键字冲突)
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("type", value)
    }

    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.prop("value", value)
    }

    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }

    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("disabled", value)
    }

    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }

    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("required", value)
    }

    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
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

    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }

    fn accept(self, value: impl IntoStorable) -> Self {
        self.attr("accept", value)
    }

    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
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
    fn for_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("for", value)
    }
}

/// 链接属性：主要用于 a, link, area
pub trait AnchorAttributes: AttributeBuilder {
    fn href<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("href", value)
    }

    fn target<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("target", value)
    }

    fn rel<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("rel", value)
    }

    fn download<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("download", value)
    }
}

/// 媒体属性：主要用于 img, video, audio, source, iframe
pub trait MediaAttributes: AttributeBuilder {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
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

    fn autoplay<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("autoplay", value)
    }

    fn controls<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("controls", value)
    }

    fn loop_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("loop", value)
    }

    fn muted<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
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
    fn open<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyBoolAttribute,
    {
        self.prop("open", value)
    }
}

/// 表格单元格属性：主要用于 td, th
pub trait TableCellAttributes: AttributeBuilder {
    fn colspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("colspan", value)
    }

    fn rowspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("rowspan", value)
    }

    fn headers<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("headers", value)
    }
}

/// 表头属性：主要用于 th
pub trait TableHeaderAttributes: AttributeBuilder {
    fn scope<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("scope", value)
    }

    fn abbr<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: ApplyStringAttribute,
    {
        self.attr("abbr", value)
    }
}
