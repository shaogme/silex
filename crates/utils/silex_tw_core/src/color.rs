//! 颜色词条解析——两侧共用（纯 OKLCH 原生 Raw 实现）。
//!
//! Tailwind CSS v4 原生基于 OKLCH 色彩空间。
//! 本模块实现原生的 OKLCH 解析、OKLCH 感知均匀插值与 `oklch(... / alpha)` 原生透明度拼接。

use std::borrow::Cow;

use crate::{
    context::TwContext,
    prefix::{COLOR_PREFIX_RULES, ColorPrefixRule},
    value::{TwDecl, TwRuleSet},
};

/// 格式化浮点数，移除无意义的结尾零
fn format_num_clean(val: f64) -> String {
    let s = format!("{:.4}", val);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// 解析 `oklch(L C H)` 格式的颜色为 `(l, c, h, alpha)`
pub fn parse_oklch(raw: &str) -> Option<(f64, f64, f64, Option<f64>)> {
    let inner = raw.strip_prefix("oklch(")?.strip_suffix(')')?;
    let (color_part, alpha_part) = match inner.split_once('/') {
        Some((c, a)) => (c.trim(), Some(a.trim())),
        None => (inner.trim(), None),
    };

    let mut tokens = color_part.split_whitespace();
    let l_str = tokens.next()?;
    let c_str = tokens.next()?;
    let h_str = tokens.next()?;

    let l = if let Some(pct) = l_str.strip_suffix('%') {
        pct.parse::<f64>().ok()? / 100.0
    } else {
        l_str.parse::<f64>().ok()?
    };

    let c = if c_str == "none" {
        0.0
    } else {
        c_str.parse::<f64>().ok()?
    };

    let h = if h_str == "none" {
        0.0
    } else {
        h_str.parse::<f64>().ok()?
    };

    let alpha = match alpha_part {
        Some(a) => {
            if let Some(pct) = a.strip_suffix('%') {
                Some(pct.parse::<f64>().ok()? / 100.0)
            } else {
                Some(a.parse::<f64>().ok()?)
            }
        }
        None => None,
    };

    Some((l, c, h, alpha))
}

/// 在 OKLCH 色彩空间对两个颜色做感知均匀插值
pub fn interpolate_oklch(raw1: &str, raw2: &str, t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let (l1, c1, h1, _) = parse_oklch(raw1).unwrap_or((1.0, 0.0, 0.0, None));
    let (l2, c2, h2, _) = parse_oklch(raw2).unwrap_or((0.0, 0.0, 0.0, None));

    let l = l1 + (l2 - l1) * t;
    let c = c1 + (c2 - c1) * t;

    // 色相沿 360 度圆环的最短路径插值
    let dh = ((h2 - h1 + 540.0) % 360.0) - 180.0;
    let mut h = h1 + dh * t;
    if h < 0.0 {
        h += 360.0;
    } else if h >= 360.0 {
        h %= 360.0;
    }

    let l_pct = (l * 100.0 * 1000.0).round() / 1000.0;
    let c_clean = (c * 10000.0).round() / 10000.0;
    let h_clean = (h * 1000.0).round() / 1000.0;

    format!(
        "oklch({}% {} {})",
        format_num_clean(l_pct),
        format_num_clean(c_clean),
        format_num_clean(h_clean)
    )
}

/// 对齐 Tailwind CSS v4 官方实现：将颜色基础值与透明度百分比（`0.0 ~ 100.0`）拼接为 `color-mix(in oklab, <color> <pct>%, transparent)`
pub fn apply_opacity(color_raw: &str, opacity_pct: f64) -> String {
    format!(
        "color-mix(in oklab, {} {}%, transparent)",
        color_raw,
        format_num_clean(opacity_pct)
    )
}

/// 标准色阶在梯度数组中的下标
const CHECKPOINTS: &[(u32, usize)] = &[
    (50, 0),
    (100, 1),
    (200, 2),
    (300, 3),
    (400, 4),
    (500, 5),
    (600, 6),
    (700, 7),
    (800, 8),
    (900, 9),
    (950, 10),
];

/// 查找标准或非标色阶的色板颜色。
///
/// 支持 1~1000 的任意色阶（`slate-850`、`indigo-25`、`red-975`）：
/// 标准档位直查，其余在相邻档位之间做 OKLCH 空间感知插值。
pub fn lookup_palette_color(ctx: &dyn TwContext, family: &str, shade: &str) -> Option<String> {
    if let Some(val) = ctx
        .config_color(&format!("{}-{}", family, shade))
        .or_else(|| ctx.config_color(family))
    {
        return Some(val);
    }

    if let Some(raw) = ctx.palette_shade(family, shade) {
        return Some(raw.to_string());
    }

    let ramp = ctx.palette_ramp(family)?;

    let target: u32 = shade.parse().ok()?;
    if target > 1000 {
        return None;
    }

    if target < 50 {
        return Some(interpolate_oklch(
            "oklch(100% 0 0)",
            ramp[0],
            target as f64 / 50.0,
        ));
    }
    if target > 950 {
        return Some(interpolate_oklch(
            ramp[10],
            "oklch(0% 0 0)",
            (target - 950) as f64 / 50.0,
        ));
    }

    CHECKPOINTS.windows(2).find_map(|w| {
        let ((s1, i1), (s2, i2)) = (w[0], w[1]);
        (target >= s1 && target <= s2).then(|| {
            let t = (target - s1) as f64 / (s2 - s1) as f64;
            interpolate_oklch(ramp[i1], ramp[i2], t)
        })
    })
}

/// 语义颜色 token（shadcn-ui 体系），映射到 `var(--<token>)`
const SEMANTIC_TOKENS: &[&str] = &[
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent",
    "accent-foreground",
    "destructive",
    "destructive-foreground",
    "border",
    "input",
    "ring",
];

/// 从颜色词条中剥离 `/<透明度>` 后缀。
fn split_opacity(token: &str) -> (&str, Option<f64>) {
    let Some((base, op_str)) = token.split_once('/') else {
        return (token, None);
    };
    let (raw, is_arbitrary) = match op_str.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(inner) => (inner, true),
        None => (op_str, false),
    };
    match raw.parse::<f64>() {
        Ok(op) if is_arbitrary && (0.0..=1.0).contains(&op) => (base, Some(op * 100.0)),
        Ok(op) => (base, Some(op)),
        Err(_) => (token, None),
    }
}

fn looks_like_color(s: &str) -> bool {
    let s = s.trim();
    s.starts_with('#')
        || s.starts_with("rgb(")
        || s.starts_with("rgba(")
        || s.starts_with("hsl(")
        || s.starts_with("hsla(")
        || s.starts_with("oklch(")
        || s.starts_with("oklab(")
        || s.starts_with("color(")
        || s == "white"
        || s == "black"
        || s == "transparent"
        || s == "currentColor"
        || s == "current"
        || s == "inherit"
}

/// 解析颜色值词条。
///
/// 支持：色板色阶（`slate-900`、`indigo-600/50`、非标的 `slate-850`）、
/// 关键字（`white` / `black` / `transparent` / `current` / `inherit`）、
/// 任意值以及语义 token（`primary` → `var(--primary)`）。
pub fn parse_color_value(ctx: &dyn TwContext, color_token: &str) -> Option<Cow<'static, str>> {
    let (base, opacity) = split_opacity(color_token);

    // 0. 用户 silex.toml 自定义颜色优先级最高
    if let Some(val) = ctx.config_color(base) {
        return Some(match opacity {
            Some(op) => Cow::Owned(apply_opacity(&val, op)),
            None => Cow::Owned(val),
        });
    }

    // 1. 颜色函数字面量与任意值
    if base.starts_with("rgba(")
        || base.starts_with("rgb(")
        || base.starts_with("hsl(")
        || base.starts_with("hsla(")
        || base.starts_with("oklch(")
    {
        return Some(match opacity {
            Some(op) => Cow::Owned(apply_opacity(base, op)),
            None => Cow::Owned(base.to_string()),
        });
    }

    // 2. 任意值包裹：`[#1e293b]` 或 `[oklch(...)]`
    if let Some(inner) = base.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
        && looks_like_color(inner)
    {
        return Some(match opacity {
            Some(op) => Cow::Owned(apply_opacity(inner, op)),
            None => Cow::Owned(inner.to_string()),
        });
    }

    // 3. 关键字颜色
    match base {
        "white" => {
            return Some(match opacity {
                Some(op) => Cow::Owned(apply_opacity("oklch(100% 0 0)", op)),
                None => Cow::Borrowed("white"),
            });
        }
        "black" => {
            return Some(match opacity {
                Some(op) => Cow::Owned(apply_opacity("oklch(0% 0 0)", op)),
                None => Cow::Borrowed("black"),
            });
        }
        "transparent" => return Some(Cow::Borrowed("transparent")),
        "current" => return Some(Cow::Borrowed("currentColor")),
        "inherit" => return Some(Cow::Borrowed("inherit")),
        _ => {}
    }

    // 4. 标准/插值色板
    if let Some((family, shade)) = base.rsplit_once('-')
        && let Some(raw) = lookup_palette_color(ctx, family, shade)
    {
        return Some(match opacity {
            Some(op) => Cow::Owned(apply_opacity(&raw, op)),
            None => Cow::Owned(raw),
        });
    }

    // 5. 语义 CSS 变量 token
    if SEMANTIC_TOKENS.contains(&base) {
        let var_expr = format!("var(--{})", base);
        return Some(Cow::Owned(match opacity {
            Some(op) => format!(
                "color-mix(in oklch, {} {}%, transparent)",
                var_expr,
                format_num_clean(op)
            ),
            None => var_expr,
        }));
    }

    None
}

/// 把命中的颜色前缀展开成完整声明（含伴生声明与伴生选择器）
fn expand(rule: &ColorPrefixRule, value: Cow<'static, str>) -> Vec<TwRuleSet> {
    let mut decls: Vec<TwDecl> = rule
        .props
        .iter()
        .map(|&p| TwDecl::new(p, value.clone()))
        .collect();

    if let Some(companion) = rule.companion {
        decls.extend(
            companion
                .decls()
                .iter()
                .map(|&(prop, val)| TwDecl::new(prop, val)),
        );
    }

    vec![TwRuleSet::scoped(rule.selector, decls)]
}

/// 解析颜色型工具类：`text-slate-900`、`bg-indigo-600/50`、`ring-blue-500`、`divide-red-500`。
pub fn resolve_color_utility(ctx: &dyn TwContext, class: &str) -> Option<Vec<TwRuleSet>> {
    COLOR_PREFIX_RULES.iter().find_map(|rule| {
        let rest = class.strip_prefix(rule.prefix)?;
        let value = parse_color_value(ctx, rest)?;
        Some(expand(rule, value))
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::context::TwContext;

    pub struct TestCtx;

    const SLATE: [&str; 11] = [
        "oklch(98.4% 0.003 247.858)",
        "oklch(96.8% 0.007 247.896)",
        "oklch(92.9% 0.013 255.508)",
        "oklch(86.9% 0.022 252.894)",
        "oklch(70.4% 0.04 256.788)",
        "oklch(55.4% 0.046 257.417)",
        "oklch(44.6% 0.043 257.281)",
        "oklch(37.2% 0.044 257.287)",
        "oklch(27.9% 0.041 260.031)",
        "oklch(20.8% 0.042 265.755)",
        "oklch(12.9% 0.042 264.695)",
    ];

    impl TwContext for TestCtx {
        fn palette_shade(&self, family: &str, shade: &str) -> Option<&str> {
            let idx = match shade {
                "50" => 0,
                "100" => 1,
                "200" => 2,
                "300" => 3,
                "400" => 4,
                "500" => 5,
                "600" => 6,
                "700" => 7,
                "800" => 8,
                "900" => 9,
                "950" => 10,
                _ => return None,
            };
            (family == "slate").then(|| SLATE[idx])
        }

        fn palette_ramp(&self, family: &str) -> Option<[&str; 11]> {
            (family == "slate").then_some(SLATE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::TestCtx, *};

    #[test]
    fn parses_oklch_values() {
        let (l, c, h, a) = parse_oklch("oklch(98.4% 0.003 247.858)").unwrap();
        assert!((l - 0.984).abs() < 1e-4);
        assert!((c - 0.003).abs() < 1e-4);
        assert!((h - 247.858).abs() < 1e-4);
        assert_eq!(a, None);

        let (_l, _c, _h, a) = parse_oklch("oklch(55.4% 0.046 257.417 / 50%)").unwrap();
        assert_eq!(a, Some(0.5));

        // 支持不带 % 的 L 与小数 alpha
        let (l, c, h, a) = parse_oklch("oklch(0.5 0.1 120 / 0.25)").unwrap();
        assert_eq!(l, 0.5);
        assert_eq!(c, 0.1);
        assert_eq!(h, 120.0);
        assert_eq!(a, Some(0.25));

        // 支持 none 关键字
        let (l, c, h, _a) = parse_oklch("oklch(50% none none)").unwrap();
        assert_eq!(l, 0.5);
        assert_eq!(c, 0.0);
        assert_eq!(h, 0.0);

        // 非 OKLCH 输入必须返回 None
        assert!(parse_oklch("rgb(255, 0, 0)").is_none());
        assert!(parse_oklch("invalid").is_none());
    }

    #[test]
    fn oklch_shortest_hue_interpolation() {
        // 从 350° 到 10°，最短弧度插值中点应该是 0° (或 360°)
        let c1 = "oklch(50% 0.1 350)";
        let c2 = "oklch(50% 0.1 10)";
        let mid = interpolate_oklch(c1, c2, 0.5);
        assert_eq!(mid, "oklch(50% 0.1 0)");

        // 从 10° 到 350°，最短弧度插值中点也应该是 0°
        let mid_reverse = interpolate_oklch(c2, c1, 0.5);
        assert_eq!(mid_reverse, "oklch(50% 0.1 0)");

        // 临界比例 t=0.0 与 t=1.0
        assert_eq!(interpolate_oklch(c1, c2, 0.0), "oklch(50% 0.1 350)");
        assert_eq!(interpolate_oklch(c1, c2, 1.0), "oklch(50% 0.1 10)");
    }

    #[test]
    fn interpolates_non_standard_shades_in_oklch() {
        let res = lookup_palette_color(&TestCtx, "slate", "850").unwrap();
        assert!(res.starts_with("oklch("));

        // 小于 50 与白色插值，大于 950 与黑色插值
        let res_sub50 = lookup_palette_color(&TestCtx, "slate", "25").unwrap();
        assert!(res_sub50.starts_with("oklch("));

        let res_over950 = lookup_palette_color(&TestCtx, "slate", "975").unwrap();
        assert!(res_over950.starts_with("oklch("));

        assert!(lookup_palette_color(&TestCtx, "nosuch", "500").is_none());
    }

    #[test]
    fn opacity_suffix_formats_native_oklch() {
        assert_eq!(
            apply_opacity("oklch(55.4% 0.046 257.417)", 50.0),
            "color-mix(in oklab, oklch(55.4% 0.046 257.417) 50%, transparent)"
        );
        // 对已有 /alpha 的 oklch 再次应用透明度，原样代入并叠加
        assert_eq!(
            apply_opacity("oklch(55.4% 0.046 257.417 / 0.8)", 50.0),
            "color-mix(in oklab, oklch(55.4% 0.046 257.417 / 0.8) 50%, transparent)"
        );

        // 关键字与自定义 CSS 变量处理
        assert_eq!(
            apply_opacity("var(--primary)", 50.0),
            "color-mix(in oklab, var(--primary) 50%, transparent)"
        );
        assert_eq!(
            apply_opacity("#1e293b", 50.0),
            "color-mix(in oklab, #1e293b 50%, transparent)"
        );

        assert_eq!(split_opacity("red-500/50"), ("red-500", Some(50.0)));
        assert_eq!(split_opacity("red-500/1"), ("red-500", Some(1.0)));
        assert_eq!(split_opacity("red-500/[0.75]"), ("red-500", Some(75.0)));
    }

    #[test]
    fn looks_like_color_discriminates_correctly() {
        // 判定为颜色的字面量
        assert!(looks_like_color("#fff"));
        assert!(looks_like_color("#1e293b80"));
        assert!(looks_like_color("oklch(50% 0.1 200)"));
        assert!(looks_like_color("rgb(0, 0, 0)"));
        assert!(looks_like_color("hsl(120, 50%, 50%)"));
        assert!(looks_like_color("white"));
        assert!(looks_like_color("black"));
        assert!(looks_like_color("transparent"));

        // 绝对不能判定为颜色的长度、数值或表达
        assert!(!looks_like_color("14px"));
        assert!(!looks_like_color("3px"));
        assert!(!looks_like_color("2rem"));
        assert!(!looks_like_color("100%"));
        assert!(!looks_like_color("calc(100% - 10px)"));
        assert!(!looks_like_color("auto"));
    }

    #[test]
    fn parses_keyword_and_palette_colors() {
        let c = &TestCtx;
        let val = |t: &str| parse_color_value(c, t).map(|v| v.into_owned());

        assert_eq!(
            val("slate-900").as_deref(),
            Some("oklch(20.8% 0.042 265.755)")
        );
        assert_eq!(val("white").as_deref(), Some("white"));
        assert_eq!(
            val("white/50").as_deref(),
            Some("color-mix(in oklab, oklch(100% 0 0) 50%, transparent)")
        );
        assert_eq!(val("black").as_deref(), Some("black"));
        assert_eq!(
            val("black/25").as_deref(),
            Some("color-mix(in oklab, oklch(0% 0 0) 25%, transparent)")
        );
        assert_eq!(val("transparent").as_deref(), Some("transparent"));
        assert_eq!(val("current").as_deref(), Some("currentColor"));
        assert_eq!(val("primary").as_deref(), Some("var(--primary)"));
        assert_eq!(
            val("primary/50").as_deref(),
            Some("color-mix(in oklch, var(--primary) 50%, transparent)")
        );
        assert_eq!(
            val("slate-500/50").as_deref(),
            Some("color-mix(in oklab, oklch(55.4% 0.046 257.417) 50%, transparent)")
        );

        // 任意 Hex 和任意 OKLCH 表达式
        assert_eq!(
            val("[#1e293b]/50").as_deref(),
            Some("color-mix(in oklab, #1e293b 50%, transparent)")
        );
        assert_eq!(
            val("[oklch(50%_0.1_200)]/80").as_deref(),
            Some("color-mix(in oklab, oklch(50%_0.1_200) 80%, transparent)")
        );

        // 尺寸与数值词条排斥测试
        assert!(val("2xl").is_none());
        assert!(val("4").is_none());
        assert!(val("[14px]").is_none());
        assert!(val("[3px]").is_none());
    }
}
