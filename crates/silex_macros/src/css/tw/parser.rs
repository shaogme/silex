use crate::css::tw::ast::{Modifier, TwInput, TwSegment};
use crate::css::tw::resolver::resolve_utility;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, Result, Token, parenthesized};

#[inline]
fn is_marker_class(token: &str) -> bool {
    token == "group" || token == "peer" || token.starts_with("group/") || token.starts_with("peer/")
}

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

impl Parse for TwInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut segments = Vec::new();
        let mut extra_classes = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                let raw_str = lit.value();
                let span = lit.span();
                let mut rules = Vec::new();

                for token in raw_str.split_whitespace() {
                    let (modifiers, body_token) = parse_modifiers_and_body(token);
                    if modifiers.is_empty()
                        && is_marker_class(body_token)
                        && !extra_classes.contains(&body_token.to_string())
                    {
                        extra_classes.push(body_token.to_string());
                    }
                    let mut resolved = resolve_utility(modifiers, body_token, span)?;
                    rules.append(&mut resolved);
                }
                segments.push(TwSegment::Static(rules));
            } else if input.peek(syn::token::Paren) {
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

                let mut then_rules = Vec::new();
                let raw_str = lit.value();
                let lit_span = lit.span();
                for token in raw_str.split_whitespace() {
                    let (modifiers, body_token) = parse_modifiers_and_body(token);
                    if modifiers.is_empty()
                        && is_marker_class(body_token)
                        && !extra_classes.contains(&body_token.to_string())
                    {
                        extra_classes.push(body_token.to_string());
                    }
                    let mut resolved = resolve_utility(modifiers, body_token, lit_span)?;
                    then_rules.append(&mut resolved);
                }

                let mut else_rules = Vec::new();
                if let Some(else_l) = else_lit {
                    let raw_str = else_l.value();
                    let else_span = else_l.span();
                    for token in raw_str.split_whitespace() {
                        let (modifiers, body_token) = parse_modifiers_and_body(token);
                        if modifiers.is_empty()
                            && is_marker_class(body_token)
                            && !extra_classes.contains(&body_token.to_string())
                        {
                            extra_classes.push(body_token.to_string());
                        }
                        let mut resolved = resolve_utility(modifiers, body_token, else_span)?;
                        else_rules.append(&mut resolved);
                    }
                }

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

/// 剥离修饰符前缀（如 `hover:`, `md:`, `dark:`）与基础 Utility Token
pub(crate) fn parse_modifiers_and_body(token: &str) -> (Vec<Modifier>, &str) {
    let mut modifiers = Vec::new();
    let mut current = token;

    while let Some((prefix, rest)) = split_modifier(current) {
        let is_custom_bp = crate::css::config::get_config()
            .map(|cfg| cfg.theme.breakpoints.contains_key(prefix))
            .unwrap_or(false);

        let modifier = if let Some(container_spec) = prefix.strip_prefix('@') {
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
        } else if prefix == "*" {
            Modifier::Child
        } else if prefix == "**" {
            Modifier::Descendant
        } else if is_custom_bp || matches!(prefix, "sm" | "md" | "lg" | "xl" | "2xl") {
            Modifier::MediaBreakpoint(prefix.to_string())
        } else {
            match prefix {
                "hover" | "focus" | "active" | "disabled" | "visited" | "first" | "last"
                | "odd" | "even" => Modifier::PseudoClass(prefix.to_string()),
                "before" | "after" | "placeholder" => Modifier::PseudoElement(prefix.to_string()),
                "dark" => Modifier::Dark,
                _ => {
                    if let Some(rest) = prefix.strip_prefix("group-") {
                        let (state, name) = split_state_and_name(rest);
                        Modifier::Group { state, name }
                    } else if let Some(rest) = prefix.strip_prefix("peer-") {
                        let (state, name) = split_state_and_name(rest);
                        Modifier::Peer { state, name }
                    } else if prefix.starts_with("data-[") && prefix.ends_with(']') {
                        let inner = &prefix[6..prefix.len() - 1];
                        if let Some((k, v)) = inner.split_once('=') {
                            Modifier::DataAttribute {
                                key: k.to_string(),
                                value: Some(v.to_string()),
                            }
                        } else {
                            Modifier::DataAttribute {
                                key: inner.to_string(),
                                value: None,
                            }
                        }
                    } else if prefix.starts_with("aria-[") && prefix.ends_with(']') {
                        let inner = &prefix[6..prefix.len() - 1];
                        if let Some((k, v)) = inner.split_once('=') {
                            Modifier::AriaAttribute {
                                key: k.to_string(),
                                value: Some(v.to_string()),
                            }
                        } else {
                            Modifier::AriaAttribute {
                                key: inner.to_string(),
                                value: None,
                            }
                        }
                    } else if let Some(rest) = prefix.strip_prefix("aria-") {
                        Modifier::AriaAttribute {
                            key: rest.to_string(),
                            value: Some("true".to_string()),
                        }
                    } else if prefix.starts_with("has-[") && prefix.ends_with(']')
                        || prefix.starts_with("has-data-[") && prefix.ends_with(']')
                    {
                        Modifier::Has(prefix.to_string())
                    } else if prefix.starts_with('[') && prefix.ends_with(']') {
                        Modifier::CustomSelector(prefix[1..prefix.len() - 1].to_string())
                    } else {
                        Modifier::PseudoClass(prefix.to_string())
                    }
                }
            }
        };

        modifiers.push(modifier);
        current = rest;
    }

    (modifiers, current)
}

fn split_modifier(s: &str) -> Option<(&str, &str)> {
    if let Some(colon_idx) = s.find(':') {
        let prefix = &s[..colon_idx];
        if prefix.contains('[') && !prefix.contains(']') {
            if let Some(close_idx) = s.find(']')
                && let Some(next_colon) = s[close_idx..].find(':')
            {
                let real_colon = close_idx + next_colon;
                return Some((&s[..real_colon], &s[real_colon + 1..]));
            }
            return None;
        }
        return Some((prefix, &s[colon_idx + 1..]));
    }
    None
}
