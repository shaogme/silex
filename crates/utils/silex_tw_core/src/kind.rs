//! 任意值的类型分类器（报告 §2.8）。
//!
//! Tailwind 里不少前缀是**多义**的：`bg-` 接颜色是 `background-color`、接
//! `url()` 是 `background-image`、接长度是 `background-size`；`border-s-` 接颜色是
//! `border-inline-start-color`、接长度是 `border-inline-start-width`。
//!
//! 此前两侧都靠"先查哪张表"的顺序来隐式决定，于是同一个前缀只能有一种解释——
//! `bg-[url(...)]` 被无条件当成 `background-color`。现在按值的**形状**分派。

/// 任意值的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// `#1e293b`、`rgb(…)`、`red`、`transparent`
    Color,
    /// `12px`、`1.5rem`、`50%`、`calc(100% - 2px)`
    Length,
    /// 无单位数值：`1.75`、`4`
    Number,
    /// `url(...)`
    Url,
    /// `top`、`center`、`left top`
    Position,
    /// 其它无法判定的值：`var(--x)`、`0 0 0 1px red`、`ease-in-out`
    Keyword,
}

/// CSS 长度/角度/时间单位。长单位必须排在其后缀的短单位之前（`rem` 先于 `em`）。
const LENGTH_UNITS: &[&str] = &[
    "rem", "px", "%", "vmin", "vmax", "vw", "vh", "svh", "svw", "lvh", "lvw", "dvh", "dvw", "ch",
    "ex", "cm", "mm", "in", "pt", "pc", "em", "fr", "deg", "rad", "turn", "ms", "s",
];

/// 位置关键字
const POSITION_KEYWORDS: &[&str] = &["top", "bottom", "left", "right", "center"];

/// CSS 具名颜色。
///
/// 需要这份完整清单是因为 `border-[red]` 与 `border-[3px]` 必须分派到不同属性：
/// 少了它，`red` 会被判成普通关键字而落到 `border-width` 上。
const NAMED_COLORS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "currentcolor",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "transparent",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

/// 会产出颜色的 CSS 函数
const COLOR_FUNCTIONS: &[&str] = &[
    "rgb(",
    "rgba(",
    "hsl(",
    "hsla(",
    "hwb(",
    "lab(",
    "lch(",
    "oklab(",
    "oklch(",
    "color(",
    "color-mix(",
];

/// 判定一个任意值的类型
pub fn classify_arbitrary_value(val: &str) -> ValueKind {
    let v = val.trim();

    if v.starts_with('#') {
        return ValueKind::Color;
    }
    if COLOR_FUNCTIONS.iter().any(|f| v.starts_with(f)) {
        return ValueKind::Color;
    }
    if v.starts_with("url(") {
        return ValueKind::Url;
    }
    if v.starts_with("calc(")
        || v.starts_with("min(")
        || v.starts_with("max(")
        || v.starts_with("clamp(")
    {
        return ValueKind::Length;
    }

    let lower = v.to_ascii_lowercase();
    if NAMED_COLORS.contains(&lower.as_str()) {
        return ValueKind::Color;
    }

    if v.parse::<f64>().is_ok() {
        return ValueKind::Number;
    }
    if let Some(unit) = LENGTH_UNITS.iter().find(|u| v.ends_with(**u))
        && v[..v.len() - unit.len()].parse::<f64>().is_ok()
    {
        return ValueKind::Length;
    }

    if !v.is_empty()
        && v.split_whitespace()
            .all(|w| POSITION_KEYWORDS.contains(&w.to_ascii_lowercase().as_str()))
    {
        return ValueKind::Position;
    }

    ValueKind::Keyword
}

/// 多义前缀在给定值类型下的目标属性。
///
/// 只登记"同一前缀按值类型分派到不同属性"的情况；单义前缀不需要出现在这里。
pub fn arbitrary_dispatch(prefix: &str, kind: ValueKind) -> Option<&'static [&'static str]> {
    match (prefix, kind) {
        // `text-[14px]` 是字号，不是颜色——此前产出 `color: 14px` 的非法 CSS
        ("text", ValueKind::Length) => Some(&["font-size"]),
        // `bg-` 按值类型分派到 image / position / size
        ("bg", ValueKind::Url) => Some(&["background-image"]),
        ("bg", ValueKind::Position) => Some(&["background-position"]),
        ("bg", ValueKind::Length) => Some(&["background-size"]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_colors() {
        assert_eq!(classify_arbitrary_value("#1e293b"), ValueKind::Color);
        assert_eq!(classify_arbitrary_value("#fff"), ValueKind::Color);
        assert_eq!(
            classify_arbitrary_value("rgba(0, 0, 0, .5)"),
            ValueKind::Color
        );
        assert_eq!(
            classify_arbitrary_value("oklch(0.7 0.1 200)"),
            ValueKind::Color
        );
        assert_eq!(classify_arbitrary_value("red"), ValueKind::Color);
        assert_eq!(classify_arbitrary_value("transparent"), ValueKind::Color);
    }

    #[test]
    fn classifies_lengths_and_numbers() {
        assert_eq!(classify_arbitrary_value("12px"), ValueKind::Length);
        assert_eq!(classify_arbitrary_value("1.5rem"), ValueKind::Length);
        assert_eq!(classify_arbitrary_value("50%"), ValueKind::Length);
        assert_eq!(
            classify_arbitrary_value("calc(100% - 2px)"),
            ValueKind::Length
        );
        assert_eq!(classify_arbitrary_value("1.75"), ValueKind::Number);
        assert_eq!(classify_arbitrary_value("-10px"), ValueKind::Length);
    }

    #[test]
    fn classifies_url_position_and_keyword() {
        assert_eq!(
            classify_arbitrary_value("url(https://a.com/b.png)"),
            ValueKind::Url
        );
        assert_eq!(classify_arbitrary_value("center"), ValueKind::Position);
        assert_eq!(classify_arbitrary_value("left top"), ValueKind::Position);
        assert_eq!(classify_arbitrary_value("var(--x)"), ValueKind::Keyword);
        assert_eq!(
            classify_arbitrary_value("0 0 0 1px red"),
            ValueKind::Keyword
        );
        assert_eq!(classify_arbitrary_value("ease-in-out"), ValueKind::Keyword);
    }

    /// `rem` 必须先于 `em` 匹配，否则 `1.5rem` 会被切成 `1.5r` + `em` 而判成 Keyword
    #[test]
    fn longer_units_win_over_their_suffixes() {
        assert_eq!(classify_arbitrary_value("1.5rem"), ValueKind::Length);
        assert_eq!(classify_arbitrary_value("1.5em"), ValueKind::Length);
        assert_eq!(classify_arbitrary_value("100vmin"), ValueKind::Length);
    }

    #[test]
    fn dispatches_ambiguous_prefixes() {
        assert_eq!(
            arbitrary_dispatch("text", ValueKind::Length),
            Some(&["font-size"][..])
        );
        assert_eq!(arbitrary_dispatch("text", ValueKind::Color), None);
        assert_eq!(
            arbitrary_dispatch("bg", ValueKind::Url),
            Some(&["background-image"][..])
        );
        assert_eq!(arbitrary_dispatch("w", ValueKind::Length), None);
    }
}
