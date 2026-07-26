use std::borrow::Cow;

/// 排版、边框、圆角、阴影等
pub fn resolve_typography_border_effect_rules(
    class_name: &str,
) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    // 静态 Typography & Border 规则匹配
    let static_typo: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        // Typography Alignment & Decoration & Case & Truncate
        "text-left" => Some(cow![("text-align", "left")]),
        "text-center" => Some(cow![("text-align", "center")]),
        "text-right" => Some(cow![("text-align", "right")]),
        "text-justify" => Some(cow![("text-align", "justify")]),
        "text-start" => Some(cow![("text-align", "start")]),
        "text-end" => Some(cow![("text-align", "end")]),
        "text-clip" => Some(cow![("text-overflow", "clip")]),
        "text-ellipsis" => Some(cow![("text-overflow", "ellipsis")]),
        // v3 写法的别名
        "overflow-ellipsis" => Some(cow![("text-overflow", "ellipsis")]),

        "italic" => Some(cow![("font-style", "italic")]),
        "not-italic" => Some(cow![("font-style", "normal")]),
        "underline" => Some(cow![("text-decoration-line", "underline")]),
        "overline" => Some(cow![("text-decoration-line", "overline")]),
        "line-through" => Some(cow![("text-decoration-line", "line-through")]),
        "no-underline" => Some(cow![("text-decoration-line", "none")]),

        "uppercase" => Some(cow![("text-transform", "uppercase")]),
        "lowercase" => Some(cow![("text-transform", "lowercase")]),
        "capitalize" => Some(cow![("text-transform", "capitalize")]),
        "normal-case" => Some(cow![("text-transform", "none")]),
        "truncate" => Some(cow![
            ("overflow", "hidden"),
            ("text-overflow", "ellipsis"),
            ("white-space", "nowrap"),
        ]),

        // Font stretch & variant numeric
        "font-stretch-normal" => Some(cow![("font-stretch", "normal")]),
        "font-stretch-condensed" => Some(cow![("font-stretch", "condensed")]),
        "font-stretch-expanded" => Some(cow![("font-stretch", "expanded")]),
        "font-stretch-ultra-condensed" => Some(cow![("font-stretch", "ultra-condensed")]),
        "font-stretch-extra-condensed" => Some(cow![("font-stretch", "extra-condensed")]),
        "font-stretch-semi-condensed" => Some(cow![("font-stretch", "semi-condensed")]),
        "font-stretch-semi-expanded" => Some(cow![("font-stretch", "semi-expanded")]),
        "font-stretch-extra-expanded" => Some(cow![("font-stretch", "extra-expanded")]),
        "font-stretch-ultra-expanded" => Some(cow![("font-stretch", "ultra-expanded")]),
        "font-condensed" => Some(cow![("font-stretch", "condensed")]),
        "font-expanded" => Some(cow![("font-stretch", "expanded")]),
        "font-extra-condensed" => Some(cow![("font-stretch", "extra-condensed")]),
        "font-extra-expanded" => Some(cow![("font-stretch", "extra-expanded")]),
        "font-semi-condensed" => Some(cow![("font-stretch", "semi-condensed")]),
        "font-semi-expanded" => Some(cow![("font-stretch", "semi-expanded")]),
        "font-ultra-condensed" => Some(cow![("font-stretch", "ultra-condensed")]),
        "font-ultra-expanded" => Some(cow![("font-stretch", "ultra-expanded")]),

        "ordinal" => Some(cow![("font-variant-numeric", "ordinal")]),
        "slashed-zero" => Some(cow![("font-variant-numeric", "slashed-zero")]),
        "lining-nums" => Some(cow![("font-variant-numeric", "lining-nums")]),
        "oldstyle-nums" => Some(cow![("font-variant-numeric", "oldstyle-nums")]),
        "proportional-nums" => Some(cow![("font-variant-numeric", "proportional-nums")]),
        "tabular-nums" => Some(cow![("font-variant-numeric", "tabular-nums")]),
        "diagonal-fractions" => Some(cow![("font-variant-numeric", "diagonal-fractions")]),
        "stacked-fractions" => Some(cow![("font-variant-numeric", "stacked-fractions")]),
        "normal-nums" => Some(cow![("font-variant-numeric", "normal")]),

        // Text decoration style & thickness & offset
        "decoration-solid" => Some(cow![("text-decoration-style", "solid")]),
        "decoration-double" => Some(cow![("text-decoration-style", "double")]),
        "decoration-dotted" => Some(cow![("text-decoration-style", "dotted")]),
        "decoration-dashed" => Some(cow![("text-decoration-style", "dashed")]),
        "decoration-wavy" => Some(cow![("text-decoration-style", "wavy")]),
        "decoration-auto" => Some(cow![("text-decoration-thickness", "auto")]),
        "decoration-from-font" => Some(cow![("text-decoration-thickness", "from-font")]),
        "underline-offset-auto" => Some(cow![("text-underline-offset", "auto")]),

        // Whitespace & Word Break & Hyphens & Text Wrap
        "whitespace-normal" => Some(cow![("white-space", "normal")]),
        "whitespace-nowrap" => Some(cow![("white-space", "nowrap")]),
        "whitespace-pre" => Some(cow![("white-space", "pre")]),
        "whitespace-pre-line" => Some(cow![("white-space", "pre-line")]),
        "whitespace-pre-wrap" => Some(cow![("white-space", "pre-wrap")]),
        "whitespace-break-spaces" => Some(cow![("white-space", "break-spaces")]),
        "break-all" => Some(cow![("word-break", "break-all")]),
        "break-keep" => Some(cow![("word-break", "keep-all")]),
        "break-normal" => Some(cow![("overflow-wrap", "normal"), ("word-break", "normal")]),
        "hyphens-none" => Some(cow![("hyphens", "none")]),
        "hyphens-manual" => Some(cow![("hyphens", "manual")]),
        "hyphens-auto" => Some(cow![("hyphens", "auto")]),
        "text-wrap" => Some(cow![("text-wrap", "wrap")]),
        "text-nowrap" => Some(cow![("text-wrap", "nowrap")]),
        "text-balance" => Some(cow![("text-wrap", "balance")]),
        "text-pretty" => Some(cow![("text-wrap", "pretty")]),
        "wrap-normal" => Some(cow![("overflow-wrap", "normal")]),
        "wrap-break-word" => Some(cow![("overflow-wrap", "break-word")]),
        // v3 写法的别名
        "break-words" => Some(cow![("overflow-wrap", "break-word")]),
        "wrap-anywhere" => Some(cow![("overflow-wrap", "anywhere")]),

        // Antialiased
        "antialiased" => Some(cow![("-webkit-font-smoothing", "antialiased")]),
        "subpixel-antialiased" => Some(cow![("-webkit-font-smoothing", "auto")]),

        // Border Base & Hidden & Style
        "border" => Some(cow![("border-style", "solid"), ("border-width", "1px")]),
        "border-hidden" => Some(cow![("border-style", "hidden")]),
        "border-solid" => Some(cow![("border-style", "solid")]),
        "border-dashed" => Some(cow![("border-style", "dashed")]),
        "border-dotted" => Some(cow![("border-style", "dotted")]),
        "border-double" => Some(cow![("border-style", "double")]),
        "border-none" => Some(cow![("border-style", "none")]),

        // Font size presets
        "text-xs" => Some(cow![("font-size", "0.75rem"), ("line-height", "1rem")]),
        "text-sm" => Some(cow![("font-size", "0.875rem"), ("line-height", "1.25rem")]),
        "text-base" | "text" => Some(cow![("font-size", "1rem"), ("line-height", "1.5rem")]),
        "text-lg" => Some(cow![("font-size", "1.125rem"), ("line-height", "1.75rem")]),
        "text-xl" => Some(cow![("font-size", "1.25rem"), ("line-height", "1.75rem")]),
        "text-2xl" => Some(cow![("font-size", "1.5rem"), ("line-height", "2rem")]),
        "text-3xl" => Some(cow![("font-size", "1.875rem"), ("line-height", "2.25rem")]),
        "text-4xl" => Some(cow![("font-size", "2.25rem"), ("line-height", "2.5rem")]),
        "text-5xl" => Some(cow![("font-size", "3rem"), ("line-height", "1")]),
        "text-6xl" => Some(cow![("font-size", "3.75rem"), ("line-height", "1")]),
        "text-7xl" => Some(cow![("font-size", "4.5rem"), ("line-height", "1")]),
        "text-8xl" => Some(cow![("font-size", "6rem"), ("line-height", "1")]),
        "text-9xl" => Some(cow![("font-size", "8rem"), ("line-height", "1")]),

        // Font family presets
        "font" | "font-sans" => Some(cow![(
            "font-family",
            "ui-sans-serif, system-ui, sans-serif"
        )]),
        "font-serif" => Some(cow![(
            "font-family",
            "ui-serif, Georgia, Cambria, Times New Roman, Times, serif"
        )]),
        "font-mono" => Some(cow![(
            "font-family",
            "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
        )]),

        // Font weight presets
        "font-thin" => Some(cow![("font-weight", "100")]),
        "font-extralight" => Some(cow![("font-weight", "200")]),
        "font-light" => Some(cow![("font-weight", "300")]),
        "font-normal" => Some(cow![("font-weight", "400")]),
        "font-medium" => Some(cow![("font-weight", "500")]),
        "font-semibold" => Some(cow![("font-weight", "600")]),
        "font-bold" => Some(cow![("font-weight", "700")]),
        "font-extrabold" => Some(cow![("font-weight", "800")]),
        "font-black" => Some(cow![("font-weight", "900")]),

        // Line height presets
        "leading-none" => Some(cow![("line-height", "1")]),
        "leading-tight" => Some(cow![("line-height", "1.25")]),
        "leading-snug" => Some(cow![("line-height", "1.375")]),
        "leading" | "leading-normal" => Some(cow![("line-height", "1.5")]),
        "leading-relaxed" => Some(cow![("line-height", "1.625")]),
        "leading-loose" => Some(cow![("line-height", "2")]),
        "leading-px" => Some(cow![("line-height", "1px")]),

        // Letter spacing presets
        "tracking-tighter" => Some(cow![("letter-spacing", "-0.05em")]),
        "tracking-tight" => Some(cow![("letter-spacing", "-0.025em")]),
        "tracking" | "tracking-normal" => Some(cow![("letter-spacing", "0em")]),
        "tracking-wide" => Some(cow![("letter-spacing", "0.025em")]),
        "tracking-wider" => Some(cow![("letter-spacing", "0.05em")]),
        "tracking-widest" => Some(cow![("letter-spacing", "0.1em")]),
        "-tracking-tighter" => Some(cow![("letter-spacing", "0.05em")]),
        "-tracking-tight" => Some(cow![("letter-spacing", "0.025em")]),
        "-tracking-normal" => Some(cow![("letter-spacing", "0em")]),
        "-tracking-wide" => Some(cow![("letter-spacing", "-0.025em")]),
        "-tracking-wider" => Some(cow![("letter-spacing", "-0.05em")]),
        "-tracking-widest" => Some(cow![("letter-spacing", "-0.1em")]),

        // Shadows & Drop Shadows & Text Shadows
        "shadow-2xs" => Some(cow![("box-shadow", "0 1px 1px 0 rgba(0, 0, 0, 0.05)")]),
        "shadow-xs" => Some(cow![
            ("--tw-shadow", "0 1px 2px 0 rgba(0, 0, 0, 0.05)"),
            (
                "box-shadow",
                "var(--tw-ring-offset-shadow, 0 0 #0000), var(--tw-ring-shadow, 0 0 #0000), var(--tw-shadow, 0 1px 2px 0 rgba(0, 0, 0, 0.05))"
            )
        ]),
        "shadow-sm" | "shadow" => Some(cow![(
            "box-shadow",
            "0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1)"
        )]),
        "shadow-md" => Some(cow![(
            "box-shadow",
            "0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1)"
        )]),
        "shadow-lg" => Some(cow![(
            "box-shadow",
            "0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)"
        )]),
        "shadow-xl" => Some(cow![(
            "box-shadow",
            "0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)"
        )]),
        "shadow-2xl" => Some(cow![(
            "box-shadow",
            "0 25px 50px -12px rgba(0, 0, 0, 0.25)"
        )]),
        "shadow-inner" => Some(cow![(
            "box-shadow",
            "inset 0 2px 4px 0 rgba(0, 0, 0, 0.05)"
        )]),
        "shadow-none" => Some(cow![("box-shadow", "none")]),

        "drop-shadow-xs" => Some(cow![
            (
                "--tw-drop-shadow",
                "drop-shadow(0 1px 1px rgba(0, 0, 0, 0.05))"
            ),
            ("filter", "var(--tw-drop-shadow)")
        ]),

        "text-shadow-2xs" => Some(cow![(
            "text-shadow",
            "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))"
        )]),
        "text-shadow-xs" => Some(cow![(
            "text-shadow",
            "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.2))"
        )]),
        "text-shadow-sm" => Some(cow![(
            "text-shadow",
            "0px 1px 0px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075)), 0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075)), 0px 2px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075))"
        )]),
        "text-shadow-md" => Some(cow![(
            "text-shadow",
            "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 1px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 2px 4px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))"
        )]),
        "text-shadow-lg" => Some(cow![(
            "text-shadow",
            "0px 1px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 3px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 4px 8px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))"
        )]),

        // `divide-*` 见 `resolver::between`——它的声明必须落在伴生选择器上，
        // 这里的 `(属性, 值)` 表达不了，放在这儿就是一份不带选择器的错误副本

        // Inset Ring & Inset Shadow
        "inset-ring" => Some(cow![("outline-width", "1px"), ("outline-offset", "-1px")]),
        "inset-shadow" | "inset-shadow-inner" => Some(cow![(
            "box-shadow",
            "inset 0 2px 4px 0 rgba(0, 0, 0, 0.05)"
        )]),
        "inset-shadow-2xs" => Some(cow![(
            "box-shadow",
            "inset 0 1px 1px 0 rgba(0, 0, 0, 0.05)"
        )]),
        "inset-shadow-xs" => Some(cow![(
            "box-shadow",
            "inset 0 1px 2px 0 rgba(0, 0, 0, 0.05)"
        )]),
        "inset-shadow-sm" => Some(cow![("box-shadow", "inset 0 1px 3px 0 rgba(0, 0, 0, 0.1)")]),
        "inset-shadow-md" => Some(cow![(
            "box-shadow",
            "inset 0 4px 6px -1px rgba(0, 0, 0, 0.1)"
        )]),
        "inset-shadow-lg" => Some(cow![(
            "box-shadow",
            "inset 0 10px 15px -3px rgba(0, 0, 0, 0.1)"
        )]),
        "inset-shadow-xl" => Some(cow![(
            "box-shadow",
            "inset 0 20px 25px -5px rgba(0, 0, 0, 0.1)"
        )]),
        "inset-shadow-2xl" => Some(cow![(
            "box-shadow",
            "inset 0 25px 50px -12px rgba(0, 0, 0, 0.25)"
        )]),
        "inset-shadow-none" => Some(cow![("box-shadow", "inset 0 0 #0000")]),
        "inset-shadow-initial" => Some(cow![("box-shadow", "initial")]),

        // Blur Filter
        "blur" => Some(cow![("filter", "blur(8px)")]),

        // Line Clamp
        "line-clamp-none" => Some(cow![
            ("overflow", "visible"),
            ("display", "block"),
            ("-webkit-box-orient", "horizontal"),
            ("-webkit-line-clamp", "none"),
        ]),

        _ => None,
    };
    if let Some(r) = static_typo {
        return Some(r.to_vec());
    }

    if let Some(rest) = class_name.strip_prefix("text-")
        && let Some(val) = super::dynamic::resolve_length_val(rest)
    {
        return Some(cow!(vec[("font-size", val)]));
    }

    if let Some(rest) = class_name.strip_prefix("leading-")
        && let Some(val) = super::dynamic::resolve_length_val(rest)
    {
        return Some(cow!(vec[("line-height", val)]));
    }

    // Border radius (rounded)
    if let Some(rules) = resolve_rounded_rules(class_name) {
        return Some(rules);
    }

    // Border width / style
    if let Some(rules) = resolve_border_rules(class_name) {
        return Some(rules);
    }

    // Columns
    //
    // 两种后缀都写进 `columns` 简写，由浏览器按值的形态区分列数与列宽——这正是
    // Tailwind 的做法。此前分派到 `column-count` / `column-width` 两个长写属性，
    // `columns-lg` 因而产出 `column-count:32rem` 这样的非法 CSS（报告 §11.5）。
    if let Some(rest) = class_name.strip_prefix("columns-") {
        // 数值先判：`resolve_length_val("3")` 会按间距档位求成 `0.75rem`
        if rest.parse::<u32>().is_ok() {
            return Some(cow!(vec[("columns", rest.to_string())]));
        }
        // `columns-none` 不是 Tailwind 类名，但历来解释为"不施加列约束"，等价于 auto
        if rest == "none" {
            return Some(cow!(vec[("columns", "auto")]));
        }
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(cow!(vec[("columns", val)]));
        }
    }

    // Blur Filters
    if let Some(rest) = class_name.strip_prefix("blur-") {
        let val = match rest {
            "none" => Some("none"),
            "xs" => Some("blur(2px)"),
            "sm" => Some("blur(4px)"),
            "md" => Some("blur(8px)"),
            "lg" => Some("blur(16px)"),
            "xl" => Some("blur(24px)"),
            "2xl" => Some("blur(40px)"),
            "3xl" => Some("blur(64px)"),
            _ => None,
        };
        if let Some(b) = val {
            return Some(cow!(vec[("filter", b)]));
        }
    }

    // Max Width Presets
    if let Some(rest) = class_name.strip_prefix("max-w-") {
        let val = match rest {
            "0" => Some("0rem"),
            "none" => Some("none"),
            "xs" => Some("20rem"),
            "sm" => Some("24rem"),
            "md" => Some("28rem"),
            "lg" => Some("32rem"),
            "xl" => Some("36rem"),
            "2xl" => Some("42rem"),
            "3xl" => Some("48rem"),
            "4xl" => Some("56rem"),
            "5xl" => Some("64rem"),
            "6xl" => Some("72rem"),
            "7xl" => Some("80rem"),
            "full" => Some("100%"),
            "prose" => Some("65rem"),
            "screen-sm" => Some("40rem"),
            "screen-md" => Some("48rem"),
            "screen-lg" => Some("64rem"),
            "screen-xl" => Some("80rem"),
            "screen-2xl" => Some("96rem"),
            _ => None,
        };
        if let Some(w) = val {
            return Some(cow!(vec[("max-width", w)]));
        }
    }

    // Break Inside / Before / After
    if let Some(rest) = class_name.strip_prefix("break-inside") {
        if rest.is_empty() {
            return Some(cow!(vec[("break-inside", "auto")]));
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(cow!(vec[("break-inside", sub.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("break-after") {
        if rest.is_empty() {
            return Some(cow!(vec[("break-after", "auto")]));
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(cow!(vec[("break-after", sub.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("break-before") {
        if rest.is_empty() {
            return Some(cow!(vec[("break-before", "auto")]));
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(cow!(vec[("break-before", sub.to_string())]));
        }
    }

    // Text Indent
    if let Some(rest) = class_name.strip_prefix("indent-")
        && let Some(val) = super::dynamic::resolve_length_val(rest)
    {
        return Some(cow!(vec[("text-indent", val)]));
    }
    if let Some(rest) = class_name.strip_prefix("-indent-")
        && let Some(val) = super::dynamic::resolve_length_val(&format!("-{}", rest))
    {
        return Some(cow!(vec[("text-indent", val)]));
    }

    // Text Underline Offset
    if let Some(rest) = class_name.strip_prefix("underline-offset-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("text-underline-offset", format!("{}px", n))]));
    }
    if let Some(rest) = class_name.strip_prefix("-underline-offset-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("text-underline-offset", format!("-{}px", n))]));
    }

    // Text Decoration Thickness
    if let Some(rest) = class_name.strip_prefix("decoration-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("text-decoration-thickness", format!("{}px", n))]));
    }

    // Line Clamp
    if let Some(rest) = class_name.strip_prefix("line-clamp-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![
            ("overflow", "hidden"),
            ("display", "-webkit-box"),
            ("-webkit-box-orient", "vertical"),
            ("-webkit-line-clamp", n.to_string()),
        ]));
    }

    // Font Stretch
    if let Some(rest) = class_name.strip_prefix("font-stretch-") {
        let stretches = [
            "50%",
            "75%",
            "90%",
            "95%",
            "100%",
            "105%",
            "110%",
            "125%",
            "150%",
            "200%",
            "ultra-condensed",
            "extra-condensed",
            "condensed",
            "semi-condensed",
            "normal",
            "semi-expanded",
            "expanded",
            "extra-expanded",
            "ultra-expanded",
        ];
        if let Some(&matched) = stretches.iter().find(|&&s| s == rest) {
            return Some(cow!(vec[("font-stretch", matched)]));
        }
    }

    // Divide Utilities
    if let Some(rest) = class_name.strip_prefix("divide-x-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![
            ("border-right-width", "0px"),
            ("border-left-width", format!("{}px", n)),
        ]));
    }
    if let Some(rest) = class_name.strip_prefix("divide-y-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![
            ("border-bottom-width", "0px"),
            ("border-top-width", format!("{}px", n)),
        ]));
    }

    // Inset Ring
    if let Some(rest) = class_name.strip_prefix("inset-ring-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![
            ("outline-width", format!("{}px", n)),
            ("outline-offset", format!("-{}px", n)),
        ]));
    }

    // Inset Shadow
    if let Some(rest) = class_name.strip_prefix("inset-shadow-") {
        let val = match rest {
            "2xs" => Some("inset 0 1px 1px 0 rgba(0, 0, 0, 0.05)"),
            "xs" => Some("inset 0 1px 2px 0 rgba(0, 0, 0, 0.05)"),
            "sm" => Some("inset 0 1px 3px 0 rgba(0, 0, 0, 0.1)"),
            "md" => Some("inset 0 4px 6px -1px rgba(0, 0, 0, 0.1)"),
            "lg" => Some("inset 0 10px 15px -3px rgba(0, 0, 0, 0.1)"),
            "xl" => Some("inset 0 20px 25px -5px rgba(0, 0, 0, 0.1)"),
            "2xl" => Some("inset 0 25px 50px -12px rgba(0, 0, 0, 0.25)"),
            "inner" => Some("inset 0 2px 4px 0 rgba(0, 0, 0, 0.05)"),
            "none" => Some("inset 0 0 #0000"),
            "initial" => Some("initial"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(cow!(vec[("box-shadow", v)]));
        }
    }

    // Tab Size
    if let Some(rest) = class_name.strip_prefix("tab-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("tab-size", n.to_string())]));
    }

    // Zoom
    if let Some(rest) = class_name.strip_prefix("zoom-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("zoom", format!("{}%", n))]));
    }

    // Border Spacing
    if let Some(rest) = class_name.strip_prefix("border-spacing-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(cow!(vec[("border-spacing", val)]));
        }
        if let Some(sub) = rest.strip_prefix("x-")
            && let Some(val) = super::dynamic::resolve_length_val(sub)
        {
            return Some(cow!(vec[("border-spacing", format!("{} 0px", val))]));
        }
        if let Some(sub) = rest.strip_prefix("y-")
            && let Some(val) = super::dynamic::resolve_length_val(sub)
        {
            return Some(cow!(vec[("border-spacing", format!("0px {}", val))]));
        }
    }

    // Filter & Backdrop Filter
    if let Some(rules) = super::filter::resolve_filter_rules(class_name) {
        return Some(rules);
    }

    // Outline
    if let Some(rules) = resolve_outline_rules(class_name) {
        return Some(rules);
    }

    // Ring
    if let Some(rules) = resolve_ring_rules(class_name) {
        return Some(rules);
    }

    None
}

pub fn resolve_outline_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let static_rules: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        "outline-none" => Some(cow![
            ("outline", "2px solid transparent"),
            ("outline-offset", "2px")
        ]),
        "outline" | "outline-solid" => Some(cow![("outline-style", "solid")]),
        "outline-hidden" => Some(cow![("outline-style", "hidden")]),
        "outline-dashed" => Some(cow![("outline-style", "dashed")]),
        "outline-dotted" => Some(cow![("outline-style", "dotted")]),
        "outline-double" => Some(cow![("outline-style", "double")]),
        _ => None,
    };
    if let Some(r) = static_rules {
        return Some(r.to_vec());
    }

    if let Some(rest) = class_name.strip_prefix("outline-offset-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("outline-offset", format!("{}px", n))]));
    }
    if let Some(rest) = class_name.strip_prefix("-outline-offset-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("outline-offset", format!("-{}px", n))]));
    }

    if let Some(rest) = class_name.strip_prefix("outline-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![
            ("outline-style", "solid"),
            ("outline-width", format!("{}px", n)),
        ]));
    }

    None
}

pub const RING_BOX_SHADOW: &str = "var(--tw-ring-inset, ) 0 0 0 var(--tw-ring-offset-width, 0px) var(--tw-ring-offset-color, #0000), 0 0 0 var(--tw-ring-width, 0px) var(--tw-ring-color, rgba(59, 130, 246, 0.5)), var(--tw-shadow, 0 0 #0000)";

pub fn resolve_ring_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let static_rules: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        "ring" => Some(cow![
            ("--tw-ring-width", "0.1875rem"),
            ("box-shadow", RING_BOX_SHADOW)
        ]),
        "ring-inset" => Some(cow![("--tw-ring-inset", "inset")]),
        _ => None,
    };
    if let Some(r) = static_rules {
        return Some(r.to_vec());
    }

    if let Some(rest) = class_name.strip_prefix("ring-") {
        if rest == "0" {
            return Some(cow!(vec![
                ("--tw-ring-width", "0px"),
                ("box-shadow", RING_BOX_SHADOW),
            ]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec![
                ("--tw-ring-width", format!("{}px", n)),
                ("box-shadow", RING_BOX_SHADOW),
            ]));
        }
        if let Some(sub) = rest.strip_prefix("offset-")
            && let Ok(n) = sub.parse::<u32>()
        {
            return Some(cow!(vec![
                ("--tw-ring-offset-width", format!("{}px", n)),
                ("box-shadow", RING_BOX_SHADOW),
            ]));
        }
    }

    None
}

pub fn resolve_rounded_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let radius_val = |s: &str| -> Option<&'static str> {
        match s {
            "none" => Some("0px"),
            "3xs" => Some("0.0625rem"),
            "2xs" => Some("0.125rem"),
            "xs" => Some("0.125rem"),
            // Tailwind v4 的 `--radius-sm` 是 0.25rem（v3 里才是 0.125rem）。
            // 对拍测试抓到这条陈旧值影响了全部 30 个 rounded-*-sm 类名。
            "sm" => Some("0.25rem"),
            "" => Some("0.25rem"),
            "md" => Some("0.375rem"),
            "lg" => Some("0.5rem"),
            "xl" => Some("0.75rem"),
            "2xl" => Some("1rem"),
            "3xl" => Some("1.5rem"),
            "4xl" => Some("2rem"),
            "full" => Some("9999px"),
            _ => None,
        }
    };

    if let Some(rest) = class_name.strip_prefix("rounded") {
        if rest.is_empty() {
            return Some(cow!(vec[("border-radius", "0.25rem")]));
        }
        if let Some(s) = rest.strip_prefix('-') {
            if let Some(val) = radius_val(s) {
                return Some(cow!(vec[("border-radius", val)]));
            }
            let sub_ss = if s == "ss" {
                Some("")
            } else {
                s.strip_prefix("ss-")
            };
            if let Some(sub) = sub_ss
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-start-start-radius", val)]));
            }
            let sub_se = if s == "se" {
                Some("")
            } else {
                s.strip_prefix("se-")
            };
            if let Some(sub) = sub_se
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-start-end-radius", val)]));
            }
            let sub_es = if s == "es" {
                Some("")
            } else {
                s.strip_prefix("es-")
            };
            if let Some(sub) = sub_es
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-end-start-radius", val)]));
            }
            let sub_ee = if s == "ee" {
                Some("")
            } else {
                s.strip_prefix("ee-")
            };
            if let Some(sub) = sub_ee
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-end-end-radius", val)]));
            }
            let sub_tl = if s == "tl" {
                Some("")
            } else {
                s.strip_prefix("tl-")
            };
            if let Some(sub) = sub_tl
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-top-left-radius", val)]));
            }
            let sub_tr = if s == "tr" {
                Some("")
            } else {
                s.strip_prefix("tr-")
            };
            if let Some(sub) = sub_tr
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-top-right-radius", val)]));
            }
            let sub_br = if s == "br" {
                Some("")
            } else {
                s.strip_prefix("br-")
            };
            if let Some(sub) = sub_br
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-bottom-right-radius", val)]));
            }
            let sub_bl = if s == "bl" {
                Some("")
            } else {
                s.strip_prefix("bl-")
            };
            if let Some(sub) = sub_bl
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec[("border-bottom-left-radius", val)]));
            }
            let sub_t = if s == "t" {
                Some("")
            } else {
                s.strip_prefix("t-")
            };
            if let Some(sub) = sub_t
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-top-left-radius", val),
                    ("border-top-right-radius", val),
                ]));
            }
            let sub_r = if s == "r" {
                Some("")
            } else {
                s.strip_prefix("r-")
            };
            if let Some(sub) = sub_r
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-top-right-radius", val),
                    ("border-bottom-right-radius", val),
                ]));
            }
            let sub_b = if s == "b" {
                Some("")
            } else {
                s.strip_prefix("b-")
            };
            if let Some(sub) = sub_b
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-bottom-left-radius", val),
                    ("border-bottom-right-radius", val),
                ]));
            }
            let sub_l = if s == "l" {
                Some("")
            } else {
                s.strip_prefix("l-")
            };
            if let Some(sub) = sub_l
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-top-left-radius", val),
                    ("border-bottom-left-radius", val),
                ]));
            }
            let sub_s = if s == "s" {
                Some("")
            } else {
                s.strip_prefix("s-")
            };
            if let Some(sub) = sub_s
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-start-start-radius", val),
                    ("border-end-start-radius", val),
                ]));
            }
            let sub_e = if s == "e" {
                Some("")
            } else {
                s.strip_prefix("e-")
            };
            if let Some(sub) = sub_e
                && let Some(val) = radius_val(sub)
            {
                return Some(cow!(vec![
                    ("border-start-end-radius", val),
                    ("border-end-end-radius", val),
                ]));
            }
        }
    }
    None
}

pub fn resolve_border_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let width_val = |s: &str| -> Option<Cow<'static, str>> {
        match s {
            "0" => Some(Cow::Borrowed("0px")),
            "" => Some(Cow::Borrowed("1px")),
            "2" => Some(Cow::Borrowed("2px")),
            "4" => Some(Cow::Borrowed("4px")),
            "8" => Some(Cow::Borrowed("8px")),
            _ => {
                if let Ok(n) = s.parse::<u32>() {
                    Some(Cow::Owned(format!("{}px", n)))
                } else {
                    None
                }
            }
        }
    };

    let static_rules: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        "border-solid" => Some(cow![("border-style", "solid")]),
        "border-dashed" => Some(cow![("border-style", "dashed")]),
        "border-dotted" => Some(cow![("border-style", "dotted")]),
        "border-double" => Some(cow![("border-style", "double")]),
        "border-none" => Some(cow![("border-style", "none")]),
        _ => None,
    };
    if let Some(r) = static_rules {
        return Some(r.to_vec());
    }

    let border_style_prefixes = &[
        ("border-b-", "border-bottom-style"),
        ("border-t-", "border-top-style"),
        ("border-l-", "border-left-style"),
        ("border-r-", "border-right-style"),
        ("border-x-", "border-inline-style"),
        ("border-y-", "border-block-style"),
        ("border-s-", "border-inline-start-style"),
        ("border-e-", "border-inline-end-style"),
        ("border-bs-", "border-block-start-style"),
        ("border-be-", "border-block-end-style"),
    ];
    for &(prefix, prop) in border_style_prefixes {
        if let Some(rest) = class_name.strip_prefix(prefix) {
            let style = match rest {
                "solid" => Some("solid"),
                "dashed" => Some("dashed"),
                "dotted" => Some("dotted"),
                "double" => Some("double"),
                "hidden" => Some("hidden"),
                "none" => Some("none"),
                _ => None,
            };
            if let Some(st) = style {
                return Some(cow!(vec[(prop, st)]));
            }
        }
    }

    if let Some(rest) = class_name.strip_prefix("border") {
        if rest.is_empty() {
            return Some(cow!(vec[("border-width", "1px")]));
        }
        if let Some(s) = rest.strip_prefix('-') {
            if let Some(w) = width_val(s) {
                return Some(vec![("border-width", w)]);
            }
            if let Some(sub) = s.strip_prefix("bs") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-block-start-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("be") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-block-end-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("t") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-top-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("r") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-right-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("b") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-bottom-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("l") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-left-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("s") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-inline-start-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("e") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![("border-inline-end-width", w)]);
                }
            }
            if let Some(sub) = s.strip_prefix("x") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![
                        ("border-left-width", w.clone()),
                        ("border-right-width", w),
                    ]);
                }
            }
            if let Some(sub) = s.strip_prefix("y") {
                let sub_clean = sub.strip_prefix('-').unwrap_or(sub);
                if let Some(w) = width_val(sub_clean) {
                    return Some(vec![
                        ("border-top-width", w.clone()),
                        ("border-bottom-width", w),
                    ]);
                }
            }
        }
    }

    None
}
