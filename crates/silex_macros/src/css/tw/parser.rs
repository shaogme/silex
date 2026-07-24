use crate::css::{
    config::get_config,
    tw::{
        ast::{Modifier, SpannedModifier, TwInput, TwSegment, UtilityRule},
        resolver::{codegen::modifiers::lookup_modifier_meta, is_marker_class, resolve_utility},
    },
};
use proc_macro2::Span;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    token::Paren,
    Expr, LitStr, Result, Token,
};

fn split_state_and_name(rest: &str) -> (String, Option<String>) {
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

fn parse_class_string(
    raw_str: &str,
    span: Span,
    extra_classes: &mut Vec<String>,
) -> Result<Vec<UtilityRule>> {
    let mut rules = Vec::new();
    for token in raw_str.split_whitespace() {
        let (modifiers, body_token) = parse_modifiers_and_body(token, span);
        if modifiers.is_empty()
            && is_marker_class(body_token)
            && !extra_classes.contains(&body_token.to_string())
        {
            extra_classes.push(body_token.to_string());
        }
        let mut resolved = resolve_utility(modifiers, body_token, span)?;
        rules.append(&mut resolved);
    }
    Ok(rules)
}

impl Parse for TwInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut segments = Vec::new();
        let mut extra_classes = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                let rules = parse_class_string(&lit.value(), lit.span(), &mut extra_classes)?;
                segments.push(TwSegment::Static(rules));
            } else if input.peek(Paren) {
                let content;
                parenthesized!(content in input);

                let (cond_expr, lit, else_lit) = if content.peek(LitStr) {
                    let lit: LitStr = content.parse()?;
                    content.parse::<Token![,]>()?;
                    let cond_expr: Expr = content.parse()?;
                    let else_lit: Option<LitStr> = if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        if content.peek(LitStr) {
                            Some(content.parse()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    (cond_expr, lit, else_lit)
                } else {
                    let cond_expr: Expr = content.parse()?;
                    content.parse::<Token![,]>()?;
                    let lit: LitStr = content.parse()?;
                    let else_lit: Option<LitStr> = if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        if content.peek(LitStr) {
                            Some(content.parse()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    (cond_expr, lit, else_lit)
                };

                let then_rules = parse_class_string(&lit.value(), lit.span(), &mut extra_classes)?;
                let else_rules = if let Some(else_l) = else_lit {
                    parse_class_string(&else_l.value(), else_l.span(), &mut extra_classes)?
                } else {
                    Vec::new()
                };

                segments.push(TwSegment::Conditional {
                    condition: cond_expr,
                    then_rules,
                    else_rules,
                });
            } else {
                return Err(
                    input.error("Expected string literal or conditional tuple in tw! macro")
                );
            }

            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(TwInput {
            segments,
            extra_classes,
        })
    }
}

fn parse_bracket_kv(rest: &str) -> Option<(String, Option<String>)> {
    if rest.starts_with('[') && rest.ends_with(']') {
        let inner = &rest[1..rest.len() - 1];
        if let Some((k, v)) = inner.split_once('=') {
            Some((k.to_string(), Some(v.to_string())))
        } else {
            Some((inner.to_string(), None))
        }
    } else {
        None
    }
}

fn parse_container_query(container_spec: &str) -> Modifier {
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

/// 解析单个修饰符前缀字符串为 `Modifier`
pub(crate) fn parse_single_modifier(prefix: &str) -> Modifier {
    // 1. 静态确切查表 O(log N)，直接调用 codegen 生成在 modifiers.rs 内的 to_modifier 方法
    if let Some(meta) = lookup_modifier_meta(prefix) {
        return meta.to_modifier(prefix);
    }

    // 2. 自定义断点响应式匹配
    let is_custom_bp = get_config()
        .map(|cfg| cfg.theme.breakpoints.contains_key(prefix))
        .unwrap_or(false);
    if is_custom_bp {
        return Modifier::MediaBreakpoint(prefix.to_string());
    }

    // 3. 结构化前缀分发器 (Prefix-Strip Dispatcher)
    if let Some(spec) = prefix.strip_prefix('@') {
        return parse_container_query(spec);
    }

    if let Some(rest) = prefix.strip_prefix("group-") {
        let (state, name) = split_state_and_name(rest);
        return Modifier::Group { state, name };
    }

    if let Some(rest) = prefix.strip_prefix("peer-") {
        let (state, name) = split_state_and_name(rest);
        return Modifier::Peer { state, name };
    }

    if let Some(rest) = prefix.strip_prefix("data-") {
        if let Some((key, value)) = parse_bracket_kv(rest) {
            return Modifier::DataAttribute { key, value };
        }
        return Modifier::DataAttribute {
            key: rest.to_string(),
            value: None,
        };
    }

    if let Some(rest) = prefix.strip_prefix("aria-") {
        if let Some((key, value)) = parse_bracket_kv(rest) {
            return Modifier::AriaAttribute { key, value };
        }
        return Modifier::AriaAttribute {
            key: rest.to_string(),
            value: Some("true".to_string()),
        };
    }

    if let Some(rest) = prefix.strip_prefix("has-") {
        return Modifier::Has(format!("has-{}", rest));
    }

    if prefix.starts_with('[') && prefix.ends_with(']') {
        return Modifier::CustomSelector(prefix[1..prefix.len() - 1].to_string());
    }

    // 4. 兜底伪类
    Modifier::PseudoClass(prefix.to_string())
}

/// 剥离修饰符前缀（如 `hover:`, `md:`, `dark:`）与基础 Utility Token，并附加细粒度 Span
pub(crate) fn parse_modifiers_and_body<'a>(
    token: &'a str,
    span: Span,
) -> (Vec<SpannedModifier>, &'a str) {
    let mut modifiers = Vec::new();
    let mut current = token;

    while let Some((prefix, rest)) = split_modifier(current) {
        let modifier = parse_single_modifier(prefix);
        modifiers.push(SpannedModifier::new(modifier, span));
        current = rest;
    }

    (modifiers, current)
}

fn split_modifier(s: &str) -> Option<(&str, &str)> {
    let colon_idx = s.find(':')?;
    let prefix = &s[..colon_idx];
    if prefix.contains('[') && !prefix.contains(']') {
        let close_idx = s.find(']')?;
        let next_colon = s[close_idx..].find(':')?;
        let real_colon = close_idx + next_colon;
        return Some((&s[..real_colon], &s[real_colon + 1..]));
    }
    Some((prefix, &s[colon_idx + 1..]))
}
