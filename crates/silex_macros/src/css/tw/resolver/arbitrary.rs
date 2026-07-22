use crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};
use proc_macro2::Span;
use syn::{Error, Result};

use super::{DIVIDE_SELECTOR, RING_BOX_SHADOW, kw, make_rule};

/// 任意值与任意属性语法解析: `w-[12px]`, `bg-[red]`, `[--tw-ring-color:rgba(79,70,229,.2)]`, `[color:red]`
pub fn parse_arbitrary_syntax(token: &str) -> Option<(&str, &str)> {
    if let Some(open_idx) = token.find('[')
        && token.ends_with(']')
    {
        let prefix = &token[..open_idx];
        let raw_val = &token[open_idx + 1..token.len() - 1];
        return Some((prefix, raw_val));
    }
    None
}

fn normalize_arbitrary_val(raw_val: &str) -> String {
    let mut out = String::with_capacity(raw_val.len() + 8);
    let mut chars = raw_val.chars().peekable();
    let mut dollar_depth: usize = 0;

    while let Some(c) = chars.next() {
        // 1. 识别 Silex 动态表达式起点 `$(...`
        if c == '$' && chars.peek() == Some(&'(') {
            dollar_depth += 1;
            out.push('$');
            out.push('(');
            chars.next(); // 消费 '('
            continue;
        }

        // 2. 在 `$(...)` 保护区域内部：原样透传，保护 Rust 标识符与表达式语法
        if dollar_depth > 0 {
            out.push(c);
            if c == '(' {
                dollar_depth += 1;
            } else if c == ')' {
                dollar_depth -= 1;
            }
            continue;
        }

        // 3. 在 `$(...)` 区域外部：统一规范化 Tailwind 任意值与 calc(...) 运算符
        if c == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else if (c == '+' || c == '-') && !out.is_empty() {
            let prev = out.chars().last().unwrap_or(' ');
            let next = chars.peek().copied().unwrap_or(' ');

            // 检查 '-' 是否为标识符/CSS变量/前缀的一部分（如 -10px, --tw-var, 100%）
            let is_identifier_hyphen = c == '-'
                && (prev == '-'
                    || next == '-'
                    || (prev.is_alphanumeric() && next.is_alphanumeric()));

            if is_identifier_hyphen {
                out.push(c);
            } else {
                if !prev.is_whitespace() && prev != '(' && prev != ',' {
                    out.push(' ');
                }
                out.push(c);
                if !next.is_whitespace() && next != ')' && next != ',' {
                    out.push(' ');
                }
            }
        } else {
            out.push(c);
        }
    }

    out
}

/// 解析任意值到 UtilityRule
pub fn resolve_arbitrary(
    modifiers: Vec<Modifier>,
    prefix: &str,
    raw_val: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let norm_val = normalize_arbitrary_val(raw_val);

    // 1. 处理任意属性语法 `[property:value]`, 如 `[--tw-ring-color:rgba(79,70,229,.2)]` 或 `[color:red]`
    if prefix.is_empty() {
        if let Some((prop, val_str)) = norm_val.split_once(':') {
            let value = if val_str.starts_with("$(") && val_str.ends_with(')') {
                let expr_inner = &val_str[2..val_str.len() - 1];
                let expr: syn::Expr =
                    syn::parse_str(expr_inner).map_err(|e| Error::new(span, e.to_string()))?;
                UtilityValue::DynamicExpr(expr, span)
            } else {
                UtilityValue::ArbitraryLiteral(val_str.to_string())
            };

            let mut rules = vec![make_rule(modifiers.clone(), prop, value, span)];
            if prop == "--tw-ring-color" {
                rules.push(make_rule(
                    modifiers,
                    "box-shadow",
                    kw(RING_BOX_SHADOW),
                    span,
                ));
            }
            return Ok(rules);
        }
        return Err(Error::new(
            span,
            format!("Invalid arbitrary property syntax '[{}]'", raw_val),
        ));
    }

    // 2. 处理带有前缀的任意值语法 `w-[12px]`, `ring-[rgba(79,70,229,.2)]`, `bg-[rgba(79,70,229,.2)]`
    let clean_prefix = prefix.strip_suffix('-').unwrap_or(prefix);

    let (prop, is_divide) = match clean_prefix {
        "p" | "padding" => ("padding", false),
        "px" => ("padding-inline", false),
        "py" => ("padding-block", false),
        "pt" => ("padding-top", false),
        "pr" => ("padding-right", false),
        "pb" => ("padding-bottom", false),
        "pl" => ("padding-left", false),
        "m" | "margin" => ("margin", false),
        "mx" => ("margin-inline", false),
        "my" => ("margin-block", false),
        "mt" => ("margin-top", false),
        "mr" => ("margin-right", false),
        "mb" => ("margin-bottom", false),
        "ml" => ("margin-left", false),
        "w" | "width" => ("width", false),
        "h" | "height" => ("height", false),
        "bg" => ("background-color", false),
        "text" => ("color", false),
        "border" => ("border-color", false),
        "border-t" => ("border-top-color", false),
        "border-r" => ("border-right-color", false),
        "border-b" => ("border-bottom-color", false),
        "border-l" => ("border-left-color", false),
        "outline" => ("outline-color", false),
        "rounded" => ("border-radius", false),
        "top" => ("top", false),
        "right" => ("right", false),
        "bottom" => ("bottom", false),
        "left" => ("left", false),
        "z" => ("z-index", false),
        "opacity" => ("opacity", false),
        "blur" => ("filter", false),
        "backdrop-blur" => ("backdrop-filter", false),
        "scale" | "scale-x" | "scale-y" | "rotate" | "translate-x" | "translate-y" => {
            ("transform", false)
        }
        "animate" => ("animation", false),
        "container" | "container-name" => ("container-name", false),
        "grid-rows" => ("grid-template-rows", false),
        "grid-cols" => ("grid-template-columns", false),
        "auto-rows" => ("grid-auto-rows", false),
        "auto-cols" => ("grid-auto-columns", false),
        "ring" => {
            if norm_val.ends_with("px")
                || norm_val.ends_with("rem")
                || norm_val.ends_with("em")
                || norm_val.parse::<f64>().is_ok()
            {
                ("--tw-ring-width", false)
            } else {
                ("--tw-ring-color", false)
            }
        }
        "ring-offset" => {
            if norm_val.ends_with("px")
                || norm_val.ends_with("rem")
                || norm_val.ends_with("em")
                || norm_val.parse::<f64>().is_ok()
            {
                ("--tw-ring-offset-width", false)
            } else {
                ("--tw-ring-offset-color", false)
            }
        }
        "aspect" => ("aspect-ratio", false),
        "object" => ("object-fit", false),
        "col" => ("grid-column", false),
        "row" => ("grid-row", false),
        "line-clamp" => ("-webkit-line-clamp", false),
        "accent" => ("accent-color", false),
        "caret" => ("caret-color", false),
        "fill" => ("fill", false),
        "stroke" => ("stroke", false),
        "shadow" => ("box-shadow", false),
        "from" => ("--tw-gradient-from", false),
        "via" => ("--tw-gradient-via", false),
        "to" => ("--tw-gradient-to", false),
        "divide" => ("border-color", true),
        _ => (clean_prefix, false),
    };

    let target_mods = if is_divide {
        [
            modifiers.clone(),
            vec![Modifier::CustomSelector(DIVIDE_SELECTOR.into())],
        ]
        .concat()
    } else {
        modifiers.clone()
    };

    let value = if norm_val.starts_with("$(") && norm_val.ends_with(')') {
        let expr_inner = &norm_val[2..norm_val.len() - 1];
        let expr: syn::Expr =
            syn::parse_str(expr_inner).map_err(|e| Error::new(span, e.to_string()))?;
        UtilityValue::DynamicExpr(expr, span)
    } else {
        match clean_prefix {
            "translate-x" => UtilityValue::ArbitraryLiteral(format!("translateX({})", norm_val)),
            "translate-y" => UtilityValue::ArbitraryLiteral(format!("translateY({})", norm_val)),
            "rotate" => UtilityValue::ArbitraryLiteral(format!("rotate({})", norm_val)),
            "scale" => UtilityValue::ArbitraryLiteral(format!("scale({})", norm_val)),
            "scale-x" => UtilityValue::ArbitraryLiteral(format!("scaleX({})", norm_val)),
            "scale-y" => UtilityValue::ArbitraryLiteral(format!("scaleY({})", norm_val)),
            _ => UtilityValue::ArbitraryLiteral(norm_val),
        }
    };

    if clean_prefix == "from" {
        return Ok(vec![
            make_rule(target_mods.clone(), "--tw-gradient-from", value, span),
            make_rule(
                target_mods.clone(),
                "--tw-gradient-to",
                kw("rgb(255 255 255 / 0)"),
                span,
            ),
            make_rule(
                target_mods,
                "--tw-gradient-stops",
                kw("var(--tw-gradient-from), var(--tw-gradient-to)"),
                span,
            ),
        ]);
    }

    if clean_prefix == "via" {
        return Ok(vec![
            make_rule(target_mods.clone(), "--tw-gradient-via", value, span),
            make_rule(
                target_mods,
                "--tw-gradient-stops",
                kw("var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)"),
                span,
            ),
        ]);
    }

    let mut rules = vec![make_rule(target_mods.clone(), prop, value, span)];
    if prop.starts_with("--tw-ring-") {
        rules.push(make_rule(
            target_mods,
            "box-shadow",
            kw(RING_BOX_SHADOW),
            span,
        ));
    }

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_arbitrary_val() {
        assert_eq!(
            normalize_arbitrary_val("calc(100%-2px)"),
            "calc(100% - 2px)"
        );
        assert_eq!(
            normalize_arbitrary_val("calc(100%_-_2px)"),
            "calc(100% - 2px)"
        );
        assert_eq!(
            normalize_arbitrary_val("calc(50%+10px)"),
            "calc(50% + 10px)"
        );
        assert_eq!(normalize_arbitrary_val("-10px"), "-10px");
        assert_eq!(normalize_arbitrary_val("$(pad_val)"), "$(pad_val)");
    }

    #[test]
    fn test_resolve_arbitrary_dynamic_expr_with_underscores() {
        let rules = resolve_arbitrary(vec![], "p", "$(pad_val)", Span::call_site()).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "padding");
        assert!(matches!(rules[0].value, UtilityValue::DynamicExpr(_, _)));
    }

    #[test]
    fn test_resolve_arbitrary_translate_x_calc() {
        let rules =
            resolve_arbitrary(vec![], "translate-x", "calc(100%-2px)", Span::call_site()).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        if let UtilityValue::ArbitraryLiteral(s) = &rules[0].value {
            assert_eq!(s, "translateX(calc(100% - 2px))");
        } else {
            panic!("Expected ArbitraryLiteral");
        }
    }
}
