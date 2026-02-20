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
silex_dom::define_tag!(FeBlend, "feBlend", fe_blend, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FeColorMatrix,
    "feColorMatrix",
    fe_color_matrix,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeComponentTransfer,
    "feComponentTransfer",
    fe_component_transfer,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeComposite,
    "feComposite",
    fe_composite,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeConvolveMatrix,
    "feConvolveMatrix",
    fe_convolve_matrix,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeDiffuseLighting,
    "feDiffuseLighting",
    fe_diffuse_lighting,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeDisplacementMap,
    "feDisplacementMap",
    fe_displacement_map,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeDistantLight,
    "feDistantLight",
    fe_distant_light,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeDropShadow,
    "feDropShadow",
    fe_drop_shadow,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(FeFlood, "feFlood", fe_flood, new_svg, void, [SvgTag]);
silex_dom::define_tag!(FeFuncA, "feFuncA", fe_func_a, new_svg, void, [SvgTag]);
silex_dom::define_tag!(FeFuncB, "feFuncB", fe_func_b, new_svg, void, [SvgTag]);
silex_dom::define_tag!(FeFuncG, "feFuncG", fe_func_g, new_svg, void, [SvgTag]);
silex_dom::define_tag!(FeFuncR, "feFuncR", fe_func_r, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FeGaussianBlur,
    "feGaussianBlur",
    fe_gaussian_blur,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(FeImage, "feImage", fe_image, new_svg, void, [SvgTag]);
silex_dom::define_tag!(FeMerge, "feMerge", fe_merge, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FeMergeNode,
    "feMergeNode",
    fe_merge_node,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeMorphology,
    "feMorphology",
    fe_morphology,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(FeOffset, "feOffset", fe_offset, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FePointLight,
    "fePointLight",
    fe_point_light,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeSpecularLighting,
    "feSpecularLighting",
    fe_specular_lighting,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(
    FeSpotLight,
    "feSpotLight",
    fe_spot_light,
    new_svg,
    void,
    [SvgTag]
);
silex_dom::define_tag!(FeTile, "feTile", fe_tile, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    FeTurbulence,
    "feTurbulence",
    fe_turbulence,
    new_svg,
    void,
    [SvgTag]
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
silex_dom::define_tag!(Image, "image", image, new_svg, non_void, [SvgTag, TextTag]);
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
silex_dom::define_tag!(Use, "use", use_tag, new_svg, void, [SvgTag]);
silex_dom::define_tag!(
    ViewTag,
    "view",
    view_tag,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);

// --- Macros ---
#[macro_export]
macro_rules! svg_a {
    () => { $crate::svg::svg_a(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_a(($($child),+)) };
}
#[macro_export]
macro_rules! animate {
    () => { $crate::svg::animate(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate(($($child),+)) };
}
#[macro_export]
macro_rules! animate_motion {
    () => { $crate::svg::animate_motion(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_motion(($($child),+)) };
}
#[macro_export]
macro_rules! animate_transform {
    () => { $crate::svg::animate_transform(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::animate_transform(($($child),+)) };
}
#[macro_export]
macro_rules! clip_path {
    () => { $crate::svg::clip_path(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::clip_path(($($child),+)) };
}
#[macro_export]
macro_rules! defs {
    () => { $crate::svg::defs(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::defs(($($child),+)) };
}
#[macro_export]
macro_rules! desc {
    () => { $crate::svg::desc(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::desc(($($child),+)) };
}
#[macro_export]
macro_rules! filter {
    () => { $crate::svg::filter(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::filter(($($child),+)) };
}
#[macro_export]
macro_rules! foreign_object {
    () => { $crate::svg::foreign_object(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::foreign_object(($($child),+)) };
}
#[macro_export]
macro_rules! g {
    () => { $crate::svg::g(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::g(($($child),+)) };
}
#[macro_export]
macro_rules! image {
    () => { $crate::svg::image(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::image(($($child),+)) };
}
#[macro_export]
macro_rules! linear_gradient {
    () => { $crate::svg::linear_gradient(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::linear_gradient(($($child),+)) };
}
#[macro_export]
macro_rules! marker {
    () => { $crate::svg::marker(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::marker(($($child),+)) };
}
#[macro_export]
macro_rules! mask {
    () => { $crate::svg::mask(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mask(($($child),+)) };
}
#[macro_export]
macro_rules! metadata {
    () => { $crate::svg::metadata(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::metadata(($($child),+)) };
}
#[macro_export]
macro_rules! mpath {
    () => { $crate::svg::mpath(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::mpath(($($child),+)) };
}
#[macro_export]
macro_rules! pattern {
    () => { $crate::svg::pattern(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::pattern(($($child),+)) };
}
#[macro_export]
macro_rules! radial_gradient {
    () => { $crate::svg::radial_gradient(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::radial_gradient(($($child),+)) };
}
#[macro_export]
macro_rules! svg_script {
    () => { $crate::svg::svg_script(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_script(($($child),+)) };
}
#[macro_export]
macro_rules! set {
    () => { $crate::svg::set(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::set(($($child),+)) };
}
#[macro_export]
macro_rules! svg_style {
    () => { $crate::svg::svg_style(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_style(($($child),+)) };
}
#[macro_export]
macro_rules! svg {
    () => { $crate::svg::svg(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg(($($child),+)) };
}
#[macro_export]
macro_rules! switch {
    () => { $crate::svg::switch(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::switch(($($child),+)) };
}
#[macro_export]
macro_rules! symbol {
    () => { $crate::svg::symbol(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::symbol(($($child),+)) };
}
#[macro_export]
macro_rules! text {
    () => { $crate::svg::text(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text(($($child),+)) };
}
#[macro_export]
macro_rules! text_path {
    () => { $crate::svg::text_path(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::text_path(($($child),+)) };
}
#[macro_export]
macro_rules! svg_title {
    () => { $crate::svg::svg_title(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::svg_title(($($child),+)) };
}
#[macro_export]
macro_rules! tspan {
    () => { $crate::svg::tspan(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::tspan(($($child),+)) };
}
#[macro_export]
macro_rules! view_tag {
    () => { $crate::svg::view_tag(()) };
    ($($child:expr),+ $(,)?) => { $crate::svg::view_tag(($($child),+)) };
}
