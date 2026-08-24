// --- Tags ---
#[rustfmt::skip] silex_view::define_tag!(SvgA, "a", svg, svg_a, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Animate, "animate", svg, animate, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(AnimateMotion, "animateMotion", svg, animate_motion, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(AnimateTransform, "animateTransform", svg, animate_transform, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Circle, "circle", svg, circle, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(ClipPath, "clipPath", svg, clip_path, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Defs, "defs", svg, defs, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Desc, "desc", svg, desc, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Ellipse, "ellipse", svg, ellipse, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(FeBlend, "feBlend", svg, fe_blend, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeColorMatrix, "feColorMatrix", svg, fe_color_matrix, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeComponentTransfer, "feComponentTransfer", svg, fe_component_transfer, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeComposite, "feComposite", svg, fe_composite, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeConvolveMatrix, "feConvolveMatrix", svg, fe_convolve_matrix, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeDiffuseLighting, "feDiffuseLighting", svg, fe_diffuse_lighting, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeDisplacementMap, "feDisplacementMap", svg, fe_displacement_map, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeDistantLight, "feDistantLight", svg, fe_distant_light, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeDropShadow, "feDropShadow", svg, fe_drop_shadow, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeFlood, "feFlood", svg, fe_flood, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeFuncA, "feFuncA", svg, fe_func_a, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeFuncB, "feFuncB", svg, fe_func_b, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeFuncG, "feFuncG", svg, fe_func_g, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeFuncR, "feFuncR", svg, fe_func_r, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeGaussianBlur, "feGaussianBlur", svg, fe_gaussian_blur, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeImage, "feImage", svg, fe_image, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeMerge, "feMerge", svg, fe_merge, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeMergeNode, "feMergeNode", svg, fe_merge_node, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeMorphology, "feMorphology", svg, fe_morphology, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeOffset, "feOffset", svg, fe_offset, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FePointLight, "fePointLight", svg, fe_point_light, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeSpecularLighting, "feSpecularLighting", svg, fe_specular_lighting, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeSpotLight, "feSpotLight", svg, fe_spot_light, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeTile, "feTile", svg, fe_tile, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(FeTurbulence, "feTurbulence", svg, fe_turbulence, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Filter, "filter", svg, filter, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(ForeignObject, "foreignObject", svg, foreign_object, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(G, "g", svg, g, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Image, "image", svg, image, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(Line, "line", svg, line, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(LinearGradient, "linearGradient", svg, linear_gradient, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Marker, "marker", svg, marker, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Mask, "mask", svg, mask, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Metadata, "metadata", svg, metadata, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Mpath, "mpath", svg, mpath, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Path, "path", svg, path, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(Pattern, "pattern", svg, pattern, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Polygon, "polygon", svg, polygon, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(Polyline, "polyline", svg, polyline, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(RadialGradient, "radialGradient", svg, radial_gradient, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Rect, "rect", svg, rect, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(SvgScript, "script", svg, svg_script, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Set, "set", svg, set, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Stop, "stop", svg, stop, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(SvgStyle, "style", svg, svg_style, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Svg, "svg", svg, svg, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Switch, "switch", svg, switch, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Symbol, "symbol", svg, symbol, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Text, "text", svg, text, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(TextPath, "textPath", svg, text_path, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(SvgTitle, "title", svg, svg_title, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(Tspan, "tspan", svg, tspan, non_void, [SvgTag, TextTag]);
#[rustfmt::skip] silex_view::define_tag!(UseEl, "use", svg, use_el, void, [SvgTag]);
#[rustfmt::skip] silex_view::define_tag!(View, "view", svg, view, non_void, [SvgTag, TextTag]);

// --- Macros ---
#[rustfmt::skip] #[macro_export] macro_rules! svg_a {
    () => { $crate::svg::svg_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_a($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate {
    () => { $crate::svg::animate($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate_motion {
    () => { $crate::svg::animate_motion($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_motion($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! animate_transform {
    () => { $crate::svg::animate_transform($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_transform($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! clip_path {
    () => { $crate::svg::clip_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::clip_path($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! defs {
    () => { $crate::svg::defs($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::defs($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! desc {
    () => { $crate::svg::desc($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::desc($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_blend {
    () => { $crate::svg::fe_blend($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_blend($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_color_matrix {
    () => { $crate::svg::fe_color_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_color_matrix($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_component_transfer {
    () => { $crate::svg::fe_component_transfer($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_component_transfer($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_composite {
    () => { $crate::svg::fe_composite($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_composite($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_convolve_matrix {
    () => { $crate::svg::fe_convolve_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_convolve_matrix($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_diffuse_lighting {
    () => { $crate::svg::fe_diffuse_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_diffuse_lighting($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_displacement_map {
    () => { $crate::svg::fe_displacement_map($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_displacement_map($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_distant_light {
    () => { $crate::svg::fe_distant_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_distant_light($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_drop_shadow {
    () => { $crate::svg::fe_drop_shadow($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_drop_shadow($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_flood {
    () => { $crate::svg::fe_flood($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_flood($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_a {
    () => { $crate::svg::fe_func_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_a($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_b {
    () => { $crate::svg::fe_func_b($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_b($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_g {
    () => { $crate::svg::fe_func_g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_g($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_func_r {
    () => { $crate::svg::fe_func_r($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_r($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_gaussian_blur {
    () => { $crate::svg::fe_gaussian_blur($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_gaussian_blur($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_image {
    () => { $crate::svg::fe_image($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_image($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_merge {
    () => { $crate::svg::fe_merge($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_merge_node {
    () => { $crate::svg::fe_merge_node($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge_node($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_morphology {
    () => { $crate::svg::fe_morphology($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_morphology($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_offset {
    () => { $crate::svg::fe_offset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_offset($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_point_light {
    () => { $crate::svg::fe_point_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_point_light($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_specular_lighting {
    () => { $crate::svg::fe_specular_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_specular_lighting($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_spot_light {
    () => { $crate::svg::fe_spot_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_spot_light($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_tile {
    () => { $crate::svg::fe_tile($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_tile($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! fe_turbulence {
    () => { $crate::svg::fe_turbulence($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_turbulence($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! filter {
    () => { $crate::svg::filter($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::filter($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! foreign_object {
    () => { $crate::svg::foreign_object($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::foreign_object($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! g {
    () => { $crate::svg::g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::g($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! linear_gradient {
    () => { $crate::svg::linear_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::linear_gradient($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! marker {
    () => { $crate::svg::marker($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::marker($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mask {
    () => { $crate::svg::mask($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mask($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! metadata {
    () => { $crate::svg::metadata($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::metadata($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! mpath {
    () => { $crate::svg::mpath($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mpath($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! pattern {
    () => { $crate::svg::pattern($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::pattern($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! radial_gradient {
    () => { $crate::svg::radial_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::radial_gradient($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_script {
    () => { $crate::svg::svg_script($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_script($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! set {
    () => { $crate::svg::set($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::set($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_style {
    () => { $crate::svg::svg_style($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_style($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg {
    () => { $crate::svg::svg($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! switch {
    () => { $crate::svg::switch($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::switch($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! symbol {
    () => { $crate::svg::symbol($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::symbol($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! text {
    () => { $crate::svg::text($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! text_path {
    () => { $crate::svg::text_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text_path($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! svg_title {
    () => { $crate::svg::svg_title($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_title($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! tspan {
    () => { $crate::svg::tspan($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::tspan($crate::chain!($($child),+)) };
}
#[rustfmt::skip] #[macro_export] macro_rules! view {
    () => { $crate::svg::view($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::view($crate::chain!($($child),+)) };
}
