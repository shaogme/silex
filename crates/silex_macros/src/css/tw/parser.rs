use crate::css::{
    config::get_config,
    tw::{
        ast::{Modifier, SpannedModifier, TwInput, TwSegment, UtilityRule},
        resolver::{
            codegen::modifiers::parse_modifier_fast, is_marker_class, resolve_utility,
            suggest::find_best_modifier_suggestion,
        },
    },
};
use proc_macro2::Span;
use syn::{
    Error, Expr, LitStr, Result, Token, parenthesized,
    parse::{Parse, ParseStream},
    token::Paren,
};

fn parse_class_string(
    raw_str: &str,
    span: Span,
    extra_classes: &mut Vec<String>,
) -> Result<Vec<UtilityRule>> {
    let mut rules = Vec::new();
    for token in raw_str.split_whitespace() {
        let (modifiers, body_token) = parse_modifiers_and_body(token, span)?;
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

/// 解析单个修饰符前缀字符串为 `Modifier`
///
/// 未知前缀会**报编译错误**并给出 Levenshtein 建议，绝不静默降级为伪类：
/// LightningCSS 不会拒绝未知伪类，`mdd:flex` 之类的拼写错误会一路生成
/// 永远不匹配任何元素的 `:mdd` 规则。若确需透传自定义伪类，请写作 `[&:my-pseudo]:`。
pub(crate) fn parse_single_modifier(prefix: &str, span: Span) -> Result<Modifier> {
    // 1. 调用 Codegen 生成的 0-alloc Match DFA / 状态机快速解析
    if let Some(m) = parse_modifier_fast(prefix) {
        return Ok(m);
    }

    // 2. 自定义断点响应式匹配
    let is_custom_bp = get_config()
        .map(|cfg| cfg.theme.breakpoints.contains_key(prefix))
        .unwrap_or(false);
    if is_custom_bp {
        return Ok(Modifier::MediaBreakpoint(prefix.to_string()));
    }

    // 3. Tailwind 已定义但本实现尚未支持的函数式变体：给出明确的“未支持”提示，
    //    避免退化成毫无帮助的拼写建议
    if let Some(family) = unsupported_functional_family(prefix) {
        return Err(Error::new(
            span,
            format!(
                "Variant '{}:' uses the Tailwind functional variant '{}-*', which is not supported yet. \
                 Use an arbitrary variant such as `[&:...]:` instead.",
                prefix, family
            ),
        ));
    }

    // 4. 未知前缀：报错并给出建议
    let msg = match find_best_modifier_suggestion(prefix) {
        Some(s) => format!(
            "Unknown variant prefix '{}:'. Did you mean '{}:'? \
             (use `[&:{}]:` to pass an arbitrary pseudo-class through)",
            prefix, s, prefix
        ),
        None => format!(
            "Unknown variant prefix '{}:'. \
             Use `[&:{}]:` if you intend to emit an arbitrary pseudo-class.",
            prefix, prefix
        ),
    };
    Err(Error::new(span, msg))
}

/// 识别 Tailwind v4 中存在、但本实现尚未支持的函数式变体家族
fn unsupported_functional_family(prefix: &str) -> Option<&'static str> {
    const FAMILIES: &[&str] = &[
        "not-",
        "in-",
        "nth-",
        "nth-last-",
        "nth-of-type-",
        "nth-last-of-type-",
        "supports-",
        "min-",
        "max-",
        "describedby-",
        "details-content-",
    ];
    FAMILIES
        .iter()
        .copied()
        .find(|f| prefix.starts_with(f))
        .map(|f| f.trim_end_matches('-'))
}

/// 剥离修饰符前缀（如 `hover:`, `md:`, `dark:`）与基础 Utility Token，并附加细粒度 Span
pub(crate) fn parse_modifiers_and_body(
    token: &str,
    span: Span,
) -> Result<(Vec<SpannedModifier>, &str)> {
    let mut modifiers = Vec::new();
    let mut current = token;

    while let Some((prefix, rest)) = split_modifier(current) {
        let modifier = parse_single_modifier(prefix, span)?;
        modifiers.push(SpannedModifier::new(modifier, span));
        current = rest;
    }

    Ok((modifiers, current))
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
