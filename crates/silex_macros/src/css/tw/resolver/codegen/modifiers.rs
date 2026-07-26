// 自动生成的 Tailwind 修饰符与断点规则表（供 silex_macros 使用）
// 由 silex_codegen 自动生成，切勿手写修改！

use crate::css::tw::ast::Modifier;

pub struct ModifierMeta {
    pub key: &'static str,
    pub priority: u32,
    pub css_selector: &'static str,
}

#[rustfmt::skip]
pub static MODIFIER_TABLE: &[ModifierMeta] = &[
    ModifierMeta { key: "*", priority: 10, css_selector: "& > *" },
    ModifierMeta { key: "**", priority: 10, css_selector: "& *" },
    ModifierMeta { key: "2xl", priority: 2536, css_selector: "(min-width: 1536px)" },
    ModifierMeta { key: "active", priority: 20, css_selector: "&:active" },
    ModifierMeta { key: "after", priority: 20, css_selector: "&::after" },
    ModifierMeta { key: "any-pointer-coarse", priority: 65, css_selector: "(any-pointer: coarse)" },
    ModifierMeta { key: "any-pointer-fine", priority: 65, css_selector: "(any-pointer: fine)" },
    ModifierMeta { key: "any-pointer-none", priority: 65, css_selector: "(any-pointer: none)" },
    ModifierMeta { key: "autofill", priority: 20, css_selector: "&:autofill" },
    ModifierMeta { key: "backdrop", priority: 20, css_selector: "&::backdrop" },
    ModifierMeta { key: "before", priority: 20, css_selector: "&::before" },
    ModifierMeta { key: "checked", priority: 20, css_selector: "&:checked" },
    ModifierMeta { key: "contrast-less", priority: 65, css_selector: "(prefers-contrast: less)" },
    ModifierMeta { key: "contrast-more", priority: 65, css_selector: "(prefers-contrast: more)" },
    ModifierMeta { key: "dark", priority: 60, css_selector: ".dark &, &.dark" },
    ModifierMeta { key: "default", priority: 20, css_selector: "&:default" },
    ModifierMeta { key: "details-content", priority: 20, css_selector: "&::details-content" },
    ModifierMeta { key: "disabled", priority: 20, css_selector: "&:disabled" },
    ModifierMeta { key: "empty", priority: 20, css_selector: "&:empty" },
    ModifierMeta { key: "enabled", priority: 20, css_selector: "&:enabled" },
    ModifierMeta { key: "even", priority: 20, css_selector: "&:nth-child(even)" },
    ModifierMeta { key: "file", priority: 20, css_selector: "&::file-selector-button" },
    ModifierMeta { key: "first", priority: 20, css_selector: "&:first-child" },
    ModifierMeta { key: "first-letter", priority: 20, css_selector: "&::first-letter" },
    ModifierMeta { key: "first-line", priority: 20, css_selector: "&::first-line" },
    ModifierMeta { key: "first-of-type", priority: 20, css_selector: "&:first-of-type" },
    ModifierMeta { key: "focus", priority: 20, css_selector: "&:focus" },
    ModifierMeta { key: "focus-visible", priority: 20, css_selector: "&:focus-visible" },
    ModifierMeta { key: "focus-within", priority: 20, css_selector: "&:focus-within" },
    ModifierMeta { key: "forced-colors", priority: 65, css_selector: "(forced-colors: active)" },
    ModifierMeta { key: "hover", priority: 20, css_selector: "&:hover" },
    ModifierMeta { key: "in-range", priority: 20, css_selector: "&:in-range" },
    ModifierMeta { key: "indeterminate", priority: 20, css_selector: "&:indeterminate" },
    ModifierMeta { key: "inert", priority: 25, css_selector: "&:is([inert], [inert] *)" },
    ModifierMeta { key: "invalid", priority: 20, css_selector: "&:invalid" },
    ModifierMeta { key: "inverted-colors", priority: 65, css_selector: "(inverted-colors: inverted)" },
    ModifierMeta { key: "landscape", priority: 65, css_selector: "(orientation: landscape)" },
    ModifierMeta { key: "last", priority: 20, css_selector: "&:last-child" },
    ModifierMeta { key: "last-of-type", priority: 20, css_selector: "&:last-of-type" },
    ModifierMeta { key: "lg", priority: 2024, css_selector: "(min-width: 1024px)" },
    ModifierMeta { key: "ltr", priority: 25, css_selector: "&:where(:dir(ltr), [dir=\"ltr\"], [dir=\"ltr\"] *)" },
    ModifierMeta { key: "marker", priority: 20, css_selector: "&::marker" },
    ModifierMeta { key: "md", priority: 1768, css_selector: "(min-width: 768px)" },
    ModifierMeta { key: "motion-reduce", priority: 65, css_selector: "(prefers-reduced-motion: reduce)" },
    ModifierMeta { key: "motion-safe", priority: 65, css_selector: "(prefers-reduced-motion: no-preference)" },
    ModifierMeta { key: "noscript", priority: 65, css_selector: "(scripting: none)" },
    ModifierMeta { key: "odd", priority: 20, css_selector: "&:nth-child(odd)" },
    ModifierMeta { key: "only", priority: 20, css_selector: "&:only-child" },
    ModifierMeta { key: "only-of-type", priority: 20, css_selector: "&:only-of-type" },
    ModifierMeta { key: "open", priority: 25, css_selector: "&:is([open], :popover-open, :open)" },
    ModifierMeta { key: "optional", priority: 20, css_selector: "&:optional" },
    ModifierMeta { key: "out-of-range", priority: 20, css_selector: "&:out-of-range" },
    ModifierMeta { key: "placeholder", priority: 20, css_selector: "&::placeholder" },
    ModifierMeta { key: "placeholder-shown", priority: 20, css_selector: "&:placeholder-shown" },
    ModifierMeta { key: "pointer-coarse", priority: 65, css_selector: "(pointer: coarse)" },
    ModifierMeta { key: "pointer-fine", priority: 65, css_selector: "(pointer: fine)" },
    ModifierMeta { key: "pointer-none", priority: 65, css_selector: "(pointer: none)" },
    ModifierMeta { key: "portrait", priority: 65, css_selector: "(orientation: portrait)" },
    ModifierMeta { key: "print", priority: 65, css_selector: "print" },
    ModifierMeta { key: "read-only", priority: 20, css_selector: "&:read-only" },
    ModifierMeta { key: "required", priority: 20, css_selector: "&:required" },
    ModifierMeta { key: "rtl", priority: 25, css_selector: "&:where(:dir(rtl), [dir=\"rtl\"], [dir=\"rtl\"] *)" },
    ModifierMeta { key: "selection", priority: 20, css_selector: "&::selection" },
    ModifierMeta { key: "sm", priority: 1640, css_selector: "(min-width: 640px)" },
    ModifierMeta { key: "target", priority: 20, css_selector: "&:target" },
    ModifierMeta { key: "user-invalid", priority: 20, css_selector: "&:user-invalid" },
    ModifierMeta { key: "user-valid", priority: 20, css_selector: "&:user-valid" },
    ModifierMeta { key: "valid", priority: 20, css_selector: "&:valid" },
    ModifierMeta { key: "visited", priority: 20, css_selector: "&:visited" },
    ModifierMeta { key: "xl", priority: 2280, css_selector: "(min-width: 1280px)" },
];

/// 根据修饰符 key 二分查找对应的元数据配置
pub fn lookup_modifier_meta(key: &str) -> Option<&'static ModifierMeta> {
    let idx = MODIFIER_TABLE.binary_search_by_key(&key, |m| m.key).ok()?;
    Some(&MODIFIER_TABLE[idx])
}

fn split_state_and_name_fast(rest: &str) -> (String, Option<String>) {
    if let Some(slash_idx) = rest.rfind('/') {
        let name_part = &rest[slash_idx + 1..];
        let state_part = &rest[..slash_idx];
        if !name_part.is_empty()
            && name_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let open_brackets = state_part.chars().filter(|&c| c == '[').count();
            let close_brackets = state_part.chars().filter(|&c| c == ']').count();
            if open_brackets == close_brackets {
                return (state_part.to_string(), Some(name_part.to_string()));
            }
        }
    }
    (rest.to_string(), None)
}

fn parse_bracket_kv_fast(rest: &str) -> (String, Option<String>) {
    if rest.starts_with('[') && rest.ends_with(']') {
        let inner = &rest[1..rest.len() - 1];
        if let Some((k, v)) = inner.split_once('=') {
            (k.to_string(), Some(v.to_string()))
        } else {
            (inner.to_string(), None)
        }
    } else {
        (rest.to_string(), None)
    }
}

fn parse_container_query_fast(container_spec: &str) -> Modifier {
    let (c_name, spec) = if let Some((name, rest)) = container_spec.split_once('/') {
        (Some(name.to_string()), rest)
    } else {
        (None, container_spec)
    };

    let min_width = match spec {
        "sm" => "640px".to_string(),
        "md" => "768px".to_string(),
        "lg" => "1024px".to_string(),
        "xl" => "1280px".to_string(),
        "2xl" => "1536px".to_string(),
        _ => {
            let cleaned = spec.strip_prefix("min-").unwrap_or(spec);
            let cleaned = cleaned.strip_prefix('-').unwrap_or(cleaned);
            if cleaned.starts_with('[') && cleaned.ends_with(']') {
                cleaned[1..cleaned.len() - 1].to_string()
            } else {
                cleaned.to_string()
            }
        }
    };

    Modifier::ContainerQuery {
        name: c_name,
        min_width,
    }
}

/// 编译期生成的 Modifier 状态机/快速匹配器
pub fn parse_modifier_fast(prefix: &str) -> Option<Modifier> {
    // 1. 编译期静态 match 确切 Modifier (比二分查找更快的 Match DFA)
    match prefix {
        "*" => return Some(Modifier::Child),
        "**" => return Some(Modifier::Descendant),
        "2xl" => return Some(Modifier::MediaBreakpoint("2xl".to_string())),
        "active" => return Some(Modifier::PseudoClass("active".to_string())),
        "after" => return Some(Modifier::PseudoElement("after".to_string())),
        "any-pointer-coarse" => {
            return Some(Modifier::MediaQuery("(any-pointer: coarse)".to_string()));
        }
        "any-pointer-fine" => return Some(Modifier::MediaQuery("(any-pointer: fine)".to_string())),
        "any-pointer-none" => return Some(Modifier::MediaQuery("(any-pointer: none)".to_string())),
        "autofill" => return Some(Modifier::PseudoClass("autofill".to_string())),
        "backdrop" => return Some(Modifier::PseudoElement("backdrop".to_string())),
        "before" => return Some(Modifier::PseudoElement("before".to_string())),
        "checked" => return Some(Modifier::PseudoClass("checked".to_string())),
        "contrast-less" => {
            return Some(Modifier::MediaQuery("(prefers-contrast: less)".to_string()));
        }
        "contrast-more" => {
            return Some(Modifier::MediaQuery("(prefers-contrast: more)".to_string()));
        }
        "dark" => return Some(Modifier::Dark),
        "default" => return Some(Modifier::PseudoClass("default".to_string())),
        "details-content" => return Some(Modifier::PseudoElement("details-content".to_string())),
        "disabled" => return Some(Modifier::PseudoClass("disabled".to_string())),
        "empty" => return Some(Modifier::PseudoClass("empty".to_string())),
        "enabled" => return Some(Modifier::PseudoClass("enabled".to_string())),
        "even" => return Some(Modifier::PseudoClass("even".to_string())),
        "file" => return Some(Modifier::PseudoElement("file".to_string())),
        "first" => return Some(Modifier::PseudoClass("first".to_string())),
        "first-letter" => return Some(Modifier::PseudoElement("first-letter".to_string())),
        "first-line" => return Some(Modifier::PseudoElement("first-line".to_string())),
        "first-of-type" => return Some(Modifier::PseudoClass("first-of-type".to_string())),
        "focus" => return Some(Modifier::PseudoClass("focus".to_string())),
        "focus-visible" => return Some(Modifier::PseudoClass("focus-visible".to_string())),
        "focus-within" => return Some(Modifier::PseudoClass("focus-within".to_string())),
        "forced-colors" => {
            return Some(Modifier::MediaQuery("(forced-colors: active)".to_string()));
        }
        "hover" => return Some(Modifier::PseudoClass("hover".to_string())),
        "in-range" => return Some(Modifier::PseudoClass("in-range".to_string())),
        "indeterminate" => return Some(Modifier::PseudoClass("indeterminate".to_string())),
        "inert" => {
            return Some(Modifier::SelectorVariant(
                "&:is([inert], [inert] *)".to_string(),
            ));
        }
        "invalid" => return Some(Modifier::PseudoClass("invalid".to_string())),
        "inverted-colors" => {
            return Some(Modifier::MediaQuery(
                "(inverted-colors: inverted)".to_string(),
            ));
        }
        "landscape" => return Some(Modifier::MediaQuery("(orientation: landscape)".to_string())),
        "last" => return Some(Modifier::PseudoClass("last".to_string())),
        "last-of-type" => return Some(Modifier::PseudoClass("last-of-type".to_string())),
        "lg" => return Some(Modifier::MediaBreakpoint("lg".to_string())),
        "ltr" => {
            return Some(Modifier::SelectorVariant(
                "&:where(:dir(ltr), [dir=\"ltr\"], [dir=\"ltr\"] *)".to_string(),
            ));
        }
        "marker" => return Some(Modifier::PseudoElement("marker".to_string())),
        "md" => return Some(Modifier::MediaBreakpoint("md".to_string())),
        "motion-reduce" => {
            return Some(Modifier::MediaQuery(
                "(prefers-reduced-motion: reduce)".to_string(),
            ));
        }
        "motion-safe" => {
            return Some(Modifier::MediaQuery(
                "(prefers-reduced-motion: no-preference)".to_string(),
            ));
        }
        "noscript" => return Some(Modifier::MediaQuery("(scripting: none)".to_string())),
        "odd" => return Some(Modifier::PseudoClass("odd".to_string())),
        "only" => return Some(Modifier::PseudoClass("only".to_string())),
        "only-of-type" => return Some(Modifier::PseudoClass("only-of-type".to_string())),
        "open" => {
            return Some(Modifier::SelectorVariant(
                "&:is([open], :popover-open, :open)".to_string(),
            ));
        }
        "optional" => return Some(Modifier::PseudoClass("optional".to_string())),
        "out-of-range" => return Some(Modifier::PseudoClass("out-of-range".to_string())),
        "placeholder" => return Some(Modifier::PseudoElement("placeholder".to_string())),
        "placeholder-shown" => return Some(Modifier::PseudoClass("placeholder-shown".to_string())),
        "pointer-coarse" => return Some(Modifier::MediaQuery("(pointer: coarse)".to_string())),
        "pointer-fine" => return Some(Modifier::MediaQuery("(pointer: fine)".to_string())),
        "pointer-none" => return Some(Modifier::MediaQuery("(pointer: none)".to_string())),
        "portrait" => return Some(Modifier::MediaQuery("(orientation: portrait)".to_string())),
        "print" => return Some(Modifier::MediaQuery("print".to_string())),
        "read-only" => return Some(Modifier::PseudoClass("read-only".to_string())),
        "required" => return Some(Modifier::PseudoClass("required".to_string())),
        "rtl" => {
            return Some(Modifier::SelectorVariant(
                "&:where(:dir(rtl), [dir=\"rtl\"], [dir=\"rtl\"] *)".to_string(),
            ));
        }
        "selection" => return Some(Modifier::PseudoElement("selection".to_string())),
        "sm" => return Some(Modifier::MediaBreakpoint("sm".to_string())),
        "target" => return Some(Modifier::PseudoClass("target".to_string())),
        "user-invalid" => return Some(Modifier::PseudoClass("user-invalid".to_string())),
        "user-valid" => return Some(Modifier::PseudoClass("user-valid".to_string())),
        "valid" => return Some(Modifier::PseudoClass("valid".to_string())),
        "visited" => return Some(Modifier::PseudoClass("visited".to_string())),
        "xl" => return Some(Modifier::MediaBreakpoint("xl".to_string())),
        _ => {}
    }

    // 2. 前缀状态匹配 (Prefix Dispatcher)
    if let Some(spec) = prefix.strip_prefix('@') {
        return Some(parse_container_query_fast(spec));
    }

    if let Some(rest) = prefix.strip_prefix("group-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Group { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("peer-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Peer { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("data-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        return Some(Modifier::DataAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("aria-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        let value = value.or_else(|| Some("true".to_string()));
        return Some(Modifier::AriaAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("has-") {
        return Some(Modifier::Has(format!("has-{}", rest)));
    }

    if prefix.starts_with('[') && prefix.ends_with(']') {
        return Some(Modifier::CustomSelector(
            prefix[1..prefix.len() - 1].to_string(),
        ));
    }

    None
}
