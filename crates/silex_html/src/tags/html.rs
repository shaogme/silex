// --- Tags ---
#[rustfmt::skip] silex_view::define_tag!(A, "a", html, a, non_void, [TextTag, AnchorTag]);
#[rustfmt::skip] silex_view::define_tag!(Abbr, "abbr", html, abbr, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Acronym, "acronym", html, acronym, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Address, "address", html, address, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Area, "area", html, area, void, [AnchorTag]);
#[rustfmt::skip] silex_view::define_tag!(Article, "article", html, article, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Aside, "aside", html, aside, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Audio, "audio", html, audio, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(B, "b", html, b, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Base, "base", html, base, void, []);
#[rustfmt::skip] silex_view::define_tag!(Bdi, "bdi", html, bdi, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Bdo, "bdo", html, bdo, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Big, "big", html, big, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Blockquote, "blockquote", html, blockquote, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Body, "body", html, body, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Br, "br", html, br, void, []);
#[rustfmt::skip] silex_view::define_tag!(Button, "button", html, button, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Canvas, "canvas", html, canvas, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Caption, "caption", html, caption, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Center, "center", html, center, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Cite, "cite", html, cite, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Code, "code", html, code, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Col, "col", html, col, void, []);
#[rustfmt::skip] silex_view::define_tag!(Colgroup, "colgroup", html, colgroup, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(DataTag, "data", html, data_tag, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Datalist, "datalist", html, datalist, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Dd, "dd", html, dd, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Del, "del", html, del, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Details, "details", html, details, non_void, [TextTag, OpenTag]);
#[rustfmt::skip] silex_view::define_tag!(Dfn, "dfn", html, dfn, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Dialog, "dialog", html, dialog, non_void, [TextTag, OpenTag]);
#[rustfmt::skip] silex_view::define_tag!(Dir, "dir", html, dir, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Div, "div", html, div, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Dl, "dl", html, dl, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Dt, "dt", html, dt, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Em, "em", html, em, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Embed, "embed", html, embed, void, [MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Fencedframe, "fencedframe", html, fencedframe, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Fieldset, "fieldset", html, fieldset, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Figcaption, "figcaption", html, figcaption, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Figure, "figure", html, figure, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Font, "font", html, font, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Footer, "footer", html, footer, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Form, "form", html, form, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Frame, "frame", html, frame, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Frameset, "frameset", html, frameset, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Geolocation, "geolocation", html, geolocation, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H1, "h1", html, h1, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H2, "h2", html, h2, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H3, "h3", html, h3, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H4, "h4", html, h4, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H5, "h5", html, h5, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(H6, "h6", html, h6, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Head, "head", html, head, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Header, "header", html, header, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Hgroup, "hgroup", html, hgroup, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Hr, "hr", html, hr, void, []);
#[rustfmt::skip] silex_view::define_tag!(Html, "html", html, html, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(I, "i", html, i, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Iframe, "iframe", html, iframe, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Img, "img", html, img, void, [MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Input, "input", html, input, void, [FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Ins, "ins", html, ins, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Kbd, "kbd", html, kbd, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Label, "label", html, label, non_void, [TextTag, LabelTag]);
#[rustfmt::skip] silex_view::define_tag!(Legend, "legend", html, legend, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Li, "li", html, li, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Link, "link", html, link, void, [AnchorTag]);
#[rustfmt::skip] silex_view::define_tag!(Main, "main", html, main, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Map, "map", html, map, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Mark, "mark", html, mark, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Marquee, "marquee", html, marquee, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Menu, "menu", html, menu, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Meta, "meta", html, meta, void, []);
#[rustfmt::skip] silex_view::define_tag!(Meter, "meter", html, meter, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Nav, "nav", html, nav, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Nobr, "nobr", html, nobr, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Noembed, "noembed", html, noembed, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Noframes, "noframes", html, noframes, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Noscript, "noscript", html, noscript, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Object, "object", html, object, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Ol, "ol", html, ol, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Optgroup, "optgroup", html, optgroup, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(OptionTag, "option", html, option_tag, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Output, "output", html, output, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(P, "p", html, p, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Param, "param", html, param, void, []);
#[rustfmt::skip] silex_view::define_tag!(Picture, "picture", html, picture, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Plaintext, "plaintext", html, plaintext, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Pre, "pre", html, pre, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Progress, "progress", html, progress, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Q, "q", html, q, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Rb, "rb", html, rb, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Rp, "rp", html, rp, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Rt, "rt", html, rt, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Rtc, "rtc", html, rtc, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Ruby, "ruby", html, ruby, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(S, "s", html, s, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Samp, "samp", html, samp, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Script, "script", html, script, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Search, "search", html, search, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Section, "section", html, section, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Select, "select", html, select, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Selectedcontent, "selectedcontent", html, selectedcontent, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Slot, "slot", html, slot, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Small, "small", html, small, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Source, "source", html, source, void, [MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Span, "span", html, span, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Strike, "strike", html, strike, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Strong, "strong", html, strong, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Style, "style", html, style, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Sub, "sub", html, sub, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Summary, "summary", html, summary, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Sup, "sup", html, sup, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Table, "table", html, table, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Tbody, "tbody", html, tbody, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Td, "td", html, td, non_void, [TextTag, TableCellTag]);
#[rustfmt::skip] silex_view::define_tag!(Template, "template", html, template, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Textarea, "textarea", html, textarea, non_void, [TextTag, FormTag]);
#[rustfmt::skip] silex_view::define_tag!(Tfoot, "tfoot", html, tfoot, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Th, "th", html, th, non_void, [TextTag, TableCellTag, TableHeaderTag]);
#[rustfmt::skip] silex_view::define_tag!(Thead, "thead", html, thead, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Time, "time", html, time, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Title, "title", html, title, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Tr, "tr", html, tr, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Track, "track", html, track, void, [MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Tt, "tt", html, tt, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(U, "u", html, u, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Ul, "ul", html, ul, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Var, "var", html, var, non_void, [TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Video, "video", html, video, non_void, [TextTag, MediaTag]);
#[rustfmt::skip] silex_view::define_tag!(Wbr, "wbr", html, wbr, void, []);
#[rustfmt::skip] silex_view::define_tag!(Xmp, "xmp", html, xmp, non_void, [TextTag]);

// --- Macros ---
#[rustfmt::skip] #[macro_export] macro_rules! a {
    () => { $crate::html::a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::a($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! abbr {
    () => { $crate::html::abbr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::abbr($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! acronym {
    () => { $crate::html::acronym($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::acronym($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! address {
    () => { $crate::html::address($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::address($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! article {
    () => { $crate::html::article($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::article($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! aside {
    () => { $crate::html::aside($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::aside($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! audio {
    () => { $crate::html::audio($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::audio($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! b {
    () => { $crate::html::b($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::b($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! bdi {
    () => { $crate::html::bdi($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdi($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! bdo {
    () => { $crate::html::bdo($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::bdo($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! big {
    () => { $crate::html::big($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::big($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! blockquote {
    () => { $crate::html::blockquote($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::blockquote($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! body {
    () => { $crate::html::body($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::body($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! button {
    () => { $crate::html::button($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::button($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! canvas {
    () => { $crate::html::canvas($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::canvas($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! caption {
    () => { $crate::html::caption($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::caption($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! center {
    () => { $crate::html::center($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::center($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! cite {
    () => { $crate::html::cite($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::cite($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! code {
    () => { $crate::html::code($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::code($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! colgroup {
    () => { $crate::html::colgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::colgroup($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! data_tag {
    () => { $crate::html::data_tag($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::data_tag($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! datalist {
    () => { $crate::html::datalist($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::datalist($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dd {
    () => { $crate::html::dd($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dd($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! del {
    () => { $crate::html::del($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::del($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! details {
    () => { $crate::html::details($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::details($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dfn {
    () => { $crate::html::dfn($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dfn($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dialog {
    () => { $crate::html::dialog($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dialog($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dir {
    () => { $crate::html::dir($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dir($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! div {
    () => { $crate::html::div($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::div($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dl {
    () => { $crate::html::dl($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dl($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! dt {
    () => { $crate::html::dt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::dt($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! em {
    () => { $crate::html::em($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::em($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fencedframe {
    () => { $crate::html::fencedframe($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::fencedframe($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fieldset {
    () => { $crate::html::fieldset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::fieldset($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! figcaption {
    () => { $crate::html::figcaption($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::figcaption($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! figure {
    () => { $crate::html::figure($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::figure($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! font {
    () => { $crate::html::font($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::font($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! footer {
    () => { $crate::html::footer($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::footer($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! form {
    () => { $crate::html::form($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::form($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! frame {
    () => { $crate::html::frame($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::frame($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! frameset {
    () => { $crate::html::frameset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::frameset($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! geolocation {
    () => { $crate::html::geolocation($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::geolocation($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h1 {
    () => { $crate::html::h1($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h1($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h2 {
    () => { $crate::html::h2($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h2($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h3 {
    () => { $crate::html::h3($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h3($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h4 {
    () => { $crate::html::h4($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h4($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h5 {
    () => { $crate::html::h5($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h5($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! h6 {
    () => { $crate::html::h6($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::h6($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! head {
    () => { $crate::html::head($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::head($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! header {
    () => { $crate::html::header($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::header($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! hgroup {
    () => { $crate::html::hgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::hgroup($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! html {
    () => { $crate::html::html($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::html($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! i {
    () => { $crate::html::i($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::i($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! iframe {
    () => { $crate::html::iframe($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::iframe($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ins {
    () => { $crate::html::ins($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ins($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! kbd {
    () => { $crate::html::kbd($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::kbd($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! label {
    () => { $crate::html::label($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::label($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! legend {
    () => { $crate::html::legend($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::legend($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! li {
    () => { $crate::html::li($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::li($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! main {
    () => { $crate::html::main($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::main($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! map {
    () => { $crate::html::map($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::map($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mark {
    () => { $crate::html::mark($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::mark($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! marquee {
    () => { $crate::html::marquee($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::marquee($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! menu {
    () => { $crate::html::menu($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::menu($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! meter {
    () => { $crate::html::meter($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::meter($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! nav {
    () => { $crate::html::nav($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::nav($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! nobr {
    () => { $crate::html::nobr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::nobr($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noembed {
    () => { $crate::html::noembed($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noembed($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noframes {
    () => { $crate::html::noframes($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noframes($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! noscript {
    () => { $crate::html::noscript($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::noscript($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! object {
    () => { $crate::html::object($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::object($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ol {
    () => { $crate::html::ol($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ol($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! optgroup {
    () => { $crate::html::optgroup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::optgroup($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! option_tag {
    () => { $crate::html::option_tag($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::option_tag($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! output {
    () => { $crate::html::output($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::output($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! p {
    () => { $crate::html::p($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::p($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! picture {
    () => { $crate::html::picture($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::picture($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! plaintext {
    () => { $crate::html::plaintext($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::plaintext($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! pre {
    () => { $crate::html::pre($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::pre($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! progress {
    () => { $crate::html::progress($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::progress($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! q {
    () => { $crate::html::q($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::q($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rb {
    () => { $crate::html::rb($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rb($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rp {
    () => { $crate::html::rp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rp($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rt {
    () => { $crate::html::rt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rt($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! rtc {
    () => { $crate::html::rtc($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::rtc($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ruby {
    () => { $crate::html::ruby($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ruby($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! s {
    () => { $crate::html::s($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::s($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! samp {
    () => { $crate::html::samp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::samp($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! script {
    () => { $crate::html::script($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::script($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! search {
    () => { $crate::html::search($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::search($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! section {
    () => { $crate::html::section($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::section($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! select {
    () => { $crate::html::select($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::select($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! selectedcontent {
    () => { $crate::html::selectedcontent($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::selectedcontent($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! slot {
    () => { $crate::html::slot($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::slot($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! small {
    () => { $crate::html::small($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::small($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! span {
    () => { $crate::html::span($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::span($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! strike {
    () => { $crate::html::strike($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::strike($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! strong {
    () => { $crate::html::strong($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::strong($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! style {
    () => { $crate::html::style($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::style($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! sub {
    () => { $crate::html::sub($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::sub($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! summary {
    () => { $crate::html::summary($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::summary($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! sup {
    () => { $crate::html::sup($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::sup($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! table {
    () => { $crate::html::table($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::table($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tbody {
    () => { $crate::html::tbody($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tbody($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! td {
    () => { $crate::html::td($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::td($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! template {
    () => { $crate::html::template($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::template($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! textarea {
    () => { $crate::html::textarea($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::textarea($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tfoot {
    () => { $crate::html::tfoot($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tfoot($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! th {
    () => { $crate::html::th($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::th($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! thead {
    () => { $crate::html::thead($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::thead($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! time {
    () => { $crate::html::time($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::time($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! title {
    () => { $crate::html::title($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::title($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tr {
    () => { $crate::html::tr($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tr($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tt {
    () => { $crate::html::tt($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::tt($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! u {
    () => { $crate::html::u($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::u($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! ul {
    () => { $crate::html::ul($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::ul($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! var {
    () => { $crate::html::var($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::var($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! video {
    () => { $crate::html::video($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::video($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! xmp {
    () => { $crate::html::xmp($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::html::xmp($crate::chain!($($child),+)) };
}
