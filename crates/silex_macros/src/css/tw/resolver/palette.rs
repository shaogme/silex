use crate::css::tw::ast::UtilityValue;

macro_rules! palette_map {
    (
        $color_name:expr, $shade:expr;
        $(
            $color:literal => [
                $c50:literal, $c100:literal, $c200:literal, $c300:literal, $c400:literal,
                $c500:literal, $c600:literal, $c700:literal, $c800:literal, $c900:literal, $c950:literal
            ]
        ),* $(,)?
    ) => {
        match ($color_name, $shade) {
            $(
                ($color, "50") => Some($c50),
                ($color, "100") => Some($c100),
                ($color, "200") => Some($c200),
                ($color, "300") => Some($c300),
                ($color, "400") => Some($c400),
                ($color, "500") => Some($c500),
                ($color, "600") => Some($c600),
                ($color, "700") => Some($c700),
                ($color, "800") => Some($c800),
                ($color, "900") => Some($c900),
                ($color, "950") => Some($c950),
            )*
            _ => None,
        }
    };
}

/// 查找标准 Tailwind 色板颜色 (50-950)
pub fn lookup_palette_color(color_name: &str, shade: &str) -> Option<&'static str> {
    palette_map! {
        color_name, shade;
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
    }
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

    // 2. Hex color literal: [#1e293b]
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

    // 4. Standard Palette colors: slate-900, indigo-600, etc.
    if let Some((color_name, shade)) = base.rsplit_once('-')
        && let Some(hex_str) = lookup_palette_color(color_name, shade)
    {
        return match opacity {
            Some(op) => Some(UtilityValue::ArbitraryLiteral(hex_to_rgba(hex_str, op))),
            None => Some(UtilityValue::HexColor(hex_str.to_string())),
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
