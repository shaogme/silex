// --- Tags ---
#[rustfmt::skip] silex_dom::define_tag!(SvgA, web_sys::HtmlAnchorElement, "a", svg_a, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Animate, web_sys::SvgElement, "animate", animate, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(AnimateMotion, web_sys::SvgElement, "animateMotion", animate_motion, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(AnimateTransform, web_sys::SvgElement, "animateTransform", animate_transform, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Circle, web_sys::SvgElement, "circle", circle, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(ClipPath, web_sys::SvgElement, "clipPath", clip_path, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Defs, web_sys::SvgElement, "defs", defs, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Desc, web_sys::SvgElement, "desc", desc, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Ellipse, web_sys::SvgElement, "ellipse", ellipse, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeBlend, web_sys::SvgElement, "feBlend", fe_blend, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeColorMatrix, web_sys::SvgElement, "feColorMatrix", fe_color_matrix, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeComponentTransfer, web_sys::SvgElement, "feComponentTransfer", fe_component_transfer, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeComposite, web_sys::SvgElement, "feComposite", fe_composite, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeConvolveMatrix, web_sys::SvgElement, "feConvolveMatrix", fe_convolve_matrix, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeDiffuseLighting, web_sys::SvgElement, "feDiffuseLighting", fe_diffuse_lighting, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeDisplacementMap, web_sys::SvgElement, "feDisplacementMap", fe_displacement_map, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeDistantLight, web_sys::SvgElement, "feDistantLight", fe_distant_light, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeDropShadow, web_sys::SvgElement, "feDropShadow", fe_drop_shadow, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeFlood, web_sys::SvgElement, "feFlood", fe_flood, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeFuncA, web_sys::SvgElement, "feFuncA", fe_func_a, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeFuncB, web_sys::SvgElement, "feFuncB", fe_func_b, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeFuncG, web_sys::SvgElement, "feFuncG", fe_func_g, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeFuncR, web_sys::SvgElement, "feFuncR", fe_func_r, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeGaussianBlur, web_sys::SvgElement, "feGaussianBlur", fe_gaussian_blur, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeImage, web_sys::SvgElement, "feImage", fe_image, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeMerge, web_sys::SvgElement, "feMerge", fe_merge, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeMergeNode, web_sys::SvgElement, "feMergeNode", fe_merge_node, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeMorphology, web_sys::SvgElement, "feMorphology", fe_morphology, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeOffset, web_sys::SvgElement, "feOffset", fe_offset, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FePointLight, web_sys::SvgElement, "fePointLight", fe_point_light, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeSpecularLighting, web_sys::SvgElement, "feSpecularLighting", fe_specular_lighting, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeSpotLight, web_sys::SvgElement, "feSpotLight", fe_spot_light, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeTile, web_sys::SvgElement, "feTile", fe_tile, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(FeTurbulence, web_sys::SvgElement, "feTurbulence", fe_turbulence, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Filter, web_sys::SvgElement, "filter", filter, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(ForeignObject, web_sys::SvgElement, "foreignObject", foreign_object, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(G, web_sys::SvgElement, "g", g, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Image, web_sys::SvgElement, "image", image, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(Line, web_sys::SvgElement, "line", line, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(LinearGradient, web_sys::SvgElement, "linearGradient", linear_gradient, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Marker, web_sys::SvgElement, "marker", marker, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Mask, web_sys::SvgElement, "mask", mask, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Metadata, web_sys::SvgElement, "metadata", metadata, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Mpath, web_sys::SvgElement, "mpath", mpath, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Path, web_sys::SvgElement, "path", path, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(Pattern, web_sys::SvgElement, "pattern", pattern, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Polygon, web_sys::SvgElement, "polygon", polygon, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(Polyline, web_sys::SvgElement, "polyline", polyline, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(RadialGradient, web_sys::SvgElement, "radialGradient", radial_gradient, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Rect, web_sys::SvgElement, "rect", rect, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(SvgScript, web_sys::SvgElement, "script", svg_script, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Set, web_sys::SvgElement, "set", set, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Stop, web_sys::SvgElement, "stop", stop, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(SvgStyle, web_sys::SvgElement, "style", svg_style, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Svg, web_sys::SvgElement, "svg", svg, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Switch, web_sys::SvgElement, "switch", switch, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Symbol, web_sys::SvgElement, "symbol", symbol, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Text, web_sys::SvgElement, "text", text, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(TextPath, web_sys::SvgElement, "textPath", text_path, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(SvgTitle, web_sys::SvgElement, "title", svg_title, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(Tspan, web_sys::SvgElement, "tspan", tspan, new_svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_dom::define_tag!(UseEl, web_sys::SvgElement, "use", use_el, new_svg, void, [SvgTag]);
#[rustfmt::skip] silex_dom::define_tag!(View, web_sys::SvgElement, "view", view, new_svg, non_void, [SvgTag, TextTag]);

// --- Macros ---
#[rustfmt::skip] #[macro_export] macro_rules! svg_a {
    () => { $crate::svg::svg_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_a($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate {
    () => { $crate::svg::animate($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate_motion {
    () => { $crate::svg::animate_motion($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_motion($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate_transform {
    () => { $crate::svg::animate_transform($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_transform($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! clip_path {
    () => { $crate::svg::clip_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::clip_path($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! defs {
    () => { $crate::svg::defs($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::defs($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! desc {
    () => { $crate::svg::desc($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::desc($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_blend {
    () => { $crate::svg::fe_blend($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_blend($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_color_matrix {
    () => { $crate::svg::fe_color_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_color_matrix($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_component_transfer {
    () => { $crate::svg::fe_component_transfer($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_component_transfer($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_composite {
    () => { $crate::svg::fe_composite($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_composite($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_convolve_matrix {
    () => { $crate::svg::fe_convolve_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_convolve_matrix($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_diffuse_lighting {
    () => { $crate::svg::fe_diffuse_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_diffuse_lighting($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_displacement_map {
    () => { $crate::svg::fe_displacement_map($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_displacement_map($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_distant_light {
    () => { $crate::svg::fe_distant_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_distant_light($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_drop_shadow {
    () => { $crate::svg::fe_drop_shadow($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_drop_shadow($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_flood {
    () => { $crate::svg::fe_flood($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_flood($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_a {
    () => { $crate::svg::fe_func_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_a($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_b {
    () => { $crate::svg::fe_func_b($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_b($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_g {
    () => { $crate::svg::fe_func_g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_g($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_r {
    () => { $crate::svg::fe_func_r($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_r($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_gaussian_blur {
    () => { $crate::svg::fe_gaussian_blur($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_gaussian_blur($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_image {
    () => { $crate::svg::fe_image($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_image($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_merge {
    () => { $crate::svg::fe_merge($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_merge_node {
    () => { $crate::svg::fe_merge_node($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge_node($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_morphology {
    () => { $crate::svg::fe_morphology($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_morphology($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_offset {
    () => { $crate::svg::fe_offset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_offset($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_point_light {
    () => { $crate::svg::fe_point_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_point_light($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_specular_lighting {
    () => { $crate::svg::fe_specular_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_specular_lighting($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_spot_light {
    () => { $crate::svg::fe_spot_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_spot_light($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_tile {
    () => { $crate::svg::fe_tile($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_tile($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_turbulence {
    () => { $crate::svg::fe_turbulence($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_turbulence($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! filter {
    () => { $crate::svg::filter($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::filter($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! foreign_object {
    () => { $crate::svg::foreign_object($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::foreign_object($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! g {
    () => { $crate::svg::g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::g($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! linear_gradient {
    () => { $crate::svg::linear_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::linear_gradient($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! marker {
    () => { $crate::svg::marker($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::marker($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mask {
    () => { $crate::svg::mask($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mask($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! metadata {
    () => { $crate::svg::metadata($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::metadata($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mpath {
    () => { $crate::svg::mpath($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mpath($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! pattern {
    () => { $crate::svg::pattern($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::pattern($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! radial_gradient {
    () => { $crate::svg::radial_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::radial_gradient($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_script {
    () => { $crate::svg::svg_script($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_script($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! set {
    () => { $crate::svg::set($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::set($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_style {
    () => { $crate::svg::svg_style($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_style($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg {
    () => { $crate::svg::svg($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! switch {
    () => { $crate::svg::switch($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::switch($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! symbol {
    () => { $crate::svg::symbol($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::symbol($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! text {
    () => { $crate::svg::text($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! text_path {
    () => { $crate::svg::text_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text_path($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_title {
    () => { $crate::svg::svg_title($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_title($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tspan {
    () => { $crate::svg::tspan($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::tspan($crate::view_chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! view {
    () => { $crate::svg::view($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::view($crate::view_chain!($($child),+)) };
}
