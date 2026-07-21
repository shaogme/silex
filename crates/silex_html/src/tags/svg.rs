// --- Tags ---
silex_dom::define_tag!(SvgA, "a", svg_a, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(
    Animate,
    "animate",
    animate,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    AnimateMotion,
    "animateMotion",
    animate_motion,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    AnimateTransform,
    "animateTransform",
    animate_transform,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Circle, "circle", circle, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    ClipPath,
    "clipPath",
    clip_path,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Defs, "defs", defs, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(Desc, "desc", desc, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(Ellipse, "ellipse", ellipse, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FeBlend,
    "feBlend",
    fe_blend,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeColorMatrix,
    "feColorMatrix",
    fe_color_matrix,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeComponentTransfer,
    "feComponentTransfer",
    fe_component_transfer,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeComposite,
    "feComposite",
    fe_composite,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeConvolveMatrix,
    "feConvolveMatrix",
    fe_convolve_matrix,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeDiffuseLighting,
    "feDiffuseLighting",
    fe_diffuse_lighting,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeDisplacementMap,
    "feDisplacementMap",
    fe_displacement_map,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeDistantLight,
    "feDistantLight",
    fe_distant_light,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeDropShadow,
    "feDropShadow",
    fe_drop_shadow,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeFlood,
    "feFlood",
    fe_flood,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeFuncA,
    "feFuncA",
    fe_func_a,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeFuncB,
    "feFuncB",
    fe_func_b,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeFuncG,
    "feFuncG",
    fe_func_g,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeFuncR,
    "feFuncR",
    fe_func_r,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeGaussianBlur,
    "feGaussianBlur",
    fe_gaussian_blur,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeImage,
    "feImage",
    fe_image,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeMerge,
    "feMerge",
    fe_merge,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeMergeNode,
    "feMergeNode",
    fe_merge_node,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeMorphology,
    "feMorphology",
    fe_morphology,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeOffset,
    "feOffset",
    fe_offset,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FePointLight,
    "fePointLight",
    fe_point_light,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeSpecularLighting,
    "feSpecularLighting",
    fe_specular_lighting,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeSpotLight,
    "feSpotLight",
    fe_spot_light,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeTile,
    "feTile",
    fe_tile,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    FeTurbulence,
    "feTurbulence",
    fe_turbulence,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    Filter,
    "filter",
    filter,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    ForeignObject,
    "foreignObject",
    foreign_object,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(G, "g", g, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(Image, "image", image, new_svg, void, [SvgTag]);
silex_dom::define_tag!(Line, "line", line, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    LinearGradient,
    "linearGradient",
    linear_gradient,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    Marker,
    "marker",
    marker,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Mask, "mask", mask, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(
    Metadata,
    "metadata",
    metadata,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Mpath, "mpath", mpath, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(Path, "path", path, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    Pattern,
    "pattern",
    pattern,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Polygon, "polygon", polygon, new_svg, void, [SvgTag]);
silex_dom::define_tag!(Polyline, "polyline", polyline, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    RadialGradient,
    "radialGradient",
    radial_gradient,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Rect, "rect", rect, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    SvgScript,
    "script",
    svg_script,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Set, "set", set, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(Stop, "stop", stop, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    SvgStyle,
    "style",
    svg_style,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Svg, "svg", svg, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(
    Switch,
    "switch",
    switch,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    Symbol,
    "symbol",
    symbol,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Text, "text", text, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(
    TextPath,
    "textPath",
    text_path,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(
    SvgTitle,
    "title",
    svg_title,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);
silex_dom::define_tag!(Tspan, "tspan", tspan, new_svg, non_void, [SvgTag, TextTag]);
silex_dom::define_tag!(UseEl, "use", use_el, new_svg, void, [SvgTag]);
silex_dom::define_tag!(View, "view", view, new_svg, non_void, [SvgTag, TextTag]);

// --- Macros ---
#[macro_export]
macro_rules! svg_a {
    () => { $crate::svg::svg_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_a($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! animate {
    () => { $crate::svg::animate($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! animate_motion {
    () => { $crate::svg::animate_motion($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_motion($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! animate_transform {
    () => { $crate::svg::animate_transform($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_transform($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! clip_path {
    () => { $crate::svg::clip_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::clip_path($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! defs {
    () => { $crate::svg::defs($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::defs($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! desc {
    () => { $crate::svg::desc($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::desc($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_blend {
    () => { $crate::svg::fe_blend($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_blend($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_color_matrix {
    () => { $crate::svg::fe_color_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_color_matrix($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_component_transfer {
    () => { $crate::svg::fe_component_transfer($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_component_transfer($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_composite {
    () => { $crate::svg::fe_composite($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_composite($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_convolve_matrix {
    () => { $crate::svg::fe_convolve_matrix($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_convolve_matrix($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_diffuse_lighting {
    () => { $crate::svg::fe_diffuse_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_diffuse_lighting($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_displacement_map {
    () => { $crate::svg::fe_displacement_map($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_displacement_map($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_distant_light {
    () => { $crate::svg::fe_distant_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_distant_light($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_drop_shadow {
    () => { $crate::svg::fe_drop_shadow($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_drop_shadow($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_flood {
    () => { $crate::svg::fe_flood($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_flood($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_func_a {
    () => { $crate::svg::fe_func_a($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_a($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_func_b {
    () => { $crate::svg::fe_func_b($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_b($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_func_g {
    () => { $crate::svg::fe_func_g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_g($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_func_r {
    () => { $crate::svg::fe_func_r($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_func_r($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_gaussian_blur {
    () => { $crate::svg::fe_gaussian_blur($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_gaussian_blur($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_image {
    () => { $crate::svg::fe_image($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_image($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_merge {
    () => { $crate::svg::fe_merge($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_merge_node {
    () => { $crate::svg::fe_merge_node($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_merge_node($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_morphology {
    () => { $crate::svg::fe_morphology($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_morphology($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_offset {
    () => { $crate::svg::fe_offset($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_offset($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_point_light {
    () => { $crate::svg::fe_point_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_point_light($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_specular_lighting {
    () => { $crate::svg::fe_specular_lighting($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_specular_lighting($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_spot_light {
    () => { $crate::svg::fe_spot_light($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_spot_light($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_tile {
    () => { $crate::svg::fe_tile($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_tile($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! fe_turbulence {
    () => { $crate::svg::fe_turbulence($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::fe_turbulence($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! filter {
    () => { $crate::svg::filter($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::filter($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! foreign_object {
    () => { $crate::svg::foreign_object($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::foreign_object($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! g {
    () => { $crate::svg::g($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::g($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! linear_gradient {
    () => { $crate::svg::linear_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::linear_gradient($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! marker {
    () => { $crate::svg::marker($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::marker($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! mask {
    () => { $crate::svg::mask($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mask($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! metadata {
    () => { $crate::svg::metadata($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::metadata($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! mpath {
    () => { $crate::svg::mpath($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mpath($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! pattern {
    () => { $crate::svg::pattern($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::pattern($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! radial_gradient {
    () => { $crate::svg::radial_gradient($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::radial_gradient($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! svg_script {
    () => { $crate::svg::svg_script($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_script($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! set {
    () => { $crate::svg::set($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::set($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! svg_style {
    () => { $crate::svg::svg_style($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_style($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! svg {
    () => { $crate::svg::svg($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! switch {
    () => { $crate::svg::switch($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::switch($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! symbol {
    () => { $crate::svg::symbol($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::symbol($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! text {
    () => { $crate::svg::text($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! text_path {
    () => { $crate::svg::text_path($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text_path($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! svg_title {
    () => { $crate::svg::svg_title($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_title($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! tspan {
    () => { $crate::svg::tspan($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::tspan($crate::view_chain!($($child),+)) };
}
#[macro_export]
macro_rules! view {
    () => { $crate::svg::view($crate::ViewNil) };
    ($($child:expr),+ $(,)?) => { $crate::svg::view($crate::view_chain!($($child),+)) };
}
