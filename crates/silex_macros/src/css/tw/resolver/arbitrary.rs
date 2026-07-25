use crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};
use proc_macro2::Span;
use syn::{Error, Result};

use super::codegen::prefix_metadata::lookup_prefix_meta;
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
    if let Some(open_idx) = token.find('(')
        && token.ends_with(')')
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
    modifiers: Vec<SpannedModifier>,
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

            let mut rules = vec![make_rule(modifiers.clone(), prop, value, span)?];
            if prop == "--tw-ring-color" {
                rules.push(make_rule(
                    modifiers,
                    "box-shadow",
                    kw(RING_BOX_SHADOW),
                    span,
                )?);
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

    let (target_props, is_divide, value_wrapper) =
        if let Some(p) = super::color_prefix_to_prop(clean_prefix) {
            (vec![p], clean_prefix == "divide", None)
        } else if clean_prefix == "ring" {
            let is_length = norm_val.ends_with("px")
                || norm_val.ends_with("rem")
                || norm_val.ends_with("em")
                || norm_val.parse::<f64>().is_ok();
            let prop = if is_length {
                "--tw-ring-width"
            } else {
                "--tw-ring-color"
            };
            (vec![prop], false, None)
        } else if clean_prefix == "ring-offset" {
            let is_length = norm_val.ends_with("px")
                || norm_val.ends_with("rem")
                || norm_val.ends_with("em")
                || norm_val.parse::<f64>().is_ok();
            let prop = if is_length {
                "--tw-ring-offset-width"
            } else {
                "--tw-ring-offset-color"
            };
            (vec![prop], false, None)
        } else if let Some(meta) = lookup_prefix_meta(clean_prefix) {
            (meta.target_props.to_vec(), false, meta.value_wrapper)
        } else {
            (vec![clean_prefix], false, None)
        };

    let target_mods = if is_divide {
        [
            modifiers.clone(),
            vec![SpannedModifier::new(
                Modifier::CustomSelector(DIVIDE_SELECTOR.into()),
                span,
            )],
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
        let val_str = if norm_val.starts_with("--") {
            format!("var({})", norm_val)
        } else {
            norm_val
        };

        if let Some(wrapper) = value_wrapper {
            UtilityValue::ArbitraryLiteral(wrapper.replace("{}", &val_str))
        } else {
            UtilityValue::ArbitraryLiteral(val_str)
        }
    };

    if clean_prefix == "from" {
        return Ok(vec![
            make_rule(target_mods.clone(), "--tw-gradient-from", value, span)?,
            make_rule(
                target_mods.clone(),
                "--tw-gradient-to",
                kw("rgb(255 255 255 / 0)"),
                span,
            )?,
            make_rule(
                target_mods,
                "--tw-gradient-stops",
                kw("var(--tw-gradient-from), var(--tw-gradient-to)"),
                span,
            )?,
        ]);
    }

    if clean_prefix == "via" {
        return Ok(vec![
            make_rule(target_mods.clone(), "--tw-gradient-via", value, span)?,
            make_rule(
                target_mods,
                "--tw-gradient-stops",
                kw("var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)"),
                span,
            )?,
        ]);
    }

    let mut rules = Vec::with_capacity(target_props.len() + 1);
    let mut has_ring_prop = false;
    for prop in target_props {
        if prop.starts_with("--tw-ring-") {
            has_ring_prop = true;
        }
        rules.push(make_rule(target_mods.clone(), prop, value.clone(), span)?);
    }

    if has_ring_prop {
        rules.push(make_rule(
            target_mods,
            "box-shadow",
            kw(RING_BOX_SHADOW),
            span,
        )?);
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
