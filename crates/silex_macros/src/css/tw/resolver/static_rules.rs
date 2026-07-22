use crate::css::tw::ast::{Modifier, UtilityRule};
use proc_macro2::Span;

use super::{hex, kw, make_rule, num, num_unitless, px, rem};

macro_rules! resolve_rules {
    (
        $modifiers:expr, $span:expr, $token:expr;
        $(
            $( $pat:pat_param )|+ => {
                $( $prop:expr => $val:expr ),* $(,)?
            }
        ),* $(,)?
    ) => {
        match $token {
            $(
                $( $pat )|+ => Some(vec![
                    $(
                        make_rule($modifiers.clone(), $prop, $val, $span),
                    )*
                ]),
            )*
            _ => None,
        }
    };
}

/// 解析静态与预设 Utility 规则
pub fn resolve_static_rules(
    modifiers: &[Modifier],
    utility_token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {
    let mods = modifiers.to_vec();

    resolve_rules! {
        mods, span, utility_token;

        // --- 布局 & Box-Sizing & Display ---
        "box-border" => { "box-sizing" => kw("border-box") },
        "box-content" => { "box-sizing" => kw("content-box") },
        "block" => { "display" => kw("block") },
        "inline-block" => { "display" => kw("inline-block") },
        "inline" => { "display" => kw("inline") },
        "flex" => { "display" => kw("flex") },
        "inline-flex" => { "display" => kw("inline-flex") },
        "grid" => { "display" => kw("grid") },
        "inline-grid" => { "display" => kw("inline-grid") },
        "hidden" => { "display" => kw("none") },
        "group" | "peer" => {},

        // --- 隔离 (Isolation) ---
        "isolate" => { "isolation" => kw("isolate") },
        "isolation-auto" => { "isolation" => kw("auto") },

        // --- 容器查询 (Container Type & Name) ---
        "@container" | "container" | "container-inline-size" => { "container-type" => kw("inline-size") },
        "container-normal" => { "container-type" => kw("normal") },
        "container-size" => { "container-type" => kw("size") },

        // --- Flexbox 方向与包裹 ---
        "flex-row" => { "flex-direction" => kw("row") },
        "flex-row-reverse" => { "flex-direction" => kw("row-reverse") },
        "flex-col" => { "flex-direction" => kw("column") },
        "flex-col-reverse" => { "flex-direction" => kw("column-reverse") },
        "flex-wrap" => { "flex-wrap" => kw("wrap") },
        "flex-nowrap" => { "flex-wrap" => kw("nowrap") },
        "flex-1" => { "flex" => kw("1 1 0%") },
        "flex-auto" => { "flex" => kw("1 1 auto") },
        "flex-initial" => { "flex" => kw("0 1 auto") },
        "flex-none" => { "flex" => kw("none") },
        "grow" => { "flex-grow" => num_unitless(1.0) },
        "grow-0" => { "flex-grow" => num_unitless(0.0) },
        "shrink" => { "flex-shrink" => num_unitless(1.0) },
        "shrink-0" => { "flex-shrink" => num_unitless(0.0) },

        // --- Align & Justify ---
        "items-start" => { "align-items" => kw("flex-start") },
        "items-center" => { "align-items" => kw("center") },
        "items-end" => { "align-items" => kw("flex-end") },
        "items-stretch" => { "align-items" => kw("stretch") },
        "items-baseline" => { "align-items" => kw("baseline") },
        "justify-start" => { "justify-content" => kw("flex-start") },
        "justify-center" => { "justify-content" => kw("center") },
        "justify-end" => { "justify-content" => kw("flex-end") },
        "justify-between" => { "justify-content" => kw("space-between") },
        "justify-around" => { "justify-content" => kw("space-around") },
        "justify-evenly" => { "justify-content" => kw("space-evenly") },
        "justify-stretch" => { "justify-content" => kw("stretch") },

        // --- Self & Place ---
        "self-auto" => { "align-self" => kw("auto") },
        "self-start" => { "align-self" => kw("flex-start") },
        "self-end" => { "align-self" => kw("flex-end") },
        "self-center" => { "align-self" => kw("center") },
        "self-stretch" => { "align-self" => kw("stretch") },
        "self-baseline" => { "align-self" => kw("baseline") },
        "justify-self-auto" => { "justify-self" => kw("auto") },
        "justify-self-start" => { "justify-self" => kw("start") },
        "justify-self-end" => { "justify-self" => kw("end") },
        "justify-self-center" => { "justify-self" => kw("center") },
        "justify-self-stretch" => { "justify-self" => kw("stretch") },
        "place-items-start" => { "place-items" => kw("start") },
        "place-items-end" => { "place-items" => kw("end") },
        "place-items-center" => { "place-items" => kw("center") },
        "place-items-stretch" => { "place-items" => kw("stretch") },
        "place-content-center" => { "place-content" => kw("center") },
        "place-content-between" => { "place-content" => kw("space-between") },

        // --- 预设尺寸关键字 ---
        "w-full" => { "width" => num(100.0, "%") },
        "h-full" => { "height" => num(100.0, "%") },
        "w-screen" => { "width" => num(100.0, "vw") },
        "h-screen" => { "height" => num(100.0, "vh") },
        "w-auto" => { "width" => kw("auto") },
        "h-auto" => { "height" => kw("auto") },
        "w-min" => { "width" => kw("min-content") },
        "w-max" => { "width" => kw("max-content") },
        "w-fit" => { "width" => kw("fit-content") },
        "h-min" => { "height" => kw("min-content") },
        "h-max" => { "height" => kw("max-content") },
        "h-fit" => { "height" => kw("fit-content") },

        // --- 最小/最大尺寸 ---
        "min-w-0" => { "min-width" => px(0.0) },
        "min-w-full" => { "min-width" => num(100.0, "%") },
        "min-w-min" => { "min-width" => kw("min-content") },
        "min-w-max" => { "min-width" => kw("max-content") },
        "min-w-fit" => { "min-width" => kw("fit-content") },
        "min-h-0" => { "min-height" => px(0.0) },
        "min-h-full" => { "min-height" => num(100.0, "%") },
        "min-h-screen" => { "min-height" => num(100.0, "vh") },
        "min-h-min" => { "min-height" => kw("min-content") },
        "min-h-max" => { "min-height" => kw("max-content") },
        "min-h-fit" => { "min-height" => kw("fit-content") },

        "max-w-0" => { "max-width" => rem(0.0) },
        "max-w-none" => { "max-width" => kw("none") },
        "max-w-xs" => { "max-width" => rem(20.0) },
        "max-w-sm" => { "max-width" => rem(24.0) },
        "max-w-md" => { "max-width" => rem(28.0) },
        "max-w-lg" => { "max-width" => rem(32.0) },
        "max-w-xl" => { "max-width" => rem(36.0) },
        "max-w-2xl" => { "max-width" => rem(42.0) },
        "max-w-3xl" => { "max-width" => rem(48.0) },
        "max-w-4xl" => { "max-width" => rem(56.0) },
        "max-w-5xl" => { "max-width" => rem(64.0) },
        "max-w-6xl" => { "max-width" => rem(72.0) },
        "max-w-7xl" => { "max-width" => rem(80.0) },
        "max-w-full" => { "max-width" => num(100.0, "%") },
        "max-w-prose" => { "max-width" => rem(65.0) },
        "max-w-screen-sm" => { "max-width" => rem(40.0) },
        "max-w-screen-md" => { "max-width" => rem(48.0) },
        "max-w-screen-lg" => { "max-width" => rem(64.0) },
        "max-w-screen-xl" => { "max-width" => rem(80.0) },
        "max-w-screen-2xl" => { "max-width" => rem(96.0) },

        "max-h-full" => { "max-height" => num(100.0, "%") },
        "max-h-screen" => { "max-height" => num(100.0, "vh") },
        "max-h-min" => { "max-height" => kw("min-content") },
        "max-h-max" => { "max-height" => kw("max-content") },
        "max-h-fit" => { "max-height" => kw("fit-content") },

        // --- 静态定位 (Inset & Position) ---
        "static" => { "position" => kw("static") },
        "fixed" => { "position" => kw("fixed") },
        "absolute" => { "position" => kw("absolute") },
        "relative" => { "position" => kw("relative") },
        "sticky" => { "position" => kw("sticky") },
        "inset-0" => { "top" => px(0.0), "right" => px(0.0), "bottom" => px(0.0), "left" => px(0.0) },
        "inset-auto" => { "top" => kw("auto"), "right" => kw("auto"), "bottom" => kw("auto"), "left" => kw("auto") },
        "inset-x-0" => { "left" => px(0.0), "right" => px(0.0) },
        "inset-x-auto" => { "left" => kw("auto"), "right" => kw("auto") },
        "inset-y-0" => { "top" => px(0.0), "bottom" => px(0.0) },
        "inset-y-auto" => { "top" => kw("auto"), "bottom" => kw("auto") },
        "top-0" => { "top" => px(0.0) },
        "top-auto" => { "top" => kw("auto") },
        "right-0" => { "right" => px(0.0) },
        "right-auto" => { "right" => kw("auto") },
        "bottom-0" => { "bottom" => px(0.0) },
        "bottom-auto" => { "bottom" => kw("auto") },
        "left-0" => { "left" => px(0.0) },
        "left-auto" => { "left" => kw("auto") },

        // --- 边距 auto 规则 ---
        "mx-auto" => { "margin-left" => kw("auto"), "margin-right" => kw("auto") },
        "my-auto" => { "margin-top" => kw("auto"), "margin-bottom" => kw("auto") },
        "mt-auto" => { "margin-top" => kw("auto") },
        "mr-auto" => { "margin-right" => kw("auto") },
        "mb-auto" => { "margin-bottom" => kw("auto") },
        "ml-auto" => { "margin-left" => kw("auto") },

        // --- 颜色预设 ---
        "bg-transparent" => { "background-color" => kw("transparent") },
        "text-transparent" => { "color" => kw("transparent") },
        "border-transparent" => { "border-color" => kw("transparent") },
        "border-t-transparent" => { "border-top-color" => kw("transparent") },
        "border-r-transparent" => { "border-right-color" => kw("transparent") },
        "border-b-transparent" => { "border-bottom-color" => kw("transparent") },
        "border-l-transparent" => { "border-left-color" => kw("transparent") },
        "bg-current" => { "background-color" => kw("currentColor") },
        "text-current" => { "color" => kw("currentColor") },
        "border-current" => { "border-color" => kw("currentColor") },
        "bg-white" => { "background-color" => hex("#ffffff") },
        "text-white" => { "color" => hex("#ffffff") },
        "border-white" => { "border-color" => hex("#ffffff") },
        "bg-black" => { "background-color" => hex("#000000") },
        "text-black" => { "color" => hex("#000000") },
        "border-black" => { "border-color" => hex("#000000") },

        // --- 排版 Font & Text ---
        "text-left" => { "text-align" => kw("left") },
        "text-center" => { "text-align" => kw("center") },
        "text-right" => { "text-align" => kw("right") },
        "text-justify" => { "text-align" => kw("justify") },

        "uppercase" => { "text-transform" => kw("uppercase") },
        "lowercase" => { "text-transform" => kw("lowercase") },
        "capitalize" => { "text-transform" => kw("capitalize") },
        "normal-case" => { "text-transform" => kw("none") },

        "italic" => { "font-style" => kw("italic") },
        "not-italic" => { "font-style" => kw("normal") },

        "underline" => { "text-decoration-line" => kw("underline") },
        "overline" => { "text-decoration-line" => kw("overline") },
        "line-through" => { "text-decoration-line" => kw("line-through") },
        "no-underline" => { "text-decoration-line" => kw("none") },

        "font-mono" => { "font-family" => kw("ui-monospace, monospace") },
        "font-sans" => { "font-family" => kw("ui-sans-serif, system-ui, sans-serif") },
        "font-serif" => { "font-family" => kw("ui-serif, Georgia, serif") },

        "tracking-tighter" => { "letter-spacing" => num(-0.05, "em") },
        "tracking-tight" => { "letter-spacing" => num(-0.025, "em") },
        "tracking-normal" => { "letter-spacing" => num(0.0, "em") },
        "tracking-wide" => { "letter-spacing" => num(0.025, "em") },
        "tracking-wider" => { "letter-spacing" => num(0.05, "em") },
        "tracking-widest" => { "letter-spacing" => num(0.1, "em") },

        "leading-none" => { "line-height" => num_unitless(1.0) },
        "leading-tight" => { "line-height" => num_unitless(1.25) },
        "leading-snug" => { "line-height" => num_unitless(1.375) },
        "leading-normal" => { "line-height" => num_unitless(1.5) },
        "leading-relaxed" => { "line-height" => num_unitless(1.625) },
        "leading-loose" => { "line-height" => num_unitless(2.0) },

        "font-thin" => { "font-weight" => num_unitless(100.0) },
        "font-extralight" => { "font-weight" => num_unitless(200.0) },
        "font-light" => { "font-weight" => num_unitless(300.0) },
        "font-normal" => { "font-weight" => num_unitless(400.0) },
        "font-medium" => { "font-weight" => num_unitless(500.0) },
        "font-semibold" => { "font-weight" => num_unitless(600.0) },
        "font-bold" => { "font-weight" => num_unitless(700.0) },
        "font-extrabold" => { "font-weight" => num_unitless(800.0) },
        "font-black" => { "font-weight" => num_unitless(900.0) },

        "text-xs" => { "font-size" => rem(0.75), "line-height" => rem(1.0) },
        "text-sm" => { "font-size" => rem(0.875), "line-height" => rem(1.25) },
        "text-base" => { "font-size" => rem(1.0), "line-height" => rem(1.5) },
        "text-lg" => { "font-size" => rem(1.125), "line-height" => rem(1.75) },
        "text-xl" => { "font-size" => rem(1.25), "line-height" => rem(1.75) },
        "text-2xl" => { "font-size" => rem(1.5), "line-height" => rem(2.0) },
        "text-3xl" => { "font-size" => rem(1.875), "line-height" => rem(2.25) },
        "text-4xl" => { "font-size" => rem(2.25), "line-height" => rem(2.5) },
        "text-5xl" => { "font-size" => rem(3.0), "line-height" => num_unitless(1.0) },
        "text-6xl" => { "font-size" => rem(3.75), "line-height" => num_unitless(1.0) },
        "text-7xl" => { "font-size" => rem(4.5), "line-height" => num_unitless(1.0) },
        "text-8xl" => { "font-size" => rem(6.0), "line-height" => num_unitless(1.0) },
        "text-9xl" => { "font-size" => rem(8.0), "line-height" => num_unitless(1.0) },

        // --- Whitespace & Word Break ---
        "whitespace-normal" => { "white-space" => kw("normal") },
        "whitespace-nowrap" => { "white-space" => kw("nowrap") },
        "whitespace-pre" => { "white-space" => kw("pre") },
        "whitespace-pre-line" => { "white-space" => kw("pre-line") },
        "whitespace-pre-wrap" => { "white-space" => kw("pre-wrap") },
        "break-normal" => { "overflow-wrap" => kw("normal"), "word-break" => kw("normal") },
        "break-words" => { "overflow-wrap" => kw("break-word") },
        "break-all" => { "word-break" => kw("break-all") },
        "break-keep" => { "word-break" => kw("keep-all") },

        // --- 圆角 Rounded ---
        "rounded-none" => { "border-radius" => px(0.0) },
        "rounded-sm" => { "border-radius" => rem(0.125) },
        "rounded" | "rounded-md" => { "border-radius" => rem(0.375) },
        "rounded-lg" => { "border-radius" => rem(0.5) },
        "rounded-xl" => { "border-radius" => rem(0.75) },
        "rounded-2xl" => { "border-radius" => rem(1.0) },
        "rounded-3xl" => { "border-radius" => rem(1.5) },
        "rounded-full" => { "border-radius" => px(9999.0) },

        // --- 边框 Border ---
        "border" => {
            "border-width" => px(1.0),
            "border-style" => kw("solid"),
        },
        "border-0" => { "border-width" => px(0.0) },
        "border-2" => { "border-width" => px(2.0) },
        "border-4" => { "border-width" => px(4.0) },
        "border-8" => { "border-width" => px(8.0) },
        "border-solid" => { "border-style" => kw("solid") },
        "border-dashed" => { "border-style" => kw("dashed") },
        "border-dotted" => { "border-style" => kw("dotted") },
        "border-double" => { "border-style" => kw("double") },
        "border-none" => { "border-style" => kw("none") },
        "outline-none" => { "outline" => kw("2px solid transparent") },

        // --- 阴影 Shadow ---
        "shadow-sm" => { "box-shadow" => kw("0 1px 2px 0 rgba(0, 0, 0, 0.05)") },
        "shadow" => {
            "box-shadow" => kw("0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1)"),
        },
        "shadow-md" => {
            "box-shadow" => kw("0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1)"),
        },
        "shadow-lg" => {
            "box-shadow" => kw("0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)"),
        },
        "shadow-xl" => {
            "box-shadow" => kw("0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)"),
        },
        "shadow-2xl" => {
            "box-shadow" => kw("0 25px 50px -12px rgba(0, 0, 0, 0.25)"),
        },
        "shadow-none" => { "box-shadow" => kw("none") },

        // --- Ring System ---
        "ring" => {
            "--tw-ring-width" => rem(0.1875),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-0" => {
            "--tw-ring-width" => px(0.0),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-1" => {
            "--tw-ring-width" => px(1.0),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-2" => {
            "--tw-ring-width" => px(2.0),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-4" => {
            "--tw-ring-width" => px(4.0),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-8" => {
            "--tw-ring-width" => px(8.0),
            "box-shadow" => kw(super::RING_BOX_SHADOW),
        },
        "ring-inset" => { "--tw-ring-inset" => kw("inset") },
        "ring-offset-0" => { "--tw-ring-offset-width" => px(0.0) },
        "ring-offset-1" => { "--tw-ring-offset-width" => px(1.0) },
        "ring-offset-2" => { "--tw-ring-offset-width" => px(2.0) },
        "ring-offset-4" => { "--tw-ring-offset-width" => px(4.0) },
        "ring-offset-8" => { "--tw-ring-offset-width" => px(8.0) },

        // --- Background Position & Repeat & Size ---
        "bg-bottom" => { "background-position" => kw("bottom") },
        "bg-center" => { "background-position" => kw("center") },
        "bg-left" => { "background-position" => kw("left") },
        "bg-left-bottom" => { "background-position" => kw("left bottom") },
        "bg-left-top" => { "background-position" => kw("left top") },
        "bg-right" => { "background-position" => kw("right") },
        "bg-right-bottom" => { "background-position" => kw("right bottom") },
        "bg-right-top" => { "background-position" => kw("right top") },
        "bg-top" => { "background-position" => kw("top") },

        "bg-repeat" => { "background-repeat" => kw("repeat") },
        "bg-no-repeat" => { "background-repeat" => kw("no-repeat") },
        "bg-repeat-x" => { "background-repeat" => kw("repeat-x") },
        "bg-repeat-y" => { "background-repeat" => kw("repeat-y") },
        "bg-repeat-round" => { "background-repeat" => kw("round") },
        "bg-repeat-space" => { "background-repeat" => kw("space") },

        "bg-auto" => { "background-size" => kw("auto") },
        "bg-cover" => { "background-size" => kw("cover") },
        "bg-contain" => { "background-size" => kw("contain") },

        // --- Gradients ---
        "bg-gradient-to-r" => { "background-image" => kw("linear-gradient(to right, var(--tw-gradient-stops))") },
        "bg-gradient-to-l" => { "background-image" => kw("linear-gradient(to left, var(--tw-gradient-stops))") },
        "bg-gradient-to-t" => { "background-image" => kw("linear-gradient(to top, var(--tw-gradient-stops))") },
        "bg-gradient-to-b" => { "background-image" => kw("linear-gradient(to bottom, var(--tw-gradient-stops))") },
        "bg-gradient-to-tr" => { "background-image" => kw("linear-gradient(to top right, var(--tw-gradient-stops))") },
        "bg-gradient-to-br" => { "background-image" => kw("linear-gradient(to bottom right, var(--tw-gradient-stops))") },
        "bg-gradient-to-tl" => { "background-image" => kw("linear-gradient(to top left, var(--tw-gradient-stops))") },
        "bg-gradient-to-bl" => { "background-image" => kw("linear-gradient(to bottom left, var(--tw-gradient-stops))") },
        "bg-none" => { "background-image" => kw("none") },

        // --- Aspect Ratio & Object Fit ---
        "aspect-auto" => { "aspect-ratio" => kw("auto") },
        "aspect-square" => { "aspect-ratio" => kw("1 / 1") },
        "aspect-video" => { "aspect-ratio" => kw("16 / 9") },
        "object-contain" => { "object-fit" => kw("contain") },
        "object-cover" => { "object-fit" => kw("cover") },
        "object-fill" => { "object-fit" => kw("fill") },
        "object-none" => { "object-fit" => kw("none") },
        "object-scale-down" => { "object-fit" => kw("scale-down") },
        "object-top" => { "object-position" => kw("top") },
        "object-bottom" => { "object-position" => kw("bottom") },
        "object-center" => { "object-position" => kw("center") },
        "object-left" => { "object-position" => kw("left") },
        "object-right" => { "object-position" => kw("right") },
        "object-left-top" => { "object-position" => kw("left top") },
        "object-left-bottom" => { "object-position" => kw("left bottom") },
        "object-right-top" => { "object-position" => kw("right top") },
        "object-right-bottom" => { "object-position" => kw("right bottom") },

        // --- Grid Spans & Rows ---
        "col-span-full" => { "grid-column" => kw("1 / -1") },
        "col-start-auto" => { "grid-column-start" => kw("auto") },
        "col-end-auto" => { "grid-column-end" => kw("auto") },
        "row-span-full" => { "grid-row" => kw("1 / -1") },
        "row-start-auto" => { "grid-row-start" => kw("auto") },
        "row-end-auto" => { "grid-row-end" => kw("auto") },
        "grid-rows-none" => { "grid-template-rows" => kw("none") },
        "grid-cols-none" => { "grid-template-columns" => kw("none") },
        "columns-auto" => { "column-count" => kw("auto") },
        "columns-3xs" => { "column-width" => rem(16.0) },
        "columns-2xs" => { "column-width" => rem(18.0) },
        "columns-xs" => { "column-width" => rem(20.0) },
        "columns-sm" => { "column-width" => rem(24.0) },
        "columns-md" => { "column-width" => rem(28.0) },
        "columns-lg" => { "column-width" => rem(32.0) },
        "columns-xl" => { "column-width" => rem(36.0) },
        "columns-2xl" => { "column-width" => rem(42.0) },
        "columns-3xl" => { "column-width" => rem(48.0) },
        "columns-4xl" => { "column-width" => rem(56.0) },
        "columns-5xl" => { "column-width" => rem(64.0) },
        "columns-6xl" => { "column-width" => rem(72.0) },
        "columns-7xl" => { "column-width" => rem(80.0) },

        // --- Break Inside / Before / After ---
        "break-inside-auto" => { "break-inside" => kw("auto") },
        "break-inside-avoid" => { "break-inside" => kw("avoid") },
        "break-inside-avoid-page" => { "break-inside" => kw("avoid-page") },
        "break-inside-avoid-column" => { "break-inside" => kw("avoid-column") },
        "break-inside-avoid-flex" => { "break-inside" => kw("avoid-flex") },
        "break-before-auto" => { "break-before" => kw("auto") },
        "break-before-avoid" => { "break-before" => kw("avoid") },
        "break-before-all" => { "break-before" => kw("all") },
        "break-before-avoid-page" => { "break-before" => kw("avoid-page") },
        "break-before-avoid-column" => { "break-before" => kw("avoid-column") },
        "break-before-page" => { "break-before" => kw("page") },
        "break-before-left" => { "break-before" => kw("left") },
        "break-before-right" => { "break-before" => kw("right") },
        "break-before-column" => { "break-before" => kw("column") },
        "break-after-auto" => { "break-after" => kw("auto") },
        "break-after-avoid" => { "break-after" => kw("avoid") },
        "break-after-all" => { "break-after" => kw("all") },
        "break-after-avoid-page" => { "break-after" => kw("avoid-page") },
        "break-after-avoid-column" => { "break-after" => kw("avoid-column") },
        "break-after-page" => { "break-after" => kw("page") },
        "break-after-left" => { "break-after" => kw("left") },
        "break-after-right" => { "break-after" => kw("right") },
        "break-after-column" => { "break-after" => kw("column") },

        // --- Box Decoration ---
        "box-decoration-slice" => { "box-decoration-break" => kw("slice") },
        "box-decoration-clone" => { "box-decoration-break" => kw("clone") },

        // --- Line Clamp & Text Overflow ---
        "truncate" => {
            "overflow" => kw("hidden"),
            "text-overflow" => kw("ellipsis"),
            "white-space" => kw("nowrap"),
        },
        "text-ellipsis" => { "text-overflow" => kw("ellipsis") },
        "text-clip" => { "text-overflow" => kw("clip") },
        "line-clamp-none" => {
            "overflow" => kw("visible"),
            "display" => kw("block"),
            "-webkit-box-orient" => kw("horizontal"),
            "-webkit-line-clamp" => kw("none"),
        },

        // --- Interactive & Pointer Events ---
        "pointer-events-none" => { "pointer-events" => kw("none") },
        "pointer-events-auto" => { "pointer-events" => kw("auto") },
        "select-none" => { "user-select" => kw("none") },
        "select-text" => { "user-select" => kw("text") },
        "select-all" => { "user-select" => kw("all") },
        "select-auto" => { "user-select" => kw("auto") },

        // --- Overflow ---
        "overflow-auto" => { "overflow" => kw("auto") },
        "overflow-hidden" => { "overflow" => kw("hidden") },
        "overflow-visible" => { "overflow" => kw("visible") },
        "overflow-scroll" => { "overflow" => kw("scroll") },
        "overflow-clip" => { "overflow" => kw("clip") },
        "overflow-x-auto" => { "overflow-x" => kw("auto") },
        "overflow-x-hidden" => { "overflow-x" => kw("hidden") },
        "overflow-x-visible" => { "overflow-x" => kw("visible") },
        "overflow-x-scroll" => { "overflow-x" => kw("scroll") },
        "overflow-x-clip" => { "overflow-x" => kw("clip") },
        "overflow-y-auto" => { "overflow-y" => kw("auto") },
        "overflow-y-hidden" => { "overflow-y" => kw("hidden") },
        "overflow-y-visible" => { "overflow-y" => kw("visible") },
        "overflow-y-scroll" => { "overflow-y" => kw("scroll") },
        "overflow-y-clip" => { "overflow-y" => kw("clip") },

        // --- Z-Index Presets ---
        "z-0" => { "z-index" => num_unitless(0.0) },
        "z-10" => { "z-index" => num_unitless(10.0) },
        "z-20" => { "z-index" => num_unitless(20.0) },
        "z-30" => { "z-index" => num_unitless(30.0) },
        "z-40" => { "z-index" => num_unitless(40.0) },
        "z-50" => { "z-index" => num_unitless(50.0) },
        "z-auto" => { "z-index" => kw("auto") },

        // --- Opacity Presets ---
        "opacity-0" => { "opacity" => num_unitless(0.0) },
        "opacity-5" => { "opacity" => num_unitless(0.05) },
        "opacity-10" => { "opacity" => num_unitless(0.1) },
        "opacity-20" => { "opacity" => num_unitless(0.2) },
        "opacity-25" => { "opacity" => num_unitless(0.25) },
        "opacity-30" => { "opacity" => num_unitless(0.3) },
        "opacity-40" => { "opacity" => num_unitless(0.4) },
        "opacity-50" => { "opacity" => num_unitless(0.5) },
        "opacity-60" => { "opacity" => num_unitless(0.6) },
        "opacity-70" => { "opacity" => num_unitless(0.7) },
        "opacity-75" => { "opacity" => num_unitless(0.75) },
        "opacity-80" => { "opacity" => num_unitless(0.8) },
        "opacity-90" => { "opacity" => num_unitless(0.9) },
        "opacity-95" => { "opacity" => num_unitless(0.95) },
        "opacity-100" => { "opacity" => num_unitless(1.0) },

        // --- Transition & Cursor ---
        "transition-all" => { "transition" => kw("all 150ms cubic-bezier(0.4, 0, 0.2, 1)") },
        "transition-colors" => {
            "transition" => kw("color, background-color, border-color, text-decoration-color, fill, stroke 150ms cubic-bezier(0.4, 0, 0.2, 1)"),
        },
        "transition" => {
            "transition" => kw("color, background-color, border-color, box-shadow, transform 150ms cubic-bezier(0.4, 0, 0.2, 1)"),
        },
        "cursor-auto" => { "cursor" => kw("auto") },
        "cursor-default" => { "cursor" => kw("default") },
        "cursor-pointer" => { "cursor" => kw("pointer") },
        "cursor-wait" => { "cursor" => kw("wait") },
        "cursor-text" => { "cursor" => kw("text") },
        "cursor-move" => { "cursor" => kw("move") },
        "cursor-not-allowed" => { "cursor" => kw("not-allowed") },

        // --- Animations & Will-Change ---
        "animate-spin" => {
            "animation" => kw("spin 1s linear infinite"),
            "will-change" => kw("transform"),
        },
        "animate-ping" => {
            "animation" => kw("ping 1s cubic-bezier(0, 0, 0.2, 1) infinite"),
            "will-change" => kw("transform, opacity"),
        },
        "animate-pulse" => {
            "animation" => kw("pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite"),
            "will-change" => kw("opacity"),
        },
        "animate-bounce" => {
            "animation" => kw("bounce 1s infinite"),
            "will-change" => kw("transform"),
        },
        "animate-none" => { "animation" => kw("none") },
        "will-change-transform" => { "will-change" => kw("transform") },
        "will-change-scroll" => { "will-change" => kw("scroll-position") },
        "will-change-auto" => { "will-change" => kw("auto") },

        // --- Filters & Backdrop Filters ---
        "blur-none" => { "filter" => kw("none") },
        "blur-sm" => { "filter" => kw("blur(4px)") },
        "blur" | "blur-md" => { "filter" => kw("blur(8px)") },
        "blur-lg" => { "filter" => kw("blur(16px)") },
        "blur-xl" => { "filter" => kw("blur(24px)") },
        "blur-2xl" => { "filter" => kw("blur(40px)") },
        "blur-3xl" => { "filter" => kw("blur(64px)") },

        "backdrop-blur-none" => { "backdrop-filter" => kw("none") },
        "backdrop-blur-sm" => { "backdrop-filter" => kw("blur(4px)") },
        "backdrop-blur" | "backdrop-blur-md" => { "backdrop-filter" => kw("blur(8px)") },
        "backdrop-blur-lg" => { "backdrop-filter" => kw("blur(16px)") },
        "backdrop-blur-xl" => { "backdrop-filter" => kw("blur(24px)") },
        "backdrop-blur-2xl" => { "backdrop-filter" => kw("blur(40px)") },
        "backdrop-blur-3xl" => { "backdrop-filter" => kw("blur(64px)") },

        // --- Transforms ---
        "translate-x-full" => { "transform" => kw("translateX(100%)") },
        "translate-y-full" => { "transform" => kw("translateY(100%)") },
        "-translate-x-full" => { "transform" => kw("translateX(-100%)") },
        "-translate-y-full" => { "transform" => kw("translateY(-100%)") },
    }
}
