//! 颜色词条解析——两侧共用。
//!
//! 此前 codegen 侧只有"色系名 + 标准色阶"的直查（`resolve_color_hex`），
//! macro 侧另有一份支持非标色阶插值、`/透明度`、语义 token 的实现。
//! 因为静态表优先命中，凡是标准调色板颜色走的都是 codegen 那份，
//! macro 那份只在冷门路径上生效——两份规则事实上谁也说不清对哪些输入负责。
//! 现在合成这一份，由 [`crate::context::TwContext`] 注入色板后端。

use std::borrow::Cow;

use crate::{
    context::TwContext,
    prefix::{COLOR_PREFIX_RULES, ColorPrefixRule},
    value::{TwDecl, TwRuleSet},
};

/// 解析 Hex 色值为 `(r, g, b)`（支持 3 / 4 / 6 / 8 位）
fn parse_hex_rgb(hex: &str) -> (u8, u8, u8) {
    let clean = hex.strip_prefix('#').unwrap_or(hex);
    match clean.len() {
        6 | 8 => (
            u8::from_str_radix(&clean[0..2], 16).unwrap_or(0),
            u8::from_str_radix(&clean[2..4], 16).unwrap_or(0),
            u8::from_str_radix(&clean[4..6], 16).unwrap_or(0),
        ),
        3 | 4 => (
            u8::from_str_radix(&clean[0..1], 16).unwrap_or(0) * 17,
            u8::from_str_radix(&clean[1..2], 16).unwrap_or(0) * 17,
            u8::from_str_radix(&clean[2..3], 16).unwrap_or(0) * 17,
        ),
        _ => (0, 0, 0),
    }
}

/// 在两个 Hex 色值之间按比例 `t`（0.0..=1.0）做 RGB 线性插值
fn interpolate_hex(hex1: &str, hex2: &str, t: f64) -> String {
    let (r1, g1, b1) = parse_hex_rgb(hex1);
    let (r2, g2, b2) = parse_hex_rgb(hex2);

    let t = t.clamp(0.0, 1.0);
    let r = (r1 as f64 + (r2 as f64 - r1 as f64) * t).round() as u8;
    let g = (g1 as f64 + (g2 as f64 - g1 as f64) * t).round() as u8;
    let b = (b1 as f64 + (b2 as f64 - b1 as f64) * t).round() as u8;

    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// 将 Hex 颜色与透明度百分比转换为 `rgba(...)`（支持 3 / 4 / 6 / 8 位 Hex）
pub fn hex_to_rgba(hex: &str, alpha_pct: f64) -> String {
    let clean = hex.strip_prefix('#').unwrap_or(hex);
    let alpha = (alpha_pct / 100.0).clamp(0.0, 1.0);

    let (r, g, b) = match clean.len() {
        6 | 8 | 3 | 4 => parse_hex_rgb(hex),
        _ => return hex.to_string(),
    };

    let alpha_str = if (alpha * 100.0).round() == alpha * 100.0 {
        format!("{:.2}", alpha)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        format!("{:.3}", alpha)
    };
    format!("rgba({}, {}, {}, {})", r, g, b, alpha_str)
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
/// 标准档位直查，其余在相邻档位之间做 RGB 线性插值，
/// 小于 50 与 `#ffffff` 外插、大于 950 与 `#000000` 外插。
pub fn lookup_palette_color(ctx: &dyn TwContext, family: &str, shade: &str) -> Option<String> {
    if let Some(val) = ctx
        .config_color(&format!("{}-{}", family, shade))
        .or_else(|| ctx.config_color(family))
    {
        return Some(val);
    }

    if let Some(hex) = ctx.palette_shade(family, shade) {
        return Some(hex.to_string());
    }

    let ramp = ctx.palette_ramp(family)?;

    let target: u32 = shade.parse().ok()?;
    if target > 1000 {
        return None;
    }

    if target < 50 {
        return Some(interpolate_hex("#ffffff", ramp[0], target as f64 / 50.0));
    }
    if target > 950 {
        return Some(interpolate_hex(
            ramp[10],
            "#000000",
            (target - 950) as f64 / 50.0,
        ));
    }

    CHECKPOINTS.windows(2).find_map(|w| {
        let ((s1, i1), (s2, i2)) = (w[0], w[1]);
        (target >= s1 && target <= s2).then(|| {
            let t = (target - s1) as f64 / (s2 - s1) as f64;
            interpolate_hex(ramp[i1], ramp[i2], t)
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
///
/// Tailwind 语义：`/<整数>` 一律按**百分比**（`/1` = 1%，不是 100%），
/// 小数形式必须写成任意值 `/[0.5]`。报告 §2.6 记录了此前把 `/1` 当作 100% 的缺陷。
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

/// 把 Hex 与可选透明度收敛成最终值文本
fn hex_value(hex: &str, opacity: Option<f64>) -> Cow<'static, str> {
    match opacity {
        Some(op) => Cow::Owned(hex_to_rgba(hex, op)),
        None => Cow::Owned(hex.to_string()),
    }
}

/// 解析颜色值词条。
///
/// 支持：色板色阶（`slate-900`、`indigo-600/50`、非标的 `slate-850`）、
/// 关键字（`white` / `black` / `transparent` / `current` / `inherit`）、
/// 任意 Hex（`[#1e293b]`）、颜色函数（`rgb()` / `rgba()` / `hsl()` / `hsla()`）、
/// 以及语义 token（`primary` → `var(--primary)`）。
pub fn parse_color_value(ctx: &dyn TwContext, color_token: &str) -> Option<Cow<'static, str>> {
    let (base, opacity) = split_opacity(color_token);

    // 0. 用户 silex.toml 自定义颜色优先级最高
    if let Some(val) = ctx.config_color(base) {
        return Some(if val.starts_with('#') {
            hex_value(&val, opacity)
        } else {
            Cow::Owned(val)
        });
    }

    // 1. 颜色函数字面量原样透传
    if base.starts_with("rgba(")
        || base.starts_with("rgb(")
        || base.starts_with("hsl(")
        || base.starts_with("hsla(")
    {
        return Some(Cow::Owned(base.to_string()));
    }

    // 2. 任意 Hex：`[#1e293b]`。显式 `/透明度` 会覆盖 8 位 Hex 自带的 alpha 通道。
    if let Some(hex) = base.strip_prefix("[#").and_then(|s| s.strip_suffix(']')) {
        return Some(hex_value(&format!("#{}", hex), opacity));
    }

    // 3. 关键字颜色
    match base {
        "white" => return Some(hex_value("#ffffff", opacity)),
        "black" => return Some(hex_value("#000000", opacity)),
        // 透明色叠任何透明度仍然是全透明
        "transparent" => {
            return Some(match opacity {
                Some(_) => Cow::Owned(hex_to_rgba("#000000", 0.0)),
                None => Cow::Borrowed("transparent"),
            });
        }
        "current" => return Some(Cow::Borrowed("currentColor")),
        "inherit" => return Some(Cow::Borrowed("inherit")),
        _ => {}
    }

    // 4. 标准/插值色板
    if let Some((family, shade)) = base.rsplit_once('-')
        && let Some(hex) = lookup_palette_color(ctx, family, shade)
    {
        return Some(hex_value(&hex, opacity));
    }

    // 5. 语义 CSS 变量 token
    if SEMANTIC_TOKENS.contains(&base) {
        let var_expr = format!("var(--{})", base);
        return Some(Cow::Owned(match opacity {
            Some(op) => format!("color-mix(in srgb, {} {}%, transparent)", var_expr, op),
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
///
/// 前缀按 [`COLOR_PREFIX_RULES`] 的顺序尝试；前缀匹配上但后半段不是合法颜色时
/// （`border-2`、`ring-2`、`divide-x-2`）继续试下一个前缀，全部落空则返回 `None`
/// 交给后续的尺寸/数值路径处理。
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

    /// 测试用的极小色板：只有 slate 一族的完整 11 阶
    pub struct TestCtx;

    const SLATE: [&str; 11] = [
        "#f8fafc", "#f1f5f9", "#e2e8f0", "#cad5e2", "#90a1b9", "#62748e", "#45556c", "#314158",
        "#1d293d", "#0f172b", "#020618",
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
    fn parses_hex_of_every_length() {
        assert_eq!(parse_hex_rgb("#ffffff"), (255, 255, 255));
        assert_eq!(parse_hex_rgb("#fff"), (255, 255, 255));
        assert_eq!(parse_hex_rgb("#123"), (0x11, 0x22, 0x33));
        assert_eq!(parse_hex_rgb("#1e293b80"), (0x1e, 0x29, 0x3b));
    }

    #[test]
    fn interpolates_non_standard_shades() {
        assert_eq!(interpolate_hex("#fff", "#000", 0.5), "#808080");
        assert_eq!(
            lookup_palette_color(&TestCtx, "slate", "850").as_deref(),
            Some("#162034")
        );
        assert!(lookup_palette_color(&TestCtx, "slate", "25").is_some());
        assert!(lookup_palette_color(&TestCtx, "slate", "975").is_some());
        assert!(lookup_palette_color(&TestCtx, "nosuch", "500").is_none());
    }

    /// 报告 §2.6：`/1` 是 1%，不是 100%。小数只能走 `/[0.5]`。
    #[test]
    fn opacity_suffix_is_always_a_percentage() {
        assert_eq!(split_opacity("red-500/1"), ("red-500", Some(1.0)));
        assert_eq!(split_opacity("red-500/50"), ("red-500", Some(50.0)));
        assert_eq!(split_opacity("red-500/[0.5]"), ("red-500", Some(50.0)));
        assert_eq!(split_opacity("red-500"), ("red-500", None));
    }

    #[test]
    fn parses_keyword_hex_and_palette_colors() {
        let c = &TestCtx;
        let val = |t: &str| parse_color_value(c, t).map(|v| v.into_owned());

        assert_eq!(val("slate-900").as_deref(), Some("#0f172b"));
        assert_eq!(
            val("[#fff]/50").as_deref(),
            Some("rgba(255, 255, 255, 0.5)")
        );
        assert_eq!(
            val("[#1e293b80]/50").as_deref(),
            Some("rgba(30, 41, 59, 0.5)")
        );
        assert_eq!(val("transparent").as_deref(), Some("transparent"));
        assert_eq!(val("current").as_deref(), Some("currentColor"));
        assert_eq!(val("primary").as_deref(), Some("var(--primary)"));
        assert_eq!(
            val("slate-500/50").as_deref(),
            Some("rgba(98, 116, 142, 0.5)")
        );

        // 尺寸/档位词条不能被误判成颜色，否则 `ring-2`、`text-2xl` 会被颜色路径吃掉
        assert!(val("2xl").is_none());
        assert!(val("4").is_none());
        assert!(val("x-2").is_none());
    }

    /// 前缀命中但后半段不是颜色时必须继续往下试，而不是就此返回 `None`
    #[test]
    fn non_color_suffixes_fall_through_to_later_paths() {
        let c = &TestCtx;
        assert!(resolve_color_utility(c, "ring-2").is_none());
        assert!(resolve_color_utility(c, "border-2").is_none());
        assert!(resolve_color_utility(c, "divide-x-2").is_none());
        assert!(resolve_color_utility(c, "text-2xl").is_none());
        assert!(resolve_color_utility(c, "bg-linear-to-r").is_none());
    }

    /// `ring-*` 颜色必须连带铺 `box-shadow` 载体，否则颜色变量无处可用（报告 §2.4）
    #[test]
    fn ring_color_carries_the_box_shadow_companion() {
        let sets = resolve_color_utility(&TestCtx, "ring-slate-500").unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].selector, None);
        assert_eq!(
            sets[0].decls,
            vec![
                TwDecl::new("--tw-ring-color", "#62748e"),
                TwDecl::new("box-shadow", crate::prefix::RING_BOX_SHADOW),
            ]
        );
    }

    /// Silex 把渐变方向内联进了 `linear-gradient(to right, var(--tw-gradient-stops))`，
    /// 因此 `from-*` 必须自己拼出 `--tw-gradient-stops`，否则整条 background-image 无效。
    #[test]
    fn gradient_stops_are_emitted_by_the_color_stop_utilities() {
        let sets = resolve_color_utility(&TestCtx, "from-slate-500").unwrap();
        let props: Vec<_> = sets[0].decls.iter().map(|d| d.prop).collect();
        assert_eq!(
            props,
            vec![
                "--tw-gradient-from",
                "--tw-gradient-to",
                "--tw-gradient-stops"
            ]
        );

        let sets = resolve_color_utility(&TestCtx, "via-slate-500").unwrap();
        let props: Vec<_> = sets[0].decls.iter().map(|d| d.prop).collect();
        assert_eq!(props, vec!["--tw-gradient-via", "--tw-gradient-stops"]);
    }

    /// `divide-*` / `placeholder-*` 的声明不落在元素自身
    #[test]
    fn scoped_prefixes_carry_their_companion_selector() {
        let sets = resolve_color_utility(&TestCtx, "divide-slate-200").unwrap();
        assert_eq!(sets[0].selector, Some(crate::prefix::DIVIDE_SELECTOR));
        assert_eq!(sets[0].decls, vec![TwDecl::new("border-color", "#e2e8f0")]);

        let sets = resolve_color_utility(&TestCtx, "placeholder-slate-400").unwrap();
        assert_eq!(sets[0].selector, Some("&::placeholder"));
        assert_eq!(sets[0].decls, vec![TwDecl::new("color", "#90a1b9")]);
    }
}
