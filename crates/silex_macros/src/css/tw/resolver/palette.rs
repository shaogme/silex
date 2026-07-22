use std::borrow::Cow;

use crate::css::tw::ast::UtilityValue;

macro_rules! get_raw_palette {
    (
        $color_name:expr;
        $(
            $color:literal => [
                $c50:literal, $c100:literal, $c200:literal, $c300:literal, $c400:literal,
                $c500:literal, $c600:literal, $c700:literal, $c800:literal, $c900:literal, $c950:literal
            ]
        ),* $(,)?
    ) => {
        match $color_name {
            $(
                $color => Some([
                    $c50, $c100, $c200, $c300, $c400,
                    $c500, $c600, $c700, $c800, $c900, $c950
                ]),
            )*
            _ => None,
        }
    };
}

/// 解析 Hex 色值为 (r, g, b)（支持 3, 4, 6, 8 位 Hex）
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

/// 在两个 Hex 色值之间按照比例 t (0.0..=1.0) 进行 RGB 线性插值
fn interpolate_hex(hex1: &str, hex2: &str, t: f64) -> String {
    let (r1, g1, b1) = parse_hex_rgb(hex1);
    let (r2, g2, b2) = parse_hex_rgb(hex2);

    let t = t.clamp(0.0, 1.0);
    let r = (r1 as f64 + (r2 as f64 - r1 as f64) * t).round() as u8;
    let g = (g1 as f64 + (g2 as f64 - g1 as f64) * t).round() as u8;
    let b = (b1 as f64 + (b2 as f64 - b1 as f64) * t).round() as u8;

    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// 查找标准或非标色阶 Tailwind 色板颜色 (支持 1~999 任意数值内插，如 slate-850, indigo-25, red-975)
pub fn lookup_palette_color(color_name: &str, shade: &str) -> Option<Cow<'static, str>> {
    let shades_array = get_raw_palette! {
        color_name;
        "slate"   => ["#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155", "#1e293b", "#0f172a", "#020617"],
        "gray"    => ["#f9fafb", "#f3f4f6", "#e5e7eb", "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151", "#1f2937", "#111827", "#030712"],
        "zinc"    => ["#fafafa", "#f4f4f5", "#e4e4e7", "#d4d4d8", "#a1a1aa", "#71717a", "#52525b", "#3f3f46", "#27272a", "#18181b", "#09090b"],
        "neutral" => ["#fafafa", "#f5f5f5", "#e5e5e5", "#d4d4d4", "#a3a3a3", "#737373", "#525252", "#404040", "#262626", "#171717", "#0a0a0a"],
        "stone"   => ["#fafaf9", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#a8a29e", "#78716c", "#57534e", "#44403c", "#292524", "#1c1917", "#0c0a09"],
        "red"     => ["#fef2f2", "#fee2e2", "#fecaca", "#fca5a5", "#f87171", "#ef4444", "#dc2626", "#b91c1c", "#991b1b", "#7f1d1d", "#450a0a"],
        "orange"  => ["#fff7ed", "#ffedd5", "#fed7aa", "#fdba74", "#fb923c", "#f97316", "#ea580c", "#c2410c", "#9a3412", "#7c2d12", "#431407"],
        "amber"   => ["#fffbeb", "#fef3c7", "#fde68a", "#fcd34d", "#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e", "#78350f", "#451a03"],
        "yellow"  => ["#fefce8", "#fef9c3", "#fef08a", "#fde047", "#facc15", "#eab308", "#ca8a04", "#a16207", "#854d0e", "#713f12", "#422006"],
        "lime"    => ["#f7fee7", "#ecfccb", "#d9f99d", "#bef264", "#a3e635", "#84cc16", "#65a30d", "#4d7c0f", "#3f6212", "#365314", "#1a2e05"],
        "green"   => ["#f0fdf4", "#dcfce7", "#bbf7d0", "#86efac", "#4ade80", "#22c55e", "#16a34a", "#15803d", "#166534", "#14532d", "#052e16"],
        "emerald" => ["#ecfdf5", "#d1fae5", "#a7f3d0", "#6ee7b7", "#34d399", "#10b981", "#059669", "#047857", "#065f46", "#064e3b", "#022c22"],
        "teal"    => ["#f0fdfa", "#ccfbf1", "#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e", "#115e59", "#134e4a", "#042f2e"],
        "cyan"    => ["#ecfeff", "#cffafe", "#a5f3fc", "#67e8f9", "#22d3ee", "#06b6d4", "#0891b2", "#0e7490", "#155e75", "#164e63", "#083344"],
        "sky"     => ["#f0f9ff", "#e0f2fe", "#bae6fd", "#7dd3fc", "#38bdf8", "#0ea5e9", "#0284c7", "#0369a1", "#075985", "#0c4a6e", "#082f49"],
        "blue"    => ["#eff6ff", "#dbeafe", "#bfdbfe", "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8", "#1e40af", "#1e3a8a", "#172554"],
        "indigo"  => ["#eef2ff", "#e0e7ff", "#c7d2fe", "#a5b4fc", "#818cf8", "#6366f1", "#4f46e5", "#4338ca", "#3730a3", "#312e81", "#1e1b4b"],
        "violet"  => ["#f5f3ff", "#ede9fe", "#ddd6fe", "#c4b5fd", "#a78bfa", "#8b5cf6", "#7c3aed", "#6d28d9", "#5b21b6", "#4c1d95", "#2e1065"],
        "purple"  => ["#faf5ff", "#f3e8ff", "#e9d5ff", "#d8b4fe", "#c084fc", "#a855f7", "#9333ea", "#7e22ce", "#6b21a8", "#581c87", "#3b0764"],
        "fuchsia" => ["#fdf4ff", "#fae8ff", "#f5d0fe", "#f0abfc", "#e879f9", "#d946ef", "#c026d3", "#a21caf", "#86198f", "#701a75", "#4a044e"],
        "pink"    => ["#fdf2f8", "#fce7f3", "#fbcfe8", "#f9a8d4", "#f472b6", "#ec4899", "#db2777", "#be185d", "#9d174d", "#831843", "#500724"],
        "rose"    => ["#fff1f2", "#ffe4e6", "#fecdd3", "#fda4af", "#fb7185", "#f43f5e", "#e11d48", "#be123c", "#9f1239", "#881337", "#4c0519"],
    }?;

    // 1. 精准匹配标准 11 个预设阶梯
    match shade {
        "50" => return Some(Cow::Borrowed(shades_array[0])),
        "100" => return Some(Cow::Borrowed(shades_array[1])),
        "200" => return Some(Cow::Borrowed(shades_array[2])),
        "300" => return Some(Cow::Borrowed(shades_array[3])),
        "400" => return Some(Cow::Borrowed(shades_array[4])),
        "500" => return Some(Cow::Borrowed(shades_array[5])),
        "600" => return Some(Cow::Borrowed(shades_array[6])),
        "700" => return Some(Cow::Borrowed(shades_array[7])),
        "800" => return Some(Cow::Borrowed(shades_array[8])),
        "900" => return Some(Cow::Borrowed(shades_array[9])),
        "950" => return Some(Cow::Borrowed(shades_array[10])),
        _ => {}
    }

    // 2. 解析为数字 (1~1000) 尝试 RGB 线性插值
    let target_shade = shade.parse::<u32>().ok()?;
    if target_shade > 1000 {
        return None;
    }

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

    // 处理边界情况 < 50 (与 #ffffff 插值)
    if target_shade < 50 {
        let t = target_shade as f64 / 50.0;
        return Some(Cow::Owned(interpolate_hex("#ffffff", shades_array[0], t)));
    }

    // 处理边界情况 > 950 (与 #000000 插值)
    if target_shade > 950 {
        let t = (target_shade - 950) as f64 / 50.0;
        return Some(Cow::Owned(interpolate_hex(shades_array[10], "#000000", t)));
    }

    // 在中间 Checkpoints 寻找相邻插值区间
    for i in 0..CHECKPOINTS.len() - 1 {
        let (s1, idx1) = CHECKPOINTS[i];
        let (s2, idx2) = CHECKPOINTS[i + 1];
        if target_shade >= s1 && target_shade <= s2 {
            let t = (target_shade - s1) as f64 / (s2 - s1) as f64;
            let hex1 = shades_array[idx1];
            let hex2 = shades_array[idx2];
            return Some(Cow::Owned(interpolate_hex(hex1, hex2, t)));
        }
    }

    None
}

/// 将 16 进制颜色及透明度百分比转换为 rgba(...) 表达式（支持 3, 4, 6, 8 位 Hex）
pub fn hex_to_rgba(hex: &str, alpha_pct: f64) -> String {
    let clean = hex.strip_prefix('#').unwrap_or(hex);
    let alpha = (alpha_pct / 100.0).clamp(0.0, 1.0);

    let (r, g, b) = match clean.len() {
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

/// 解析颜色值词条（支持色板名如 `slate-900`, `indigo-600/50`, `white`, `black`, `transparent`, `[#1e293b]`, `rgba(...)`, `rgb(...)`, `hsl(...)`）
pub fn parse_color_value(color_token: &str) -> Option<UtilityValue> {
    let (base, opacity) = if let Some((b, op_str)) = color_token.split_once('/') {
        if let Ok(op) = op_str.parse::<f64>() {
            let pct = if (0.0..=1.0).contains(&op) {
                op * 100.0
            } else {
                op
            };
            (b, Some(pct))
        } else {
            (color_token, None)
        }
    } else {
        (color_token, None)
    };

    // 1. Direct function colors: rgba(...), rgb(...), hsl(...), hsla(...)
    if base.starts_with("rgba(")
        || base.starts_with("rgb(")
        || base.starts_with("hsl(")
        || base.starts_with("hsla(")
    {
        return Some(UtilityValue::ArbitraryLiteral(base.to_string()));
    }

    // 2. Hex color literal: [#1e293b] 或带透明度 [#1e293b]/50
    // 注意：若带有显式透明度修饰符（如 /50），会将 Hex 转换为 rgba(...) 表达式，
    // 显式修饰符会覆盖原 8 位 Hex 中自带的 Alpha 通道。
    if base.starts_with("[#") && base.ends_with(']') {
        let hex = &base[2..base.len() - 1];
        let full_hex = format!("#{}", hex);
        return match opacity {
            Some(op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba(&full_hex, op))),
            None => Some(UtilityValue::HexColor(full_hex)),
        };
    }

    // 3. Keyword colors: white, black, transparent
    if base == "white" {
        return match opacity {
            Some(op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba("#ffffff", op))),
            None => Some(UtilityValue::HexColor("#ffffff".to_string())),
        };
    }
    if base == "black" {
        return match opacity {
            Some(op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba("#000000", op))),
            None => Some(UtilityValue::HexColor("#000000".to_string())),
        };
    }
    if base == "transparent" {
        return match opacity {
            Some(_op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba("#000000", 0.0))),
            None => Some(UtilityValue::Keyword("transparent")),
        };
    }

    // 4. Standard Palette colors: slate-900, indigo-600, etc. (Supports interpolation for non-standard shades like slate-850)
    if let Some((color_name, shade)) = base.rsplit_once('-')
        && let Some(hex_str) = lookup_palette_color(color_name, shade)
    {
        return match opacity {
            Some(op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba(&hex_str, op))),
            None => Some(UtilityValue::HexColor(hex_str.into_owned())),
        };
    }

    None
}

/// 解析颜色相关的 Utility 类 (如 `text-slate-900`, `bg-indigo-600/50`, `bg-[#1e293b]`)
pub fn parse_color_utility(token: &str) -> Option<(&'static str, UtilityValue)> {
    const PREFIXES: &[(&str, &str)] = &[
        ("border-t", "border-top-color"),
        ("border-r", "border-right-color"),
        ("border-b", "border-bottom-color"),
        ("border-l", "border-left-color"),
        ("border", "border-color"),
        ("outline", "outline-color"),
        ("accent", "accent-color"),
        ("caret", "caret-color"),
        ("bg", "background-color"),
        ("text", "color"),
        ("fill", "fill"),
        ("stroke", "stroke"),
    ];

    for &(prefix, prop) in PREFIXES {
        if let Some(rest) = token.strip_prefix(prefix)
            && let Some(rest) = rest.strip_prefix('-')
        {
            if let Some(val) = parse_color_value(rest) {
                return Some((prop, val));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_palette_color_standard() {
        assert_eq!(
            lookup_palette_color("slate", "900").as_deref(),
            Some("#0f172a")
        );
    }

    #[test]
    fn test_lookup_palette_color_interpolated() {
        let color_850 = lookup_palette_color("slate", "850");
        assert!(color_850.is_some());
        assert_eq!(color_850.unwrap().as_ref(), "#172033");

        let color_25 = lookup_palette_color("indigo", "25");
        assert!(color_25.is_some());

        let color_975 = lookup_palette_color("red", "975");
        assert!(color_975.is_some());
    }

    #[test]
    fn test_parse_hex_rgb_all_lengths() {
        assert_eq!(parse_hex_rgb("#ffffff"), (255, 255, 255));
        assert_eq!(parse_hex_rgb("#fff"), (255, 255, 255));
        assert_eq!(parse_hex_rgb("#000"), (0, 0, 0));
        assert_eq!(parse_hex_rgb("#123"), (0x11, 0x22, 0x33));
        assert_eq!(parse_hex_rgb("#1e293b80"), (0x1e, 0x29, 0x3b));
    }

    #[test]
    fn test_interpolate_hex_3digit() {
        let res = interpolate_hex("#fff", "#000", 0.5);
        assert_eq!(res, "#808080");
    }

    #[test]
    fn test_parse_color_value_arbitrary_hex_and_opacity() {
        let val = parse_color_value("[#fff]/50").unwrap();
        assert_eq!(val, UtilityValue::ArbitraryLiteral("rgba(255, 255, 255, 0.5)".to_string()));

        let val8 = parse_color_value("[#1e293b80]/50").unwrap();
        assert_eq!(val8, UtilityValue::ArbitraryLiteral("rgba(30, 41, 59, 0.5)".to_string()));
    }

    #[test]
    fn test_parse_color_utility_prefixes() {
        let (prop, val) = parse_color_utility("bg-indigo-600/50").unwrap();
        assert_eq!(prop, "background-color");
        assert_eq!(val, UtilityValue::ArbitraryLiteral("rgba(79, 70, 229, 0.5)".to_string()));

        let (prop2, val2) = parse_color_utility("border-t-red-500").unwrap();
        assert_eq!(prop2, "border-top-color");
        assert_eq!(val2, UtilityValue::HexColor("#ef4444".to_string()));
    }
}

