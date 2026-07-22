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

/// 解析任意值到 UtilityRule
pub fn resolve_arbitrary(
    modifiers: Vec<Modifier>,
    prefix: &str,
    raw_val: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    // 1. 处理任意属性语法 `[property:value]`, 如 `[--tw-ring-color:rgba(79,70,229,.2)]` 或 `[color:red]`
    if prefix.is_empty() {
        if let Some((prop, val_str)) = raw_val.split_once(':') {
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
        "ring" => ("--tw-ring-color", false),
        "ring-offset" => ("--tw-ring-offset-color", false),
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

    let value = if raw_val.starts_with("$(") && raw_val.ends_with(')') {
        let expr_inner = &raw_val[2..raw_val.len() - 1];
        let expr: syn::Expr =
            syn::parse_str(expr_inner).map_err(|e| Error::new(span, e.to_string()))?;
        UtilityValue::DynamicExpr(expr, span)
    } else {
        UtilityValue::ArbitraryLiteral(raw_val.to_string())
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
    if prop == "--tw-ring-color" {
        rules.push(make_rule(
            target_mods,
            "box-shadow",
            kw(RING_BOX_SHADOW),
            span,
        ));
    }

    Ok(rules)
}
