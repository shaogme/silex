/// 排版、边框、圆角、阴影等
pub fn resolve_typography_border_effect_rules(
    class_name: &str,
) -> Option<Vec<(&'static str, String)>> {
    // Font size
    let font_size = match class_name {
        "text-xs" => Some(vec![("font-size", "0.75rem"), ("line-height", "1rem")]),
        "text-sm" => Some(vec![("font-size", "0.875rem"), ("line-height", "1.25rem")]),
        "text-base" => Some(vec![("font-size", "1rem"), ("line-height", "1.5rem")]),
        "text-lg" => Some(vec![("font-size", "1.125rem"), ("line-height", "1.75rem")]),
        "text-xl" => Some(vec![("font-size", "1.25rem"), ("line-height", "1.75rem")]),
        "text-2xl" => Some(vec![("font-size", "1.5rem"), ("line-height", "2rem")]),
        "text-3xl" => Some(vec![("font-size", "1.875rem"), ("line-height", "2.25rem")]),
        "text-4xl" => Some(vec![("font-size", "2.25rem"), ("line-height", "2.5rem")]),
        "text-5xl" => Some(vec![("font-size", "3rem"), ("line-height", "1")]),
        "text-6xl" => Some(vec![("font-size", "3.75rem"), ("line-height", "1")]),
        "text-7xl" => Some(vec![("font-size", "4.5rem"), ("line-height", "1")]),
        "text-8xl" => Some(vec![("font-size", "6rem"), ("line-height", "1")]),
        "text-9xl" => Some(vec![("font-size", "8rem"), ("line-height", "1")]),
        _ => None,
    };
    if let Some(rules) = font_size {
        return Some(rules.into_iter().map(|(k, v)| (k, v.to_string())).collect());
    }
    if class_name == "text" {
        return Some(vec![
            ("font-size", "1rem".to_string()),
            ("line-height", "1.5rem".to_string()),
        ]);
    }
    if let Some(rest) = class_name.strip_prefix("text-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("font-size", val)]);
        }
    }

    // Font family
    let font_family = match class_name {
        "font" | "font-sans" => Some("ui-sans-serif, system-ui, sans-serif"),
        "font-serif" => Some("ui-serif, Georgia, Cambria, Times New Roman, Times, serif"),
        "font-mono" => Some("ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"),
        _ => None,
    };
    if let Some(family) = font_family {
        return Some(vec![("font-family", family.to_string())]);
    }

    // Font weight
    let font_weight = match class_name {
        "font-thin" => Some("100"),
        "font-extralight" => Some("200"),
        "font-light" => Some("300"),
        "font-normal" => Some("400"),
        "font-medium" => Some("500"),
        "font-semibold" => Some("600"),
        "font-bold" => Some("700"),
        "font-extrabold" => Some("800"),
        "font-black" => Some("900"),
        _ => None,
    };
    if let Some(weight) = font_weight {
        return Some(vec![("font-weight", weight.to_string())]);
    }

    // Line height (leading)
    let leading = match class_name {
        "leading-none" => Some("1"),
        "leading-tight" => Some("1.25"),
        "leading-snug" => Some("1.375"),
        "leading" | "leading-normal" => Some("1.5"),
        "leading-relaxed" => Some("1.625"),
        "leading-loose" => Some("2"),
        "leading-px" => Some("1px"),
        _ => None,
    };
    if let Some(ld) = leading {
        return Some(vec![("line-height", ld.to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("leading-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("line-height", val)]);
        }
    }

    // Letter spacing (tracking)
    let tracking = match class_name {
        "tracking-tighter" => Some("-0.05em"),
        "tracking-tight" => Some("-0.025em"),
        "tracking" | "tracking-normal" => Some("0em"),
        "tracking-wide" => Some("0.025em"),
        "tracking-wider" => Some("0.05em"),
        "tracking-widest" => Some("0.1em"),
        "-tracking-tighter" => Some("0.05em"),
        "-tracking-tight" => Some("0.025em"),
        "-tracking-normal" => Some("0em"),
        "-tracking-wide" => Some("-0.025em"),
        "-tracking-wider" => Some("-0.05em"),
        "-tracking-widest" => Some("-0.1em"),
        _ => None,
    };
    if let Some(tr) = tracking {
        return Some(vec![("letter-spacing", tr.to_string())]);
    }

    // Border radius (rounded)
    if let Some(rules) = resolve_rounded_rules(class_name) {
        return Some(rules);
    }

    // Border width / style
    if let Some(rules) = resolve_border_rules(class_name) {
        return Some(rules);
    }

    // Shadow & Text Shadow & Drop Shadow
    match class_name {
        "shadow-2xs" => return Some(vec![("box-shadow", "0 1px 1px 0 rgba(0, 0, 0, 0.05)".to_string())]),
        "shadow-xs" => return Some(vec![("--tw-shadow", "0 1px 2px 0 rgba(0, 0, 0, 0.05)".to_string()), ("box-shadow", "var(--tw-ring-offset-shadow, 0 0 #0000), var(--tw-ring-shadow, 0 0 #0000), var(--tw-shadow, 0 1px 2px 0 rgba(0, 0, 0, 0.05))".to_string())]),
        "shadow-sm" | "shadow" => return Some(vec![("box-shadow", "0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px -1px rgba(0, 0, 0, 0.1)".to_string())]),
        "shadow-md" => return Some(vec![("box-shadow", "0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -2px rgba(0, 0, 0, 0.1)".to_string())]),
        "shadow-lg" => return Some(vec![("box-shadow", "0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -4px rgba(0, 0, 0, 0.1)".to_string())]),
        "shadow-xl" => return Some(vec![("box-shadow", "0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1)".to_string())]),
        "shadow-2xl" => return Some(vec![("box-shadow", "0 25px 50px -12px rgba(0, 0, 0, 0.25)".to_string())]),
        "shadow-inner" => return Some(vec![("box-shadow", "inset 0 2px 4px 0 rgba(0, 0, 0, 0.05)".to_string())]),
        "shadow-none" => return Some(vec![("box-shadow", "none".to_string())]),

        "drop-shadow-xs" => return Some(vec![("--tw-drop-shadow", "drop-shadow(0 1px 1px rgba(0, 0, 0, 0.05))".to_string()), ("filter", "var(--tw-drop-shadow)".to_string())]),

        "text-shadow-2xs" => return Some(vec![("text-shadow", "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))".to_string())]),
        "text-shadow-xs" => return Some(vec![("text-shadow", "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.2))".to_string())]),
        "text-shadow-sm" => return Some(vec![("text-shadow", "0px 1px 0px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075)), 0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075)), 0px 2px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.075))".to_string())]),
        "text-shadow-md" => return Some(vec![("text-shadow", "0px 1px 1px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 1px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 2px 4px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))".to_string())]),
        "text-shadow-lg" => return Some(vec![("text-shadow", "0px 1px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 3px 2px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1)), 0px 4px 8px var(--tw-text-shadow-color, rgba(0, 0, 0, 0.1))".to_string())]),
        _ => {}
    }
    // Columns
    if let Some(rest) = class_name.strip_prefix("columns-") {
        if rest == "auto" {
            return Some(vec![("column-count", "auto".to_string())]);
        }
        if rest == "none" {
            return Some(vec![
                ("column-width", "auto".to_string()),
                ("column-count", "auto".to_string()),
            ]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("column-count", n.to_string())]);
        }
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("column-width", val)]);
        }
    }

    // Blur Filters
    if class_name == "blur" {
        return Some(vec![("filter", "blur(8px)".to_string())]);
    }
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
            return Some(vec![("filter", b.to_string())]);
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
            return Some(vec![("max-width", w.to_string())]);
        }
    }

    // Break Inside / Before / After
    if let Some(rest) = class_name.strip_prefix("break-inside") {
        if rest.is_empty() {
            return Some(vec![("break-inside", "auto".to_string())]);
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(vec![("break-inside", sub.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("break-after") {
        if rest.is_empty() {
            return Some(vec![("break-after", "auto".to_string())]);
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(vec![("break-after", sub.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("break-before") {
        if rest.is_empty() {
            return Some(vec![("break-before", "auto".to_string())]);
        }
        if let Some(sub) = rest.strip_prefix('-') {
            return Some(vec![("break-before", sub.to_string())]);
        }
    }

    // Text Indent
    if let Some(rest) = class_name.strip_prefix("indent-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("text-indent", val)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-indent-") {
        if let Some(val) = super::dynamic::resolve_length_val(&format!("-{}", rest)) {
            return Some(vec![("text-indent", val)]);
        }
    }

    // Text Underline Offset
    if let Some(rest) = class_name.strip_prefix("underline-offset-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("text-underline-offset", format!("{}px", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-underline-offset-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("text-underline-offset", format!("-{}px", n))]);
        }
    }

    // Text Decoration Thickness
    if let Some(rest) = class_name.strip_prefix("decoration-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("text-decoration-thickness", format!("{}px", n))]);
        }
    }

    // Font Variant Numeric
    let font_numeric = match class_name {
        "lining-nums" => Some("lining-nums"),
        "oldstyle-nums" => Some("oldstyle-nums"),
        "proportional-nums" => Some("proportional-nums"),
        "tabular-nums" => Some("tabular-nums"),
        "diagonal-fractions" => Some("diagonal-fractions"),
        "stacked-fractions" => Some("stacked-fractions"),
        "ordinal" => Some("ordinal"),
        "slashed-zero" => Some("slashed-zero"),
        "normal-nums" => Some("normal"),
        _ => None,
    };
    if let Some(num) = font_numeric {
        return Some(vec![("font-variant-numeric", num.to_string())]);
    }

    // Text Wrap & Word Break
    let text_wrap = match class_name {
        "text-wrap" => Some(("text-wrap", "wrap")),
        "text-nowrap" => Some(("text-wrap", "nowrap")),
        "text-balance" => Some(("text-wrap", "balance")),
        "text-pretty" => Some(("text-wrap", "pretty")),
        "wrap-normal" => Some(("overflow-wrap", "normal")),
        "wrap-break-word" => Some(("overflow-wrap", "break-word")),
        "wrap-anywhere" => Some(("overflow-wrap", "anywhere")),
        _ => None,
    };
    if let Some((prop, val)) = text_wrap {
        return Some(vec![(prop, val.to_string())]);
    }

    // Line Clamp
    if class_name == "line-clamp-none" {
        return Some(vec![
            ("overflow", "visible".to_string()),
            ("display", "block".to_string()),
            ("-webkit-box-orient", "horizontal".to_string()),
            ("-webkit-line-clamp", "none".to_string()),
        ]);
    }
    if let Some(rest) = class_name.strip_prefix("line-clamp-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("overflow", "hidden".to_string()),
                ("display", "-webkit-box".to_string()),
                ("-webkit-box-orient", "vertical".to_string()),
                ("-webkit-line-clamp", n.to_string()),
            ]);
        }
    }

    // Font Stretch
    let font_stretch = match class_name {
        "font-condensed" => Some("condensed"),
        "font-expanded" => Some("expanded"),
        "font-extra-condensed" => Some("extra-condensed"),
        "font-extra-expanded" => Some("extra-expanded"),
        "font-semi-condensed" => Some("semi-condensed"),
        "font-semi-expanded" => Some("semi-expanded"),
        "font-ultra-condensed" => Some("ultra-condensed"),
        "font-ultra-expanded" => Some("ultra-expanded"),
        _ => None,
    };
    if let Some(st) = font_stretch {
        return Some(vec![("font-stretch", st.to_string())]);
    }
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
        if stretches.contains(&rest) {
            return Some(vec![("font-stretch", rest.to_string())]);
        }
    }

    // Divide Utilities
    if class_name == "divide-x" {
        return Some(vec![
            ("border-right-width", "0px".to_string()),
            ("border-left-width", "1px".to_string()),
        ]);
    }
    if class_name == "divide-y" {
        return Some(vec![
            ("border-bottom-width", "0px".to_string()),
            ("border-top-width", "1px".to_string()),
        ]);
    }
    if class_name == "divide-x-reverse" {
        return Some(vec![("--tw-divide-x-reverse", "1".to_string())]);
    }
    if class_name == "divide-y-reverse" {
        return Some(vec![("--tw-divide-y-reverse", "1".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("divide-x-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("border-right-width", "0px".to_string()),
                ("border-left-width", format!("{}px", n)),
            ]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("divide-y-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("border-bottom-width", "0px".to_string()),
                ("border-top-width", format!("{}px", n)),
            ]);
        }
    }
    let divide_style = match class_name {
        "divide-solid" => Some("solid"),
        "divide-dashed" => Some("dashed"),
        "divide-dotted" => Some("dotted"),
        "divide-double" => Some("double"),
        "divide-none" => Some("none"),
        _ => None,
    };
    if let Some(ds) = divide_style {
        return Some(vec![("border-style", ds.to_string())]);
    }

    // Inset Ring
    if class_name == "inset-ring" {
        return Some(vec![
            ("outline-width", "1px".to_string()),
            ("outline-offset", "-1px".to_string()),
        ]);
    }
    if let Some(rest) = class_name.strip_prefix("inset-ring-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("outline-width", format!("{}px", n)),
                ("outline-offset", format!("-{}px", n)),
            ]);
        }
    }

    // Inset Shadow
    if class_name == "inset-shadow" {
        return Some(vec![(
            "box-shadow",
            "inset 0 2px 4px 0 rgba(0, 0, 0, 0.05)".to_string(),
        )]);
    }
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
            return Some(vec![("box-shadow", v.to_string())]);
        }
    }

    // Tab Size
    if let Some(rest) = class_name.strip_prefix("tab-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("tab-size", n.to_string())]);
        }
    }

    // Zoom
    if let Some(rest) = class_name.strip_prefix("zoom-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("zoom", format!("{}%", n))]);
        }
    }

    // Border Spacing
    if let Some(rest) = class_name.strip_prefix("border-spacing-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("border-spacing", val)]);
        }
        if let Some(sub) = rest.strip_prefix("x-") {
            if let Some(val) = super::dynamic::resolve_length_val(sub) {
                return Some(vec![("border-spacing", format!("{} 0px", val))]);
            }
        }
        if let Some(sub) = rest.strip_prefix("y-") {
            if let Some(val) = super::dynamic::resolve_length_val(sub) {
                return Some(vec![("border-spacing", format!("0px {}", val))]);
            }
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

pub fn resolve_outline_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    match class_name {
        "outline-none" => {
            return Some(vec![
                ("outline", "2px solid transparent".to_string()),
                ("outline-offset", "2px".to_string()),
            ]);
        }
        "outline" => return Some(vec![("outline-style", "solid".to_string())]),
        "outline-solid" => return Some(vec![("outline-style", "solid".to_string())]),
        "outline-hidden" => return Some(vec![("outline-style", "hidden".to_string())]),
        "outline-dashed" => return Some(vec![("outline-style", "dashed".to_string())]),
        "outline-dotted" => return Some(vec![("outline-style", "dotted".to_string())]),
        "outline-double" => return Some(vec![("outline-style", "double".to_string())]),
        _ => {}
    }

    if let Some(rest) = class_name.strip_prefix("outline-offset-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("outline-offset", format!("{}px", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-outline-offset-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("outline-offset", format!("-{}px", n))]);
        }
    }

    if let Some(rest) = class_name.strip_prefix("outline-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("outline-style", "solid".to_string()),
                ("outline-width", format!("{}px", n)),
            ]);
        }
    }

    None
}

pub const RING_BOX_SHADOW: &str = "var(--tw-ring-inset, ) 0 0 0 var(--tw-ring-offset-width, 0px) var(--tw-ring-offset-color, #0000), 0 0 0 var(--tw-ring-width, 0px) var(--tw-ring-color, rgba(59, 130, 246, 0.5)), var(--tw-shadow, 0 0 #0000)";

pub fn resolve_ring_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    if class_name == "ring" {
        return Some(vec![
            ("--tw-ring-width", "0.1875rem".to_string()),
            ("box-shadow", RING_BOX_SHADOW.to_string()),
        ]);
    }
    if class_name == "ring-inset" {
        return Some(vec![("--tw-ring-inset", "inset".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("ring-") {
        if rest == "0" {
            return Some(vec![
                ("--tw-ring-width", "0px".to_string()),
                ("box-shadow", RING_BOX_SHADOW.to_string()),
            ]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![
                ("--tw-ring-width", format!("{}px", n)),
                ("box-shadow", RING_BOX_SHADOW.to_string()),
            ]);
        }
        if let Some(sub) = rest.strip_prefix("offset-") {
            if let Ok(n) = sub.parse::<u32>() {
                return Some(vec![
                    ("--tw-ring-offset-width", format!("{}px", n)),
                    ("box-shadow", RING_BOX_SHADOW.to_string()),
                ]);
            }
        }
    }

    None
}

pub fn resolve_rounded_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let radius_val = |s: &str| -> Option<&'static str> {
        match s {
            "none" => Some("0px"),
            "3xs" => Some("0.0625rem"),
            "2xs" => Some("0.125rem"),
            "xs" => Some("0.125rem"),
            "sm" => Some("0.125rem"),
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
            return Some(vec![("border-radius", "0.25rem".to_string())]);
        }
        if let Some(s) = rest.strip_prefix('-') {
            if let Some(val) = radius_val(s) {
                return Some(vec![("border-radius", val.to_string())]);
            }
            let sub_ss = if s == "ss" {
                Some("")
            } else {
                s.strip_prefix("ss-")
            };
            if let Some(sub) = sub_ss {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-start-start-radius", val.to_string())]);
                }
            }
            let sub_se = if s == "se" {
                Some("")
            } else {
                s.strip_prefix("se-")
            };
            if let Some(sub) = sub_se {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-start-end-radius", val.to_string())]);
                }
            }
            let sub_es = if s == "es" {
                Some("")
            } else {
                s.strip_prefix("es-")
            };
            if let Some(sub) = sub_es {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-end-start-radius", val.to_string())]);
                }
            }
            let sub_ee = if s == "ee" {
                Some("")
            } else {
                s.strip_prefix("ee-")
            };
            if let Some(sub) = sub_ee {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-end-end-radius", val.to_string())]);
                }
            }
            let sub_tl = if s == "tl" {
                Some("")
            } else {
                s.strip_prefix("tl-")
            };
            if let Some(sub) = sub_tl {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-top-left-radius", val.to_string())]);
                }
            }
            let sub_tr = if s == "tr" {
                Some("")
            } else {
                s.strip_prefix("tr-")
            };
            if let Some(sub) = sub_tr {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-top-right-radius", val.to_string())]);
                }
            }
            let sub_br = if s == "br" {
                Some("")
            } else {
                s.strip_prefix("br-")
            };
            if let Some(sub) = sub_br {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-bottom-right-radius", val.to_string())]);
                }
            }
            let sub_bl = if s == "bl" {
                Some("")
            } else {
                s.strip_prefix("bl-")
            };
            if let Some(sub) = sub_bl {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![("border-bottom-left-radius", val.to_string())]);
                }
            }
            let sub_t = if s == "t" {
                Some("")
            } else {
                s.strip_prefix("t-")
            };
            if let Some(sub) = sub_t {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-top-left-radius", val.to_string()),
                        ("border-top-right-radius", val.to_string()),
                    ]);
                }
            }
            let sub_r = if s == "r" {
                Some("")
            } else {
                s.strip_prefix("r-")
            };
            if let Some(sub) = sub_r {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-top-right-radius", val.to_string()),
                        ("border-bottom-right-radius", val.to_string()),
                    ]);
                }
            }
            let sub_b = if s == "b" {
                Some("")
            } else {
                s.strip_prefix("b-")
            };
            if let Some(sub) = sub_b {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-bottom-left-radius", val.to_string()),
                        ("border-bottom-right-radius", val.to_string()),
                    ]);
                }
            }
            let sub_l = if s == "l" {
                Some("")
            } else {
                s.strip_prefix("l-")
            };
            if let Some(sub) = sub_l {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-top-left-radius", val.to_string()),
                        ("border-bottom-left-radius", val.to_string()),
                    ]);
                }
            }
            let sub_s = if s == "s" {
                Some("")
            } else {
                s.strip_prefix("s-")
            };
            if let Some(sub) = sub_s {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-start-start-radius", val.to_string()),
                        ("border-end-start-radius", val.to_string()),
                    ]);
                }
            }
            let sub_e = if s == "e" {
                Some("")
            } else {
                s.strip_prefix("e-")
            };
            if let Some(sub) = sub_e {
                if let Some(val) = radius_val(sub) {
                    return Some(vec![
                        ("border-start-end-radius", val.to_string()),
                        ("border-end-end-radius", val.to_string()),
                    ]);
                }
            }
        }
    }
    None
}

pub fn resolve_border_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let width_val = |s: &str| -> Option<String> {
        match s {
            "0" => Some("0px".to_string()),
            "" => Some("1px".to_string()),
            "2" => Some("2px".to_string()),
            "4" => Some("4px".to_string()),
            "8" => Some("8px".to_string()),
            _ => {
                if let Ok(n) = s.parse::<u32>() {
                    Some(format!("{}px", n))
                } else {
                    None
                }
            }
        }
    };

    match class_name {
        "border-solid" => return Some(vec![("border-style", "solid".to_string())]),
        "border-dashed" => return Some(vec![("border-style", "dashed".to_string())]),
        "border-dotted" => return Some(vec![("border-style", "dotted".to_string())]),
        "border-double" => return Some(vec![("border-style", "double".to_string())]),
        "border-none" => return Some(vec![("border-style", "none".to_string())]),
        _ => {}
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
                return Some(vec![(prop, st.to_string())]);
            }
        }
    }

    if let Some(rest) = class_name.strip_prefix("border") {
        if rest.is_empty() {
            return Some(vec![("border-width", "1px".to_string())]);
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
