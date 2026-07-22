pub mod palette;
pub mod suggest;

use crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};
use proc_macro2::Span;
use syn::{Error, Result};

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
                $( $pat )|+ => Ok(vec![
                    $(
                        make_rule($modifiers.clone(), $prop, $val, $span),
                    )*
                ]),
            )*
            _ => resolve_pattern_utility($modifiers, $token, $span),
        }
    };
}

#[inline]
fn kw(s: &'static str) -> UtilityValue {
    UtilityValue::Keyword(s)
}

#[inline]
fn num(v: f64, u: &'static str) -> UtilityValue {
    UtilityValue::Numeric(v, u)
}

#[inline]
fn num_unitless(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "")
}

#[inline]
fn rem(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "rem")
}

#[inline]
fn px(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "px")
}

#[inline]
fn hex(s: &str) -> UtilityValue {
    UtilityValue::HexColor(s.to_string())
}

/// 将基础的 Utility 词条（如 `p-4`, `hover:bg-primary`, `w-[12px]`）解析为标准的 `UtilityRule`
pub fn resolve_utility(
    modifiers: Vec<Modifier>,
    utility_token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    // 1. 静态与预设规则精准匹配（不匹配则转由 resolve_pattern_utility 深入解析）
    resolve_rules! {
        modifiers, span, utility_token;

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

        // --- 容器查询 (Container Type & Name) ---
        "@container" | "container" | "container-inline-size" => { "container-type" => kw("inline-size") },
        "container-normal" => { "container-type" => kw("normal") },
        "container-size" => { "container-type" => kw("size") },

        // --- Flexbox方向与包裹 ---
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

        // --- Align Items & Justify Content ---
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

        // --- 颜色预设 ---
        "bg-transparent" => { "background-color" => kw("transparent") },
        "text-transparent" => { "color" => kw("transparent") },
        "border-transparent" => { "border-color" => kw("transparent") },
        "border-t-transparent" => { "border-top-color" => kw("transparent") },
        "bg-current" => { "background-color" => kw("currentColor") },
        "text-current" => { "color" => kw("currentColor") },
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
        "font-mono" => { "font-family" => kw("ui-monospace, monospace") },
        "font-sans" => { "font-family" => kw("ui-sans-serif, system-ui, sans-serif") },
        "tracking-tight" => { "letter-spacing" => num(-0.025, "em") },
        "tracking-wider" => { "letter-spacing" => num(0.05, "em") },
        "tracking-widest" => { "letter-spacing" => num(0.1, "em") },
        "leading-relaxed" => { "line-height" => num_unitless(1.625) },
        "min-h-screen" => { "min-height" => num(100.0, "vh") },
        "max-w-2xl" => { "max-width" => rem(42.0) },
        "max-w-5xl" => { "max-width" => rem(64.0) },
        "mx-auto" => {
            "margin-left" => kw("auto"),
            "margin-right" => kw("auto"),
        },

        "font-thin" => { "font-weight" => num_unitless(100.0) },
        "font-light" => { "font-weight" => num_unitless(300.0) },
        "font-normal" => { "font-weight" => num_unitless(400.0) },
        "font-medium" => { "font-weight" => num_unitless(500.0) },
        "font-semibold" => { "font-weight" => num_unitless(600.0) },
        "font-bold" => { "font-weight" => num_unitless(700.0) },
        "font-black" => { "font-weight" => num_unitless(900.0) },

        "text-xs" => {
            "font-size" => rem(0.75),
            "line-height" => rem(1.0),
        },
        "text-sm" => {
            "font-size" => rem(0.875),
            "line-height" => rem(1.25),
        },
        "text-base" => {
            "font-size" => rem(1.0),
            "line-height" => rem(1.5),
        },
        "text-lg" => {
            "font-size" => rem(1.125),
            "line-height" => rem(1.75),
        },
        "text-xl" => {
            "font-size" => rem(1.25),
            "line-height" => rem(1.75),
        },
        "text-2xl" => {
            "font-size" => rem(1.5),
            "line-height" => rem(2.0),
        },
        "text-3xl" => {
            "font-size" => rem(1.875),
            "line-height" => rem(2.25),
        },
        "text-4xl" => {
            "font-size" => rem(2.25),
            "line-height" => rem(2.5),
        },

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
        "shadow-none" => { "box-shadow" => kw("none") },

        // --- Transition & Cursor ---
        "transition-all" => { "transition" => kw("all 150ms cubic-bezier(0.4, 0, 0.2, 1)") },
        "transition-colors" => {
            "transition" => kw("color, background-color, border-color, text-decoration-color, fill, stroke 150ms cubic-bezier(0.4, 0, 0.2, 1)"),
        },
        "transition" => {
            "transition" => kw("color, background-color, border-color, box-shadow, transform 150ms cubic-bezier(0.4, 0, 0.2, 1)"),
        },
        "cursor-pointer" => { "cursor" => kw("pointer") },
        "cursor-default" => { "cursor" => kw("default") },
        "cursor-not-allowed" => { "cursor" => kw("not-allowed") },

        // --- Animations (Keyframes) & Will-Change ---
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
        "scale-0" => { "transform" => kw("scale(0)") },
        "scale-50" => { "transform" => kw("scale(0.5)") },
        "scale-75" => { "transform" => kw("scale(0.75)") },
        "scale-90" => { "transform" => kw("scale(0.9)") },
        "scale-95" => { "transform" => kw("scale(0.95)") },
        "scale-100" => { "transform" => kw("scale(1)") },
        "scale-105" => { "transform" => kw("scale(1.05)") },
        "scale-110" => { "transform" => kw("scale(1.1)") },
        "scale-125" => { "transform" => kw("scale(1.25)") },
        "scale-150" => { "transform" => kw("scale(1.5)") },

        "rotate-0" => { "transform" => kw("rotate(0deg)") },
        "rotate-45" => { "transform" => kw("rotate(45deg)") },
        "rotate-90" => { "transform" => kw("rotate(90deg)") },
        "rotate-180" => { "transform" => kw("rotate(180deg)") },
        "-rotate-45" => { "transform" => kw("rotate(-45deg)") },
        "-rotate-90" => { "transform" => kw("rotate(-90deg)") },
        "-rotate-180" => { "transform" => kw("rotate(-180deg)") },

        "translate-x-full" => { "transform" => kw("translateX(100%)") },
        "translate-y-full" => { "transform" => kw("translateY(100%)") },
        "-translate-x-full" => { "transform" => kw("translateX(-100%)") },
        "-translate-y-full" => { "transform" => kw("translateY(-100%)") },
        "translate-x-0" => { "transform" => kw("translateX(0)") },
        "translate-y-0" => { "transform" => kw("translateY(0)") },
    }
}

macro_rules! resolve_numeric_rules {
    (
        $prefix:expr, $modifiers:expr, $span:expr, $default_val:expr;
        $(
            $pat:pat => $target:tt
        ),* $(,)?
    ) => {
        match $prefix {
            $(
                $pat => return Ok(resolve_numeric_rules!(@expand $modifiers, $span, $default_val; $target)),
            )*
            _ => {}
        }
    };

    (@expand $modifiers:expr, $span:expr, $default_val:expr; $prop:literal) => {
        vec![make_rule($modifiers, $prop, $default_val, $span)]
    };

    (@expand $modifiers:expr, $span:expr, $default_val:expr; [ $p1:literal, $p2:literal ]) => {
        vec![
            make_rule($modifiers.clone(), $p1, $default_val.clone(), $span),
            make_rule($modifiers, $p2, $default_val, $span),
        ]
    };

    (@expand $modifiers:expr, $span:expr, $default_val:expr; { $prop:expr => $val:expr }) => {
        vec![make_rule($modifiers, $prop, $val, $span)]
    };
}

#[inline]
fn color_prefix_to_prop(prefix: &str) -> Option<&'static str> {
    match prefix {
        "bg" => Some("background-color"),
        "text" => Some("color"),
        "border" => Some("border-color"),
        "border-t" => Some("border-top-color"),
        "border-r" => Some("border-right-color"),
        "border-b" => Some("border-bottom-color"),
        "border-l" => Some("border-left-color"),
        "outline" => Some("outline-color"),
        "ring" => Some("outline-color"),
        "fill" => Some("fill"),
        "stroke" => Some("stroke"),
        _ => None,
    }
}

/// 解析前缀规律型 Utility (如 `p-4`, `mt-2`, `w-16`, `bg-theme(primary)`, `text-slate-900`, `bg-indigo-600/50`, `w-[12px]`)
fn resolve_pattern_utility(
    modifiers: Vec<Modifier>,
    token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    // 1. Theme 变量, 如 `bg-theme(primary)` / `text-theme(border)` / `bg-theme(primary/50)`
    if let Some((prefix, theme_var, opacity)) = parse_theme_var(token) {
        if let Some(prop) = color_prefix_to_prop(prefix) {
            return Ok(vec![make_rule(
                modifiers,
                prop,
                UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                span,
            )]);
        }
        return Err(Error::new(
            span,
            format!("Unsupported theme prefix: '{}'", prefix),
        ));
    }

    // 2. Phase 5: Tailwind 标准 Palette 色系、Hex 颜色与 /alpha 透明度后缀换算
    // 支持 `text-slate-900`, `bg-indigo-600/50`, `border-emerald-500/25`, `bg-[#1e293b]/80`, `bg-white/50`
    if let Some((prop, val)) = palette::parse_color_utility(token) {
        return Ok(vec![make_rule(modifiers, prop, val, span)]);
    }

    // 3. 任意值与动态表达式, 如 `w-[100px]` 或 `p-[$(pad_val)]`
    if let Some((prefix, raw_val)) = parse_arbitrary_syntax(token) {
        return resolve_arbitrary(modifiers, prefix, raw_val, span);
    }

    // 4. 数值前缀工具类 System: `p-{n}`, `px-{n}`, `m-{n}`, `w-{n}`, `gap-{n}`, `opacity-{n}` 等
    let is_negative = token.starts_with('-');
    let search_token = if is_negative { &token[1..] } else { token };

    if let Some((prefix, val_str)) = search_token.rsplit_once('-')
        && let Ok(n) = val_str.parse::<f64>()
    {
        let scale = if is_negative { -0.25 } else { 0.25 };
        let rem_val = n * scale;
        let val = UtilityValue::Numeric(rem_val, "rem");

        resolve_numeric_rules! {
            prefix, modifiers, span, val;

            // 单属性映射 (默认使用计算后的 rem 数值)
            "p"  => "padding",
            "pt" => "padding-top",
            "pr" => "padding-right",
            "pb" => "padding-bottom",
            "pl" => "padding-left",
            "m"  => "margin",
            "mt" => "margin-top",
            "mr" => "margin-right",
            "mb" => "margin-bottom",
            "ml" => "margin-left",
            "gap" => "gap",
            "w"  => "width",
            "h"  => "height",

            // 双属性对称映射 (默认使用计算后的 rem 数值)
            "px"   => ["padding-left", "padding-right"],
            "py"   => ["padding-top", "padding-bottom"],
            "mx"   => ["margin-left", "margin-right"],
            "my"   => ["margin-top", "margin-bottom"],
            "size" => ["width", "height"],

            // 自定义计算/转换规则
            "grid-cols"   => { "grid-template-columns" => UtilityValue::ArbitraryLiteral(format!("repeat({}, minmax(0, 1fr))", n as usize)) },
            "opacity"     => { "opacity" => UtilityValue::Numeric(n / 100.0, "") },
            "duration"    => { "transition-duration" => UtilityValue::Numeric(n, "ms") },
            "rotate"      => { "transform" => UtilityValue::ArbitraryLiteral(format!("rotate({}deg)", if is_negative { -n } else { n })) },
            "scale"       => { "transform" => UtilityValue::ArbitraryLiteral(format!("scale({})", n / 100.0)) },
            "translate-x" => { "transform" => UtilityValue::ArbitraryLiteral(format!("translateX({}rem)", rem_val)) },
            "translate-y" => { "transform" => UtilityValue::ArbitraryLiteral(format!("translateY({}rem)", rem_val)) },
        }
    }

    let suggestion = suggest::find_best_suggestion(token);
    let msg = match suggestion {
        Some(s) => format!(
            "Unknown or unsupported Utility class '{}'. Did you mean '{}'?",
            token, s
        ),
        None => format!("Unknown or unsupported Utility class '{}'.", token),
    };

    Err(Error::new(span, msg))
}

/// 任意值语法解析: `w-[12px]`, `bg-[red]`
fn parse_arbitrary_syntax(token: &str) -> Option<(&str, &str)> {
    if let Some(open_idx) = token.find('[')
        && token.ends_with(']')
        && open_idx > 0
    {
        let prefix = &token[..open_idx];
        let raw_val = &token[open_idx + 1..token.len() - 1];
        return Some((prefix, raw_val));
    }
    None
}

/// 解析任意值到 UtilityRule
fn resolve_arbitrary(
    modifiers: Vec<Modifier>,
    prefix: &str,
    raw_val: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let clean_prefix = prefix.strip_suffix('-').unwrap_or(prefix);
    let prop = match clean_prefix {
        "p" | "padding" => "padding",
        "px" => "padding-inline",
        "py" => "padding-block",
        "pt" => "padding-top",
        "pr" => "padding-right",
        "pb" => "padding-bottom",
        "pl" => "padding-left",
        "m" | "margin" => "margin",
        "mx" => "margin-inline",
        "my" => "margin-block",
        "mt" => "margin-top",
        "mr" => "margin-right",
        "mb" => "margin-bottom",
        "ml" => "margin-left",
        "w" | "width" => "width",
        "h" | "height" => "height",
        "bg" => "background-color",
        "text" => "color",
        "border" => "border-color",
        "rounded" => "border-radius",
        "top" => "top",
        "right" => "right",
        "bottom" => "bottom",
        "left" => "left",
        "z" => "z-index",
        "opacity" => "opacity",
        "blur" => "filter",
        "backdrop-blur" => "backdrop-filter",
        "scale" | "scale-x" | "scale-y" | "rotate" | "translate-x" | "translate-y" => "transform",
        "animate" => "animation",
        "container" | "container-name" => "container-name",
        _ => clean_prefix,
    };

    if raw_val.starts_with("$(") && raw_val.ends_with(')') {
        let expr_inner = &raw_val[2..raw_val.len() - 1];
        let expr: syn::Expr =
            syn::parse_str(expr_inner).map_err(|e| Error::new(span, e.to_string()))?;
        return Ok(vec![make_rule(
            modifiers,
            prop,
            UtilityValue::DynamicExpr(expr, span),
            span,
        )]);
    }

    let val_str = raw_val.to_string();
    Ok(vec![make_rule(
        modifiers,
        prop,
        UtilityValue::ArbitraryLiteral(val_str),
        span,
    )])
}

fn parse_theme_var(token: &str) -> Option<(&str, &str, Option<f64>)> {
    if let Some((prefix, rest)) = token.split_once("-theme(")
        && let Some(inner) = rest.strip_suffix(')')
    {
        if let Some((var_name, op_str)) = inner.split_once('/')
            && let Ok(op) = op_str.parse::<f64>()
        {
            return Some((prefix, var_name, Some(op)));
        }
        return Some((prefix, inner, None));
    }
    None
}

fn make_rule(modifiers: Vec<Modifier>, prop: &str, value: UtilityValue, span: Span) -> UtilityRule {
    UtilityRule {
        modifiers,
        css_property: prop.to_string(),
        value,
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn test_resolve_pattern_numeric_rules() {
        let span = Span::call_site();

        // 1. 单属性规则 (rem 缩放)
        let rules = resolve_pattern_utility(vec![], "p-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "padding");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.0, "rem"));

        let rules = resolve_pattern_utility(vec![], "-mt-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "margin-top");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-0.5, "rem"));

        // 2. 双属性规则 (对称方向)
        let rules = resolve_pattern_utility(vec![], "px-6", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "padding-left");
        assert_eq!(rules[1].css_property, "padding-right");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.5, "rem"));
        assert_eq!(rules[1].value, UtilityValue::Numeric(1.5, "rem"));

        let rules = resolve_pattern_utility(vec![], "size-8", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(rules[1].css_property, "height");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, "rem"));

        // 3. 自定义数值计算与转换规则
        let rules = resolve_pattern_utility(vec![], "grid-cols-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "grid-template-columns");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("repeat(4, minmax(0, 1fr))".into())
        );

        let rules = resolve_pattern_utility(vec![], "opacity-50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "opacity");
        assert_eq!(rules[0].value, UtilityValue::Numeric(0.5, ""));

        let rules = resolve_pattern_utility(vec![], "rotate-45", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rotate(45deg)".into())
        );

        let rules = resolve_pattern_utility(vec![], "-rotate-90", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rotate(-90deg)".into())
        );

        let rules = resolve_pattern_utility(vec![], "bg-theme(primary)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), None)
        );

        let rules = resolve_pattern_utility(vec![], "bg-theme(primary/50)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), Some(50.0))
        );

        // 4. Hex 颜色解析规则
        let rules = resolve_utility(vec![], "bg-[#1e293b]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#1e293b".into()));

        let rules = resolve_utility(vec![], "text-[#818cf8]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#818cf8".into()));

        // 5. 通用任意值语法解析规则
        let rules = resolve_utility(vec![], "w-[100px]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("100px".into())
        );

        // 6. Levenshtein 拼写纠错测试
        let err = resolve_utility(vec![], "flexx", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'flex'?"));

        let err = resolve_utility(vec![], "items-centerr", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'items-center'?"));

        // 7. Phase 4: Container Query Utilities
        let rules = resolve_utility(vec![], "@container", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-type");
        assert_eq!(rules[0].value, UtilityValue::Keyword("inline-size"));

        let rules = resolve_utility(vec![], "container-[sidebar]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-name");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("sidebar".into())
        );

        // 8. Phase 5: Standard Color Palette & Opacity Suffix Rules
        let rules = resolve_utility(vec![], "text-slate-900", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#0f172a".into()));

        let rules = resolve_utility(vec![], "bg-indigo-600/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(79, 70, 229, 0.5)".into())
        );

        let rules = resolve_utility(vec![], "border-emerald-500/25", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(16, 185, 129, 0.25)".into())
        );

        let rules = resolve_utility(vec![], "border-t-rose-500", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#f43f5e".into()));

        let rules = resolve_utility(vec![], "bg-white/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(255, 255, 255, 0.5)".into())
        );
    }
}
