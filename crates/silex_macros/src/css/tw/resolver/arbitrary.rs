use crate::css::tw::ast::{SpannedModifier, UtilityRule, UtilityValue};
use proc_macro2::Span;
use silex_tw_core::{
    ValueKind, arbitrary_dispatch, classify_arbitrary_value, prefix::lookup_color_prefix,
};
use syn::{Error, Result};

use super::codegen::prefix_metadata::lookup_prefix_meta;
use super::{RING_BOX_SHADOW, expand_color_prefix_rule, kw, make_rule};

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

    // 多义前缀按**值的类型**分派，而不是靠"先查哪张表"的隐式顺序（报告 §2.8）。
    // `border-s-[3px]` 是宽度、`border-s-[red]` 是颜色，二者只能靠值区分。
    let kind = classify_arbitrary_value(&norm_val);

    // `ring-[…]` / `ring-offset-[…]` 的宽度形态还要额外铺 box-shadow 载体
    let is_sized = matches!(kind, ValueKind::Length | ValueKind::Number);
    let ring_width_prop = match clean_prefix {
        "ring" if is_sized => Some("--tw-ring-width"),
        "ring-offset" if is_sized => Some("--tw-ring-offset-width"),
        _ => None,
    };
    if let Some(prop) = ring_width_prop {
        let value = build_value(&norm_val, None, span)?;
        return Ok(vec![
            make_rule(modifiers.clone(), prop, value, span)?,
            make_rule(modifiers, "box-shadow", kw(RING_BOX_SHADOW), span)?,
        ]);
    }

    // 1. 显式登记的多义分派（`bg-[url(…)]` → background-image、`text-[14px]` → font-size）
    if let Some(props) = arbitrary_dispatch(clean_prefix, kind) {
        return emit(&modifiers, props, &norm_val, None, span);
    }

    // 2. 值确实是颜色时才让颜色前缀表接管。
    //    反过来（颜色表优先）会把 `border-s-[3px]` 判成 `border-inline-start-color`、
    //    把 `shadow-[0_0_0_1px_red]` 判成 `--tw-shadow-color`。
    let size_meta = lookup_prefix_meta(clean_prefix);
    let color_rule = lookup_color_prefix(clean_prefix);

    let color_first = kind == ValueKind::Color;
    let color_path = |modifiers: &[SpannedModifier]| -> Option<Result<Vec<UtilityRule>>> {
        let rule = color_rule?;
        Some(
            build_value(&norm_val, None, span)
                .and_then(|value| expand_color_prefix_rule(modifiers, rule, value, span)),
        )
    };
    let size_path = |modifiers: &[SpannedModifier]| -> Option<Result<Vec<UtilityRule>>> {
        let meta = size_meta?;
        Some(emit(
            modifiers,
            meta.target_props,
            &norm_val,
            meta.value_wrapper,
            span,
        ))
    };

    if color_first && let Some(rules) = color_path(&modifiers) {
        return rules;
    }
    if let Some(rules) = size_path(&modifiers) {
        return rules;
    }
    // 类型判不出来（`bg-[var(--x)]` 之类）且没有尺寸元数据时，仍按颜色前缀解释
    if let Some(rules) = color_path(&modifiers) {
        return rules;
    }

    // 3. 兜底：把前缀本身当作 CSS 属性名（`[mask-type]-[luminance]` 之类）
    emit(
        &modifiers,
        std::slice::from_ref(&clean_prefix),
        &norm_val,
        None,
        span,
    )
}

/// 把一个任意值写进一组目标属性
fn emit(
    modifiers: &[SpannedModifier],
    props: &[&str],
    norm_val: &str,
    value_wrapper: Option<&'static str>,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let value = build_value(norm_val, value_wrapper, span)?;
    props
        .iter()
        .map(|&prop| make_rule(modifiers.to_vec(), prop, value.clone(), span))
        .collect()
}

/// 把规范化后的任意值构造成 `UtilityValue`
fn build_value(
    norm_val: &str,
    value_wrapper: Option<&'static str>,
    span: Span,
) -> Result<UtilityValue> {
    if let Some(expr_inner) = norm_val
        .strip_prefix("$(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let expr: syn::Expr =
            syn::parse_str(expr_inner).map_err(|e| Error::new(span, e.to_string()))?;
        return Ok(UtilityValue::DynamicExpr(expr, span));
    }

    let val_str = if norm_val.starts_with("--") {
        format!("var({})", norm_val)
    } else {
        norm_val.to_string()
    };

    Ok(UtilityValue::ArbitraryLiteral(match value_wrapper {
        Some(wrapper) => wrapper.replace("{}", &val_str),
        None => val_str,
    }))
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
