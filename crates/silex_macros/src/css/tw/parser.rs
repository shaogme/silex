use crate::css::tw::ast::{Modifier, TwInput, UtilityValue};
use crate::css::tw::resolver::resolve_utility;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, Result, Token, parenthesized};

impl Parse for TwInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut rules = Vec::new();
        let mut extra_classes = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                let raw_str = lit.value();
                let span = lit.span();

                for token in raw_str.split_whitespace() {
                    let (modifiers, body_token) = parse_modifiers_and_body(token);
                    if modifiers.is_empty() && (body_token == "group" || body_token == "peer") {
                        if !extra_classes.contains(&body_token.to_string()) {
                            extra_classes.push(body_token.to_string());
                        }
                    }
                    let mut resolved = resolve_utility(modifiers, body_token, span)?;
                    rules.append(&mut resolved);
                }
            } else if input.peek(syn::token::Paren) {
                // 条件句元组形态: ("bg-primary", is_active)
                let content;
                parenthesized!(content in input);
                let lit: LitStr = content.parse()?;
                content.parse::<Token![,]>()?;
                let cond_expr: Expr = content.parse()?;

                let raw_str = lit.value();
                let span = lit.span();
                for token in raw_str.split_whitespace() {
                    let (modifiers, body_token) = parse_modifiers_and_body(token);
                    if modifiers.is_empty() && (body_token == "group" || body_token == "peer") {
                        if !extra_classes.contains(&body_token.to_string()) {
                            extra_classes.push(body_token.to_string());
                        }
                    }
                    let mut sub_rules = resolve_utility(modifiers, body_token, span)?;
                    for rule in &mut sub_rules {
                        // 包装为动态表达式关联条件
                        rule.value = UtilityValue::DynamicExpr(cond_expr.clone(), span);
                    }
                    rules.append(&mut sub_rules);
                }
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
            rules,
            extra_classes,
        })
    }
}

/// 剥离修饰符前缀（如 `hover:`, `md:`, `dark:`）与基础 Utility Token
fn parse_modifiers_and_body(token: &str) -> (Vec<Modifier>, &str) {
    let mut modifiers = Vec::new();
    let mut current = token;

    while let Some((prefix, rest)) = current.split_once(':') {
        // 排除任意值选择器 [*:hover] 的冒号
        if prefix.contains('[') {
            break;
        }

        let modifier = match prefix {
            "hover" | "focus" | "active" | "disabled" | "visited" | "first" | "last" | "odd"
            | "even" => Modifier::PseudoClass(prefix.to_string()),
            "before" | "after" | "placeholder" => Modifier::PseudoElement(prefix.to_string()),
            "sm" | "md" | "lg" | "xl" | "2xl" => Modifier::MediaBreakpoint(prefix.to_string()),
            "dark" => Modifier::Dark,
            _ => {
                if let Some(group_state) = prefix.strip_prefix("group-") {
                    Modifier::Group(group_state.to_string())
                } else if let Some(peer_state) = prefix.strip_prefix("peer-") {
                    Modifier::Peer(peer_state.to_string())
                } else if prefix.starts_with('[') && prefix.ends_with(']') {
                    Modifier::CustomSelector(prefix[1..prefix.len() - 1].to_string())
                } else {
                    Modifier::PseudoClass(prefix.to_string())
                }
            }
        };

        modifiers.push(modifier);
        current = rest;
    }

    (modifiers, current)
}
