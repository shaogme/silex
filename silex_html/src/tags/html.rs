use crate::attributes::*;
use silex_dom::TypedElement;
use silex_dom::attribute::*;
use wasm_bindgen::JsCast;

// --- Tags ---
silex_dom::define_tag!(A, "a", a, new, non_void, [TextTag, AnchorTag]);
impl AnchorAttributes for TypedElement<A> {
    fn href<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAnchorElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_href(v));
        self
    }
    fn target<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAnchorElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_target(v));
        self
    }
    fn rel<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAnchorElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_rel(v));
        self
    }
    fn download<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAnchorElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_download(v));
        self
    }
}
silex_dom::define_tag!(Abbr, "abbr", abbr, new, non_void, [TextTag]);
silex_dom::define_tag!(Acronym, "acronym", acronym, new, non_void, [TextTag]);
silex_dom::define_tag!(Address, "address", address, new, non_void, [TextTag]);
silex_dom::define_tag!(Area, "area", area, new, void, [AnchorTag]);
impl AnchorAttributes for TypedElement<Area> {
    fn href<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_href(v));
        self
    }
    fn target<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_target(v));
        self
    }
    fn rel<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_rel(v));
        self
    }
    fn download<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_download(v));
        self
    }
}
silex_dom::define_tag!(Article, "article", article, new, non_void, [TextTag]);
silex_dom::define_tag!(Aside, "aside", aside, new, non_void, [TextTag]);
silex_dom::define_tag!(Audio, "audio", audio, new, non_void, [TextTag, MediaTag]);
impl MediaAttributes for TypedElement<Audio> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlAudioElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
    fn autoplay<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlAudioElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_autoplay(v));
        self
    }
    fn controls<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlAudioElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_controls(v));
        self
    }
    fn loop_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlAudioElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_loop(v));
        self
    }
    fn muted<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlAudioElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_muted(v));
        self
    }
}
silex_dom::define_tag!(B, "b", b, new, non_void, [TextTag]);
silex_dom::define_tag!(Base, "base", base, new, void, []);
silex_dom::define_tag!(Bdi, "bdi", bdi, new, non_void, [TextTag]);
silex_dom::define_tag!(Bdo, "bdo", bdo, new, non_void, [TextTag]);
silex_dom::define_tag!(Big, "big", big, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Blockquote,
    "blockquote",
    blockquote,
    new,
    non_void,
    [TextTag]
);
silex_dom::define_tag!(Body, "body", body, new, non_void, [TextTag]);
silex_dom::define_tag!(Br, "br", br, new, void, []);
silex_dom::define_tag!(Button, "button", button, new, non_void, [TextTag, FormTag]);
impl FormAttributes for TypedElement<Button> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlButtonElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlButtonElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(Canvas, "canvas", canvas, new, non_void, [TextTag]);
silex_dom::define_tag!(Caption, "caption", caption, new, non_void, [TextTag]);
silex_dom::define_tag!(Center, "center", center, new, non_void, [TextTag]);
silex_dom::define_tag!(Cite, "cite", cite, new, non_void, [TextTag]);
silex_dom::define_tag!(Code, "code", code, new, non_void, [TextTag]);
silex_dom::define_tag!(Col, "col", col, new, void, []);
silex_dom::define_tag!(Colgroup, "colgroup", colgroup, new, non_void, [TextTag]);
silex_dom::define_tag!(DataTag, "data", data_tag, new, non_void, [TextTag]);
silex_dom::define_tag!(Datalist, "datalist", datalist, new, non_void, [TextTag]);
silex_dom::define_tag!(Dd, "dd", dd, new, non_void, [TextTag]);
silex_dom::define_tag!(Del, "del", del, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Details,
    "details",
    details,
    new,
    non_void,
    [TextTag, OpenTag]
);
impl OpenAttributes for TypedElement<Details> {
    fn open<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlDetailsElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_open(v));
        self
    }
}
silex_dom::define_tag!(Dfn, "dfn", dfn, new, non_void, [TextTag]);
silex_dom::define_tag!(Dialog, "dialog", dialog, new, non_void, [TextTag, OpenTag]);
impl OpenAttributes for TypedElement<Dialog> {
    fn open<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlDialogElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_open(v));
        self
    }
}
silex_dom::define_tag!(Dir, "dir", dir, new, non_void, [TextTag]);
silex_dom::define_tag!(Div, "div", div, new, non_void, [TextTag]);
silex_dom::define_tag!(Dl, "dl", dl, new, non_void, [TextTag]);
silex_dom::define_tag!(Dt, "dt", dt, new, non_void, [TextTag]);
silex_dom::define_tag!(Em, "em", em, new, non_void, [TextTag]);
silex_dom::define_tag!(Embed, "embed", embed, new, void, [MediaTag]);
impl MediaAttributes for TypedElement<Embed> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("src", value)
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(
    Fencedframe,
    "fencedframe",
    fencedframe,
    new,
    non_void,
    [TextTag]
);
silex_dom::define_tag!(
    Fieldset,
    "fieldset",
    fieldset,
    new,
    non_void,
    [TextTag, FormTag]
);
impl FormAttributes for TypedElement<Fieldset> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.prop("value", value)
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlFieldSetElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(
    Figcaption,
    "figcaption",
    figcaption,
    new,
    non_void,
    [TextTag]
);
silex_dom::define_tag!(Figure, "figure", figure, new, non_void, [TextTag]);
silex_dom::define_tag!(Font, "font", font, new, non_void, [TextTag]);
silex_dom::define_tag!(Footer, "footer", footer, new, non_void, [TextTag]);
silex_dom::define_tag!(Form, "form", form, new, non_void, [TextTag, FormTag]);
impl FormAttributes for TypedElement<Form> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.prop("value", value)
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("disabled", value)
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(Frame, "frame", frame, new, non_void, [TextTag]);
silex_dom::define_tag!(Frameset, "frameset", frameset, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Geolocation,
    "geolocation",
    geolocation,
    new,
    non_void,
    [TextTag]
);
silex_dom::define_tag!(H1, "h1", h1, new, non_void, [TextTag]);
silex_dom::define_tag!(H2, "h2", h2, new, non_void, [TextTag]);
silex_dom::define_tag!(H3, "h3", h3, new, non_void, [TextTag]);
silex_dom::define_tag!(H4, "h4", h4, new, non_void, [TextTag]);
silex_dom::define_tag!(H5, "h5", h5, new, non_void, [TextTag]);
silex_dom::define_tag!(H6, "h6", h6, new, non_void, [TextTag]);
silex_dom::define_tag!(Head, "head", head, new, non_void, [TextTag]);
silex_dom::define_tag!(Header, "header", header, new, non_void, [TextTag]);
silex_dom::define_tag!(Hgroup, "hgroup", hgroup, new, non_void, [TextTag]);
silex_dom::define_tag!(Hr, "hr", hr, new, void, []);
silex_dom::define_tag!(Html, "html", html, new, non_void, [TextTag]);
silex_dom::define_tag!(I, "i", i, new, non_void, [TextTag]);
silex_dom::define_tag!(Iframe, "iframe", iframe, new, non_void, [TextTag, MediaTag]);
impl MediaAttributes for TypedElement<Iframe> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlIFrameElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(Img, "img", img, new, void, [MediaTag]);
impl MediaAttributes for TypedElement<Img> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlImageElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(Input, "input", input, new, void, [FormTag]);
impl FormAttributes for TypedElement<Input> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_type(v));
        self
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_checked(v));
        self
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_placeholder(v));
        self
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_read_only(v));
        self
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_required(v));
        self
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_multiple(v));
        self
    }
}
silex_dom::define_tag!(Ins, "ins", ins, new, non_void, [TextTag]);
silex_dom::define_tag!(Kbd, "kbd", kbd, new, non_void, [TextTag]);
silex_dom::define_tag!(Label, "label", label, new, non_void, [TextTag, LabelTag]);
impl LabelAttributes for TypedElement<Label> {
    fn for_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlLabelElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_html_for(v));
        self
    }
}
silex_dom::define_tag!(Legend, "legend", legend, new, non_void, [TextTag]);
silex_dom::define_tag!(Li, "li", li, new, non_void, [TextTag]);
silex_dom::define_tag!(Link, "link", link, new, void, [AnchorTag]);
impl AnchorAttributes for TypedElement<Link> {
    fn href<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlLinkElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_href(v));
        self
    }
    fn target<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlLinkElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_target(v));
        self
    }
    fn rel<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlLinkElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_rel(v));
        self
    }
    fn download<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("download", value)
    }
}
silex_dom::define_tag!(Main, "main", main, new, non_void, [TextTag]);
silex_dom::define_tag!(Map, "map", map, new, non_void, [TextTag]);
silex_dom::define_tag!(Mark, "mark", mark, new, non_void, [TextTag]);
silex_dom::define_tag!(Marquee, "marquee", marquee, new, non_void, [TextTag]);
silex_dom::define_tag!(Menu, "menu", menu, new, non_void, [TextTag]);
silex_dom::define_tag!(Meta, "meta", meta, new, void, []);
silex_dom::define_tag!(Meter, "meter", meter, new, non_void, [TextTag]);
silex_dom::define_tag!(Nav, "nav", nav, new, non_void, [TextTag]);
silex_dom::define_tag!(Nobr, "nobr", nobr, new, non_void, [TextTag]);
silex_dom::define_tag!(Noembed, "noembed", noembed, new, non_void, [TextTag]);
silex_dom::define_tag!(Noframes, "noframes", noframes, new, non_void, [TextTag]);
silex_dom::define_tag!(Noscript, "noscript", noscript, new, non_void, [TextTag]);
silex_dom::define_tag!(Object, "object", object, new, non_void, [TextTag, MediaTag]);
impl MediaAttributes for TypedElement<Object> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("src", value)
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(Ol, "ol", ol, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Optgroup,
    "optgroup",
    optgroup,
    new,
    non_void,
    [TextTag, FormTag]
);
impl FormAttributes for TypedElement<Optgroup> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.prop("value", value)
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlOptGroupElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(
    OptionTag,
    "option",
    option_tag,
    new,
    non_void,
    [TextTag, FormTag]
);
impl FormAttributes for TypedElement<OptionTag> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlOptionElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlOptionElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlOptionElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_selected(v));
        self
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(Output, "output", output, new, non_void, [TextTag, FormTag]);
impl FormAttributes for TypedElement<Output> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlOutputElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("disabled", value)
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("required", value)
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(P, "p", p, new, non_void, [TextTag]);
silex_dom::define_tag!(Param, "param", param, new, void, []);
silex_dom::define_tag!(Picture, "picture", picture, new, non_void, [TextTag]);
silex_dom::define_tag!(Plaintext, "plaintext", plaintext, new, non_void, [TextTag]);
silex_dom::define_tag!(Pre, "pre", pre, new, non_void, [TextTag]);
silex_dom::define_tag!(Progress, "progress", progress, new, non_void, [TextTag]);
silex_dom::define_tag!(Q, "q", q, new, non_void, [TextTag]);
silex_dom::define_tag!(Rb, "rb", rb, new, non_void, [TextTag]);
silex_dom::define_tag!(Rp, "rp", rp, new, non_void, [TextTag]);
silex_dom::define_tag!(Rt, "rt", rt, new, non_void, [TextTag]);
silex_dom::define_tag!(Rtc, "rtc", rtc, new, non_void, [TextTag]);
silex_dom::define_tag!(Ruby, "ruby", ruby, new, non_void, [TextTag]);
silex_dom::define_tag!(S, "s", s, new, non_void, [TextTag]);
silex_dom::define_tag!(Samp, "samp", samp, new, non_void, [TextTag]);
silex_dom::define_tag!(Script, "script", script, new, non_void, [TextTag]);
silex_dom::define_tag!(Search, "search", search, new, non_void, [TextTag]);
silex_dom::define_tag!(Section, "section", section, new, non_void, [TextTag]);
silex_dom::define_tag!(Select, "select", select, new, non_void, [TextTag, FormTag]);
impl FormAttributes for TypedElement<Select> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlSelectElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlSelectElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("placeholder", value)
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("readOnly", value)
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlSelectElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_required(v));
        self
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlSelectElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_multiple(v));
        self
    }
}
silex_dom::define_tag!(
    Selectedcontent,
    "selectedcontent",
    selectedcontent,
    new,
    non_void,
    [TextTag]
);
silex_dom::define_tag!(Slot, "slot", slot, new, non_void, [TextTag]);
silex_dom::define_tag!(Small, "small", small, new, non_void, [TextTag]);
silex_dom::define_tag!(Source, "source", source, new, void, [MediaTag]);
impl MediaAttributes for TypedElement<Source> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlSourceElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(Span, "span", span, new, non_void, [TextTag]);
silex_dom::define_tag!(Strike, "strike", strike, new, non_void, [TextTag]);
silex_dom::define_tag!(Strong, "strong", strong, new, non_void, [TextTag]);
silex_dom::define_tag!(Style, "style", style, new, non_void, [TextTag]);
silex_dom::define_tag!(Sub, "sub", sub, new, non_void, [TextTag]);
silex_dom::define_tag!(Summary, "summary", summary, new, non_void, [TextTag]);
silex_dom::define_tag!(Sup, "sup", sup, new, non_void, [TextTag]);
silex_dom::define_tag!(Table, "table", table, new, non_void, [TextTag]);
silex_dom::define_tag!(Tbody, "tbody", tbody, new, non_void, [TextTag]);
silex_dom::define_tag!(Td, "td", td, new, non_void, [TextTag, TableCellTag]);
impl TableCellAttributes for TypedElement<Td> {
    fn colspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| {
            if let Ok(n) = v.parse::<u32>() {
                el.set_col_span(n);
            } else {
                let _ = el.set_attribute("colspan", v);
            }
        });
        self
    }
    fn rowspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| {
            if let Ok(n) = v.parse::<u32>() {
                el.set_row_span(n);
            } else {
                let _ = el.set_attribute("rowspan", v);
            }
        });
        self
    }
    fn headers<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_headers(v));
        self
    }
}
silex_dom::define_tag!(Template, "template", template, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Textarea,
    "textarea",
    textarea,
    new,
    non_void,
    [TextTag, FormTag]
);
impl FormAttributes for TypedElement<Textarea> {
    fn type_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        self.attr("type", value)
    }
    fn value<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_value(v));
        self
    }
    fn checked<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("checked", value)
    }
    fn disabled<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_disabled(v));
        self
    }
    fn placeholder<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_placeholder(v));
        self
    }
    fn readonly<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_read_only(v));
        self
    }
    fn required<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_required(v));
        self
    }
    fn selected<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("selected", value)
    }
    fn multiple<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        self.prop("multiple", value)
    }
}
silex_dom::define_tag!(Tfoot, "tfoot", tfoot, new, non_void, [TextTag]);
silex_dom::define_tag!(
    Th,
    "th",
    th,
    new,
    non_void,
    [TextTag, TableCellTag, TableHeaderTag]
);
impl TableCellAttributes for TypedElement<Th> {
    fn colspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| {
            if let Ok(n) = v.parse::<u32>() {
                el.set_col_span(n);
            } else {
                let _ = el.set_attribute("colspan", v);
            }
        });
        self
    }
    fn rowspan<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| {
            if let Ok(n) = v.parse::<u32>() {
                el.set_row_span(n);
            } else {
                let _ = el.set_attribute("rowspan", v);
            }
        });
        self
    }
    fn headers<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value
            .into_storable()
            .apply_string(move |v| el.set_headers(v));
        self
    }
}
impl TableHeaderAttributes for TypedElement<Th> {
    fn scope<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_scope(v));
        self
    }
    fn abbr<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTableCellElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_abbr(v));
        self
    }
}
silex_dom::define_tag!(Thead, "thead", thead, new, non_void, [TextTag]);
silex_dom::define_tag!(Time, "time", time, new, non_void, [TextTag]);
silex_dom::define_tag!(Title, "title", title, new, non_void, [TextTag]);
silex_dom::define_tag!(Tr, "tr", tr, new, non_void, [TextTag]);
silex_dom::define_tag!(Track, "track", track, new, void, [MediaTag]);
impl MediaAttributes for TypedElement<Track> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlTrackElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
}
silex_dom::define_tag!(Tt, "tt", tt, new, non_void, [TextTag]);
silex_dom::define_tag!(U, "u", u, new, non_void, [TextTag]);
silex_dom::define_tag!(Ul, "ul", ul, new, non_void, [TextTag]);
silex_dom::define_tag!(Var, "var", var, new, non_void, [TextTag]);
silex_dom::define_tag!(Video, "video", video, new, non_void, [TextTag, MediaTag]);
impl MediaAttributes for TypedElement<Video> {
    fn src<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyStringAttribute,
    {
        let el: web_sys::HtmlVideoElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyStringAttribute;
        value.into_storable().apply_string(move |v| el.set_src(v));
        self
    }
    // width/height passed to attr for flexibility (%, px, auto)
    fn autoplay<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlVideoElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_autoplay(v));
        self
    }
    fn controls<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlVideoElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value
            .into_storable()
            .apply_bool(move |v| el.set_controls(v));
        self
    }
    fn loop_<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlVideoElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_loop(v));
        self
    }
    fn muted<V>(self, value: V) -> Self
    where
        V: IntoStorable,
        V::Stored: silex_dom::ApplyBoolAttribute,
    {
        let el: web_sys::HtmlVideoElement = self.element.dom_element.clone().unchecked_into();
        use silex_dom::ApplyBoolAttribute;
        value.into_storable().apply_bool(move |v| el.set_muted(v));
        self
    }
}
silex_dom::define_tag!(Wbr, "wbr", wbr, new, void, []);
silex_dom::define_tag!(Xmp, "xmp", xmp, new, non_void, [TextTag]);

// --- Macros ---
#[macro_export]
macro_rules! a {
    () => { $crate::html::a(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::a(($($child),+)) };
}
#[macro_export]
macro_rules! abbr {
    () => { $crate::html::abbr(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::abbr(($($child),+)) };
}
#[macro_export]
macro_rules! acronym {
    () => { $crate::html::acronym(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::acronym(($($child),+)) };
}
#[macro_export]
macro_rules! address {
    () => { $crate::html::address(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::address(($($child),+)) };
}
#[macro_export]
macro_rules! article {
    () => { $crate::html::article(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::article(($($child),+)) };
}
#[macro_export]
macro_rules! aside {
    () => { $crate::html::aside(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::aside(($($child),+)) };
}
#[macro_export]
macro_rules! audio {
    () => { $crate::html::audio(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::audio(($($child),+)) };
}
#[macro_export]
macro_rules! b {
    () => { $crate::html::b(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::b(($($child),+)) };
}
#[macro_export]
macro_rules! bdi {
    () => { $crate::html::bdi(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdi(($($child),+)) };
}
#[macro_export]
macro_rules! bdo {
    () => { $crate::html::bdo(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdo(($($child),+)) };
}
#[macro_export]
macro_rules! big {
    () => { $crate::html::big(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::big(($($child),+)) };
}
#[macro_export]
macro_rules! blockquote {
    () => { $crate::html::blockquote(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::blockquote(($($child),+)) };
}
#[macro_export]
macro_rules! body {
    () => { $crate::html::body(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::body(($($child),+)) };
}
#[macro_export]
macro_rules! button {
    () => { $crate::html::button(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::button(($($child),+)) };
}
#[macro_export]
macro_rules! canvas {
    () => { $crate::html::canvas(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::canvas(($($child),+)) };
}
#[macro_export]
macro_rules! caption {
    () => { $crate::html::caption(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::caption(($($child),+)) };
}
#[macro_export]
macro_rules! center {
    () => { $crate::html::center(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::center(($($child),+)) };
}
#[macro_export]
macro_rules! cite {
    () => { $crate::html::cite(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::cite(($($child),+)) };
}
#[macro_export]
macro_rules! code {
    () => { $crate::html::code(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::code(($($child),+)) };
}
#[macro_export]
macro_rules! colgroup {
    () => { $crate::html::colgroup(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::colgroup(($($child),+)) };
}
#[macro_export]
macro_rules! data_tag {
    () => { $crate::html::data_tag(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::data_tag(($($child),+)) };
}
#[macro_export]
macro_rules! datalist {
    () => { $crate::html::datalist(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::datalist(($($child),+)) };
}
#[macro_export]
macro_rules! dd {
    () => { $crate::html::dd(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dd(($($child),+)) };
}
#[macro_export]
macro_rules! del {
    () => { $crate::html::del(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::del(($($child),+)) };
}
#[macro_export]
macro_rules! details {
    () => { $crate::html::details(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::details(($($child),+)) };
}
#[macro_export]
macro_rules! dfn {
    () => { $crate::html::dfn(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dfn(($($child),+)) };
}
#[macro_export]
macro_rules! dialog {
    () => { $crate::html::dialog(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dialog(($($child),+)) };
}
#[macro_export]
macro_rules! dir {
    () => { $crate::html::dir(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dir(($($child),+)) };
}
#[macro_export]
macro_rules! div {
    () => { $crate::html::div(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::div(($($child),+)) };
}
#[macro_export]
macro_rules! dl {
    () => { $crate::html::dl(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dl(($($child),+)) };
}
#[macro_export]
macro_rules! dt {
    () => { $crate::html::dt(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::dt(($($child),+)) };
}
#[macro_export]
macro_rules! em {
    () => { $crate::html::em(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::em(($($child),+)) };
}
#[macro_export]
macro_rules! fencedframe {
    () => { $crate::html::fencedframe(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::fencedframe(($($child),+)) };
}
#[macro_export]
macro_rules! fieldset {
    () => { $crate::html::fieldset(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::fieldset(($($child),+)) };
}
#[macro_export]
macro_rules! figcaption {
    () => { $crate::html::figcaption(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::figcaption(($($child),+)) };
}
#[macro_export]
macro_rules! figure {
    () => { $crate::html::figure(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::figure(($($child),+)) };
}
#[macro_export]
macro_rules! font {
    () => { $crate::html::font(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::font(($($child),+)) };
}
#[macro_export]
macro_rules! footer {
    () => { $crate::html::footer(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::footer(($($child),+)) };
}
#[macro_export]
macro_rules! form {
    () => { $crate::html::form(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::form(($($child),+)) };
}
#[macro_export]
macro_rules! frame {
    () => { $crate::html::frame(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::frame(($($child),+)) };
}
#[macro_export]
macro_rules! frameset {
    () => { $crate::html::frameset(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::frameset(($($child),+)) };
}
#[macro_export]
macro_rules! geolocation {
    () => { $crate::html::geolocation(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::geolocation(($($child),+)) };
}
#[macro_export]
macro_rules! h1 {
    () => { $crate::html::h1(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h1(($($child),+)) };
}
#[macro_export]
macro_rules! h2 {
    () => { $crate::html::h2(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h2(($($child),+)) };
}
#[macro_export]
macro_rules! h3 {
    () => { $crate::html::h3(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h3(($($child),+)) };
}
#[macro_export]
macro_rules! h4 {
    () => { $crate::html::h4(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h4(($($child),+)) };
}
#[macro_export]
macro_rules! h5 {
    () => { $crate::html::h5(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h5(($($child),+)) };
}
#[macro_export]
macro_rules! h6 {
    () => { $crate::html::h6(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::h6(($($child),+)) };
}
#[macro_export]
macro_rules! head {
    () => { $crate::html::head(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::head(($($child),+)) };
}
#[macro_export]
macro_rules! header {
    () => { $crate::html::header(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::header(($($child),+)) };
}
#[macro_export]
macro_rules! hgroup {
    () => { $crate::html::hgroup(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::hgroup(($($child),+)) };
}
#[macro_export]
macro_rules! html {
    () => { $crate::html::html(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::html(($($child),+)) };
}
#[macro_export]
macro_rules! i {
    () => { $crate::html::i(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::i(($($child),+)) };
}
#[macro_export]
macro_rules! iframe {
    () => { $crate::html::iframe(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::iframe(($($child),+)) };
}
#[macro_export]
macro_rules! ins {
    () => { $crate::html::ins(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::ins(($($child),+)) };
}
#[macro_export]
macro_rules! kbd {
    () => { $crate::html::kbd(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::kbd(($($child),+)) };
}
#[macro_export]
macro_rules! label {
    () => { $crate::html::label(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::label(($($child),+)) };
}
#[macro_export]
macro_rules! legend {
    () => { $crate::html::legend(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::legend(($($child),+)) };
}
#[macro_export]
macro_rules! li {
    () => { $crate::html::li(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::li(($($child),+)) };
}
#[macro_export]
macro_rules! main {
    () => { $crate::html::main(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::main(($($child),+)) };
}
#[macro_export]
macro_rules! map {
    () => { $crate::html::map(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::map(($($child),+)) };
}
#[macro_export]
macro_rules! mark {
    () => { $crate::html::mark(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::mark(($($child),+)) };
}
#[macro_export]
macro_rules! marquee {
    () => { $crate::html::marquee(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::marquee(($($child),+)) };
}
#[macro_export]
macro_rules! menu {
    () => { $crate::html::menu(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::menu(($($child),+)) };
}
#[macro_export]
macro_rules! meter {
    () => { $crate::html::meter(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::meter(($($child),+)) };
}
#[macro_export]
macro_rules! nav {
    () => { $crate::html::nav(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::nav(($($child),+)) };
}
#[macro_export]
macro_rules! nobr {
    () => { $crate::html::nobr(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::nobr(($($child),+)) };
}
#[macro_export]
macro_rules! noembed {
    () => { $crate::html::noembed(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::noembed(($($child),+)) };
}
#[macro_export]
macro_rules! noframes {
    () => { $crate::html::noframes(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::noframes(($($child),+)) };
}
#[macro_export]
macro_rules! noscript {
    () => { $crate::html::noscript(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::noscript(($($child),+)) };
}
#[macro_export]
macro_rules! object {
    () => { $crate::html::object(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::object(($($child),+)) };
}
#[macro_export]
macro_rules! ol {
    () => { $crate::html::ol(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::ol(($($child),+)) };
}
#[macro_export]
macro_rules! optgroup {
    () => { $crate::html::optgroup(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::optgroup(($($child),+)) };
}
#[macro_export]
macro_rules! option_tag {
    () => { $crate::html::option_tag(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::option_tag(($($child),+)) };
}
#[macro_export]
macro_rules! output {
    () => { $crate::html::output(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::output(($($child),+)) };
}
#[macro_export]
macro_rules! p {
    () => { $crate::html::p(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::p(($($child),+)) };
}
#[macro_export]
macro_rules! picture {
    () => { $crate::html::picture(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::picture(($($child),+)) };
}
#[macro_export]
macro_rules! plaintext {
    () => { $crate::html::plaintext(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::plaintext(($($child),+)) };
}
#[macro_export]
macro_rules! pre {
    () => { $crate::html::pre(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::pre(($($child),+)) };
}
#[macro_export]
macro_rules! progress {
    () => { $crate::html::progress(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::progress(($($child),+)) };
}
#[macro_export]
macro_rules! q {
    () => { $crate::html::q(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::q(($($child),+)) };
}
#[macro_export]
macro_rules! rb {
    () => { $crate::html::rb(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::rb(($($child),+)) };
}
#[macro_export]
macro_rules! rp {
    () => { $crate::html::rp(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::rp(($($child),+)) };
}
#[macro_export]
macro_rules! rt {
    () => { $crate::html::rt(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::rt(($($child),+)) };
}
#[macro_export]
macro_rules! rtc {
    () => { $crate::html::rtc(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::rtc(($($child),+)) };
}
#[macro_export]
macro_rules! ruby {
    () => { $crate::html::ruby(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::ruby(($($child),+)) };
}
#[macro_export]
macro_rules! s {
    () => { $crate::html::s(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::s(($($child),+)) };
}
#[macro_export]
macro_rules! samp {
    () => { $crate::html::samp(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::samp(($($child),+)) };
}
#[macro_export]
macro_rules! script {
    () => { $crate::html::script(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::script(($($child),+)) };
}
#[macro_export]
macro_rules! search {
    () => { $crate::html::search(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::search(($($child),+)) };
}
#[macro_export]
macro_rules! section {
    () => { $crate::html::section(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::section(($($child),+)) };
}
#[macro_export]
macro_rules! select {
    () => { $crate::html::select(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::select(($($child),+)) };
}
#[macro_export]
macro_rules! selectedcontent {
    () => { $crate::html::selectedcontent(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::selectedcontent(($($child),+)) };
}
#[macro_export]
macro_rules! slot {
    () => { $crate::html::slot(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::slot(($($child),+)) };
}
#[macro_export]
macro_rules! small {
    () => { $crate::html::small(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::small(($($child),+)) };
}
#[macro_export]
macro_rules! span {
    () => { $crate::html::span(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::span(($($child),+)) };
}
#[macro_export]
macro_rules! strike {
    () => { $crate::html::strike(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::strike(($($child),+)) };
}
#[macro_export]
macro_rules! strong {
    () => { $crate::html::strong(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::strong(($($child),+)) };
}
#[macro_export]
macro_rules! style {
    () => { $crate::html::style(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::style(($($child),+)) };
}
#[macro_export]
macro_rules! sub {
    () => { $crate::html::sub(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::sub(($($child),+)) };
}
#[macro_export]
macro_rules! summary {
    () => { $crate::html::summary(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::summary(($($child),+)) };
}
#[macro_export]
macro_rules! sup {
    () => { $crate::html::sup(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::sup(($($child),+)) };
}
#[macro_export]
macro_rules! table {
    () => { $crate::html::table(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::table(($($child),+)) };
}
#[macro_export]
macro_rules! tbody {
    () => { $crate::html::tbody(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::tbody(($($child),+)) };
}
#[macro_export]
macro_rules! td {
    () => { $crate::html::td(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::td(($($child),+)) };
}
#[macro_export]
macro_rules! template {
    () => { $crate::html::template(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::template(($($child),+)) };
}
#[macro_export]
macro_rules! textarea {
    () => { $crate::html::textarea(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::textarea(($($child),+)) };
}
#[macro_export]
macro_rules! tfoot {
    () => { $crate::html::tfoot(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::tfoot(($($child),+)) };
}
#[macro_export]
macro_rules! th {
    () => { $crate::html::th(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::th(($($child),+)) };
}
#[macro_export]
macro_rules! thead {
    () => { $crate::html::thead(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::thead(($($child),+)) };
}
#[macro_export]
macro_rules! time {
    () => { $crate::html::time(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::time(($($child),+)) };
}
#[macro_export]
macro_rules! title {
    () => { $crate::html::title(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::title(($($child),+)) };
}
#[macro_export]
macro_rules! tr {
    () => { $crate::html::tr(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::tr(($($child),+)) };
}
#[macro_export]
macro_rules! tt {
    () => { $crate::html::tt(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::tt(($($child),+)) };
}
#[macro_export]
macro_rules! u {
    () => { $crate::html::u(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::u(($($child),+)) };
}
#[macro_export]
macro_rules! ul {
    () => { $crate::html::ul(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::ul(($($child),+)) };
}
#[macro_export]
macro_rules! var {
    () => { $crate::html::var(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::var(($($child),+)) };
}
#[macro_export]
macro_rules! video {
    () => { $crate::html::video(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::video(($($child),+)) };
}
#[macro_export]
macro_rules! xmp {
    () => { $crate::html::xmp(()) };
    ($($child:expr),+ $(,)?) => { $crate::html::xmp(($($child),+)) };
}
