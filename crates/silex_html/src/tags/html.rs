// --- Tags ---
#[rustfmt::skip] silex_dom::define_tag!(A, web_sys::HtmlAnchorElement, "a", a, new, non_void, [TextTag, AnchorTag]);
#[rustfmt::skip] silex_dom::define_tag!(Abbr, web_sys::HtmlElement, "abbr", abbr, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Acronym, web_sys::HtmlElement, "acronym", acronym, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Address, web_sys::HtmlElement, "address", address, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Area, web_sys::HtmlElement, "area", area, new, void, [AnchorTag]);
#[rustfmt::skip] silex_dom::define_tag!(Article, web_sys::HtmlElement, "article", article, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Aside, web_sys::HtmlElement, "aside", aside, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Audio, web_sys::HtmlAudioElement, "audio", audio, new, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(B, web_sys::HtmlElement, "b", b, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Base, web_sys::HtmlElement, "base", base, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Bdi, web_sys::HtmlElement, "bdi", bdi, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Bdo, web_sys::HtmlElement, "bdo", bdo, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Big, web_sys::HtmlElement, "big", big, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Blockquote, web_sys::HtmlElement, "blockquote", blockquote, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Body, web_sys::HtmlElement, "body", body, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Br, web_sys::HtmlElement, "br", br, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Button, web_sys::HtmlButtonElement, "button", button, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Canvas, web_sys::HtmlCanvasElement, "canvas", canvas, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Caption, web_sys::HtmlElement, "caption", caption, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Center, web_sys::HtmlElement, "center", center, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Cite, web_sys::HtmlElement, "cite", cite, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Code, web_sys::HtmlElement, "code", code, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Col, web_sys::HtmlElement, "col", col, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Colgroup, web_sys::HtmlElement, "colgroup", colgroup, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(DataTag, web_sys::HtmlElement, "data", data_tag, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Datalist, web_sys::HtmlElement, "datalist", datalist, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dd, web_sys::HtmlElement, "dd", dd, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Del, web_sys::HtmlElement, "del", del, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Details, web_sys::HtmlDetailsElement, "details", details, new, non_void, [TextTag, OpenTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dfn, web_sys::HtmlElement, "dfn", dfn, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dialog, web_sys::HtmlDialogElement, "dialog", dialog, new, non_void, [TextTag, OpenTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dir, web_sys::HtmlElement, "dir", dir, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Div, web_sys::HtmlElement, "div", div, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dl, web_sys::HtmlElement, "dl", dl, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Dt, web_sys::HtmlElement, "dt", dt, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Em, web_sys::HtmlElement, "em", em, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Embed, web_sys::HtmlElement, "embed", embed, new, void, [MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Fencedframe, web_sys::HtmlElement, "fencedframe", fencedframe, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Fieldset, web_sys::HtmlElement, "fieldset", fieldset, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Figcaption, web_sys::HtmlElement, "figcaption", figcaption, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Figure, web_sys::HtmlElement, "figure", figure, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Font, web_sys::HtmlElement, "font", font, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Footer, web_sys::HtmlElement, "footer", footer, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Form, web_sys::HtmlFormElement, "form", form, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Frame, web_sys::HtmlElement, "frame", frame, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Frameset, web_sys::HtmlElement, "frameset", frameset, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Geolocation, web_sys::HtmlElement, "geolocation", geolocation, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H1, web_sys::HtmlElement, "h1", h1, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H2, web_sys::HtmlElement, "h2", h2, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H3, web_sys::HtmlElement, "h3", h3, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H4, web_sys::HtmlElement, "h4", h4, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H5, web_sys::HtmlElement, "h5", h5, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(H6, web_sys::HtmlElement, "h6", h6, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Head, web_sys::HtmlElement, "head", head, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Header, web_sys::HtmlElement, "header", header, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Hgroup, web_sys::HtmlElement, "hgroup", hgroup, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Hr, web_sys::HtmlElement, "hr", hr, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Html, web_sys::HtmlElement, "html", html, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(I, web_sys::HtmlElement, "i", i, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Iframe, web_sys::HtmlIFrameElement, "iframe", iframe, new, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Img, web_sys::HtmlImageElement, "img", img, new, void, [MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Input, web_sys::HtmlInputElement, "input", input, new, void, [FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Ins, web_sys::HtmlElement, "ins", ins, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Kbd, web_sys::HtmlElement, "kbd", kbd, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Label, web_sys::HtmlElement, "label", label, new, non_void, [TextTag, LabelTag]);
#[rustfmt::skip] silex_dom::define_tag!(Legend, web_sys::HtmlElement, "legend", legend, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Li, web_sys::HtmlElement, "li", li, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Link, web_sys::HtmlElement, "link", link, new, void, [AnchorTag]);
#[rustfmt::skip] silex_dom::define_tag!(Main, web_sys::HtmlElement, "main", main, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Map, web_sys::HtmlElement, "map", map, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Mark, web_sys::HtmlElement, "mark", mark, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Marquee, web_sys::HtmlElement, "marquee", marquee, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Menu, web_sys::HtmlElement, "menu", menu, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Meta, web_sys::HtmlElement, "meta", meta, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Meter, web_sys::HtmlElement, "meter", meter, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Nav, web_sys::HtmlElement, "nav", nav, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Nobr, web_sys::HtmlElement, "nobr", nobr, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Noembed, web_sys::HtmlElement, "noembed", noembed, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Noframes, web_sys::HtmlElement, "noframes", noframes, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Noscript, web_sys::HtmlElement, "noscript", noscript, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Object, web_sys::HtmlElement, "object", object, new, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Ol, web_sys::HtmlElement, "ol", ol, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Optgroup, web_sys::HtmlOptGroupElement, "optgroup", optgroup, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(OptionTag, web_sys::HtmlOptionElement, "option", option_tag, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Output, web_sys::HtmlElement, "output", output, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(P, web_sys::HtmlElement, "p", p, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Param, web_sys::HtmlElement, "param", param, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Picture, web_sys::HtmlElement, "picture", picture, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Plaintext, web_sys::HtmlElement, "plaintext", plaintext, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Pre, web_sys::HtmlElement, "pre", pre, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Progress, web_sys::HtmlElement, "progress", progress, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Q, web_sys::HtmlElement, "q", q, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Rb, web_sys::HtmlElement, "rb", rb, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Rp, web_sys::HtmlElement, "rp", rp, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Rt, web_sys::HtmlElement, "rt", rt, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Rtc, web_sys::HtmlElement, "rtc", rtc, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Ruby, web_sys::HtmlElement, "ruby", ruby, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(S, web_sys::HtmlElement, "s", s, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Samp, web_sys::HtmlElement, "samp", samp, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Script, web_sys::HtmlElement, "script", script, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Search, web_sys::HtmlElement, "search", search, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Section, web_sys::HtmlElement, "section", section, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Select, web_sys::HtmlSelectElement, "select", select, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Selectedcontent, web_sys::HtmlElement, "selectedcontent", selectedcontent, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Slot, web_sys::HtmlElement, "slot", slot, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Small, web_sys::HtmlElement, "small", small, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Source, web_sys::HtmlElement, "source", source, new, void, [MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Span, web_sys::HtmlElement, "span", span, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Strike, web_sys::HtmlElement, "strike", strike, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Strong, web_sys::HtmlElement, "strong", strong, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Style, web_sys::HtmlElement, "style", style, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Sub, web_sys::HtmlElement, "sub", sub, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Summary, web_sys::HtmlElement, "summary", summary, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Sup, web_sys::HtmlElement, "sup", sup, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Table, web_sys::HtmlElement, "table", table, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Tbody, web_sys::HtmlElement, "tbody", tbody, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Td, web_sys::HtmlElement, "td", td, new, non_void, [TextTag, TableCellTag]);
#[rustfmt::skip] silex_dom::define_tag!(Template, web_sys::HtmlElement, "template", template, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Textarea, web_sys::HtmlTextAreaElement, "textarea", textarea, new, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_dom::define_tag!(Tfoot, web_sys::HtmlElement, "tfoot", tfoot, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Th, web_sys::HtmlElement, "th", th, new, non_void, [TextTag, TableCellTag, TableHeaderTag]);
#[rustfmt::skip] silex_dom::define_tag!(Thead, web_sys::HtmlElement, "thead", thead, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Time, web_sys::HtmlElement, "time", time, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Title, web_sys::HtmlElement, "title", title, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Tr, web_sys::HtmlElement, "tr", tr, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Track, web_sys::HtmlElement, "track", track, new, void, [MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Tt, web_sys::HtmlElement, "tt", tt, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(U, web_sys::HtmlElement, "u", u, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Ul, web_sys::HtmlElement, "ul", ul, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Var, web_sys::HtmlElement, "var", var, new, non_void, [TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Video, web_sys::HtmlVideoElement, "video", video, new, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_dom::define_tag!(Wbr, web_sys::HtmlElement, "wbr", wbr, new, void, []);
#[rustfmt::skip] silex_dom::define_tag!(Xmp, web_sys::HtmlElement, "xmp", xmp, new, non_void, [TextTag]);

// --- Macros ---
#[rustfmt::skip] #[macro_export] macro_rules! a {
    () => { $crate::html::a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::a($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! abbr {
    () => { $crate::html::abbr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::abbr($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! acronym {
    () => { $crate::html::acronym($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::acronym($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! address {
    () => { $crate::html::address($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::address($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! article {
    () => { $crate::html::article($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::article($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! aside {
    () => { $crate::html::aside($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::aside($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! audio {
    () => { $crate::html::audio($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::audio($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! b {
    () => { $crate::html::b($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::b($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! bdi {
    () => { $crate::html::bdi($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdi($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! bdo {
    () => { $crate::html::bdo($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdo($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! big {
    () => { $crate::html::big($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::big($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! blockquote {
    () => { $crate::html::blockquote($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::blockquote($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! body {
    () => { $crate::html::body($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::body($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! button {
    () => { $crate::html::button($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::button($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! canvas {
    () => { $crate::html::canvas($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::canvas($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! caption {
    () => { $crate::html::caption($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::caption($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! center {
    () => { $crate::html::center($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::center($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! cite {
    () => { $crate::html::cite($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::cite($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! code {
    () => { $crate::html::code($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::code($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! colgroup {
    () => { $crate::html::colgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::colgroup($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! data_tag {
    () => { $crate::html::data_tag($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::data_tag($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! datalist {
    () => { $crate::html::datalist($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::datalist($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dd {
    () => { $crate::html::dd($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dd($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! del {
    () => { $crate::html::del($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::del($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! details {
    () => { $crate::html::details($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::details($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dfn {
    () => { $crate::html::dfn($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dfn($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dialog {
    () => { $crate::html::dialog($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dialog($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dir {
    () => { $crate::html::dir($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dir($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! div {
    () => { $crate::html::div($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::div($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dl {
    () => { $crate::html::dl($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dl($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dt {
    () => { $crate::html::dt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dt($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! em {
    () => { $crate::html::em($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::em($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fencedframe {
    () => { $crate::html::fencedframe($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::fencedframe($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fieldset {
    () => { $crate::html::fieldset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::fieldset($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! figcaption {
    () => { $crate::html::figcaption($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::figcaption($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! figure {
    () => { $crate::html::figure($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::figure($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! font {
    () => { $crate::html::font($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::font($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! footer {
    () => { $crate::html::footer($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::footer($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! form {
    () => { $crate::html::form($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::form($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! frame {
    () => { $crate::html::frame($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::frame($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! frameset {
    () => { $crate::html::frameset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::frameset($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! geolocation {
    () => { $crate::html::geolocation($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::geolocation($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h1 {
    () => { $crate::html::h1($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h1($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h2 {
    () => { $crate::html::h2($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h2($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h3 {
    () => { $crate::html::h3($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h3($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h4 {
    () => { $crate::html::h4($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h4($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h5 {
    () => { $crate::html::h5($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h5($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h6 {
    () => { $crate::html::h6($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h6($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! head {
    () => { $crate::html::head($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::head($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! header {
    () => { $crate::html::header($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::header($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! hgroup {
    () => { $crate::html::hgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::hgroup($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! html {
    () => { $crate::html::html($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::html($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! i {
    () => { $crate::html::i($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::i($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! iframe {
    () => { $crate::html::iframe($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::iframe($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ins {
    () => { $crate::html::ins($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ins($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! kbd {
    () => { $crate::html::kbd($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::kbd($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! label {
    () => { $crate::html::label($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::label($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! legend {
    () => { $crate::html::legend($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::legend($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! li {
    () => { $crate::html::li($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::li($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! main {
    () => { $crate::html::main($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::main($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! map {
    () => { $crate::html::map($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::map($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mark {
    () => { $crate::html::mark($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::mark($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! marquee {
    () => { $crate::html::marquee($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::marquee($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! menu {
    () => { $crate::html::menu($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::menu($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! meter {
    () => { $crate::html::meter($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::meter($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! nav {
    () => { $crate::html::nav($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::nav($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! nobr {
    () => { $crate::html::nobr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::nobr($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noembed {
    () => { $crate::html::noembed($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noembed($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noframes {
    () => { $crate::html::noframes($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noframes($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noscript {
    () => { $crate::html::noscript($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noscript($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! object {
    () => { $crate::html::object($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::object($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ol {
    () => { $crate::html::ol($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ol($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! optgroup {
    () => { $crate::html::optgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::optgroup($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! option_tag {
    () => { $crate::html::option_tag($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::option_tag($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! output {
    () => { $crate::html::output($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::output($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! p {
    () => { $crate::html::p($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::p($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! picture {
    () => { $crate::html::picture($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::picture($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! plaintext {
    () => { $crate::html::plaintext($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::plaintext($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! pre {
    () => { $crate::html::pre($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::pre($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! progress {
    () => { $crate::html::progress($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::progress($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! q {
    () => { $crate::html::q($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::q($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rb {
    () => { $crate::html::rb($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rb($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rp {
    () => { $crate::html::rp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rp($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rt {
    () => { $crate::html::rt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rt($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rtc {
    () => { $crate::html::rtc($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rtc($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ruby {
    () => { $crate::html::ruby($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ruby($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! s {
    () => { $crate::html::s($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::s($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! samp {
    () => { $crate::html::samp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::samp($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! script {
    () => { $crate::html::script($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::script($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! search {
    () => { $crate::html::search($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::search($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! section {
    () => { $crate::html::section($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::section($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! select {
    () => { $crate::html::select($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::select($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! selectedcontent {
    () => { $crate::html::selectedcontent($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::selectedcontent($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! slot {
    () => { $crate::html::slot($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::slot($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! small {
    () => { $crate::html::small($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::small($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! span {
    () => { $crate::html::span($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::span($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! strike {
    () => { $crate::html::strike($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::strike($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! strong {
    () => { $crate::html::strong($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::strong($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! style {
    () => { $crate::html::style($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::style($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! sub {
    () => { $crate::html::sub($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::sub($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! summary {
    () => { $crate::html::summary($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::summary($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! sup {
    () => { $crate::html::sup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::sup($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! table {
    () => { $crate::html::table($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::table($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tbody {
    () => { $crate::html::tbody($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tbody($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! td {
    () => { $crate::html::td($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::td($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! template {
    () => { $crate::html::template($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::template($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! textarea {
    () => { $crate::html::textarea($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::textarea($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tfoot {
    () => { $crate::html::tfoot($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tfoot($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! th {
    () => { $crate::html::th($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::th($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! thead {
    () => { $crate::html::thead($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::thead($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! time {
    () => { $crate::html::time($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::time($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! title {
    () => { $crate::html::title($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::title($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tr {
    () => { $crate::html::tr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tr($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tt {
    () => { $crate::html::tt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tt($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! u {
    () => { $crate::html::u($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::u($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ul {
    () => { $crate::html::ul($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ul($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! var {
    () => { $crate::html::var($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::var($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! video {
    () => { $crate::html::video($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::video($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! xmp {
    () => { $crate::html::xmp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::xmp($crate::view_chain!($($child),+)) };
}
