use crate::css::ast::{CssAtRule, CssBlock, CssDeclaration, CssNested, CssRule};
use crate::css::tw::ast::{Modifier, TwInput, UtilityRule, UtilityValue};
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::{HashMap, HashSet};
use syn::Result;

/// 将解析后的 `TwInput` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_tw(input: TwInput) -> Result<CssBlock> {
    let mut root_raw_rules = Vec::new();
    let mut modifier_groups: HashMap<Vec<Modifier>, Vec<UtilityRule>> = HashMap::new();
    let mut detected_keyframes: HashSet<String> = HashSet::new();

    for rule in input.rules {
        // 收集所需 keyframes 动画
        if rule.css_property == "animation" {
            check_and_collect_keyframes(&rule.value, &mut detected_keyframes);
        }

        if rule.modifiers.is_empty() {
            root_raw_rules.push(rule);
        } else {
            modifier_groups
                .entry(rule.modifiers.clone())
                .or_default()
                .push(rule);
        }
    }

    let mut root_rules = Vec::new();

    // 1. 根声明消解并转换为 CssRule
    let deduped_root = deduplicate_utility_rules(root_raw_rules);
    for rule in deduped_root {
        root_rules.push(convert_rule_to_declaration(&rule));
    }

    // 2. 转换分组修饰符（带消解）
    for (modifiers, rules) in modifier_groups {
        let deduped_rules = deduplicate_utility_rules(rules);
        let nested_css_rule = build_modifier_rule(modifiers, deduped_rules)?;
        root_rules.push(nested_css_rule);
    }

    // 3. 自动注入 Keyframes 规则
    inject_keyframes_rules(&mut root_rules, &detected_keyframes);

    Ok(CssBlock { rules: root_rules })
}

/// 编译期 Tailwind Merge: 相同修饰符组下的实用类属性消解 (Last-wins 覆盖先出者)
fn deduplicate_utility_rules(rules: Vec<UtilityRule>) -> Vec<UtilityRule> {
    let mut seen_properties = HashSet::new();
    let mut deduped_rev = Vec::new();

    for rule in rules.into_iter().rev() {
        if seen_properties.insert(rule.css_property.clone()) {
            deduped_rev.push(rule);
        }
    }
    deduped_rev.into_iter().rev().collect()
}

fn check_and_collect_keyframes(value: &UtilityValue, keyframes: &mut HashSet<String>) {
    let anim_str = match value {
        UtilityValue::Keyword(kw) => *kw,
        UtilityValue::ArbitraryLiteral(s) => s.as_str(),
        _ => return,
    };

    for name in &["spin", "ping", "pulse", "bounce"] {
        if anim_str.starts_with(name) || anim_str.contains(name) {
            keyframes.insert((*name).to_string());
        }
    }
}

fn inject_keyframes_rules(root_rules: &mut Vec<CssRule>, keyframes: &HashSet<String>) {
    for name in keyframes {
        if let Some(at_rule) = build_keyframe_at_rule(name) {
            root_rules.push(CssRule::AtRule(at_rule));
        }
    }
}

fn build_keyframe_at_rule(name: &str) -> Option<CssAtRule> {
    let at_name = Ident::new("keyframes", Span::call_site());
    let params: TokenStream = name.parse().ok()?;

    let mut keyframe_rules = Vec::new();

    match name {
        "spin" => {
            keyframe_rules.push(make_nested_rule(
                "from",
                vec![("transform", quote!(rotate(0deg)))],
            ));
            keyframe_rules.push(make_nested_rule(
                "to",
                vec![("transform", quote!(rotate(360deg)))],
            ));
        }
        "ping" => {
            keyframe_rules.push(make_nested_rule(
                "75%, 100%",
                vec![("transform", quote!(scale(2))), ("opacity", quote!(0))],
            ));
        }
        "pulse" => {
            keyframe_rules.push(make_nested_rule("50%", vec![("opacity", quote!(0.5))]));
        }
        "bounce" => {
            keyframe_rules.push(make_nested_rule(
                "0%, 100%",
                vec![
                    ("transform", quote!(translateY(-25%))),
                    (
                        "animation-timing-function",
                        quote!(cubic - bezier(0.8, 0, 1, 1)),
                    ),
                ],
            ));
            keyframe_rules.push(make_nested_rule(
                "50%",
                vec![
                    ("transform", quote!(none)),
                    (
                        "animation-timing-function",
                        quote!(cubic - bezier(0, 0, 0.2, 1)),
                    ),
                ],
            ));
        }
        _ => return None,
    }

    Some(CssAtRule {
        name: at_name,
        params,
        block: CssBlock {
            rules: keyframe_rules,
        },
    })
}

fn make_nested_rule(selector: &str, declarations: Vec<(&str, TokenStream)>) -> CssRule {
    let selectors: TokenStream = selector.parse().unwrap();
    let mut decl_rules = Vec::new();

    for (prop, vals) in declarations {
        decl_rules.push(CssRule::Declaration(CssDeclaration {
            property: prop.to_string(),
            values: vals,
            semi_token: Some(syn::token::Semi(Span::call_site())),
        }));
    }

    CssRule::Nested(CssNested {
        selectors,
        block: CssBlock { rules: decl_rules },
    })
}

fn convert_rule_to_declaration(rule: &UtilityRule) -> CssRule {
    let prop = rule.css_property.clone();
    let values = match &rule.value {
        UtilityValue::Keyword(kw) => {
            let ts: TokenStream = kw.parse().unwrap_or_else(|_| quote!(#kw));
            ts
        }
        UtilityValue::Numeric(val, unit) => {
            if unit.is_empty() {
                let lit = proc_macro2::Literal::f64_unsuffixed(*val);
                quote!(#lit)
            } else {
                let val_str = format!("{}{}", val, unit);
                let lit = proc_macro2::Literal::string(&val_str);
                quote!(#lit)
            }
        }
        UtilityValue::HexColor(hex) => {
            let lit = proc_macro2::Literal::string(hex);
            quote!(#lit)
        }
        UtilityValue::ThemeVar(var) => {
            let var_name = format!("var(--slx-theme-{})", var);
            let lit = proc_macro2::Literal::string(&var_name);
            quote!(#lit)
        }
        UtilityValue::ArbitraryLiteral(lit) => {
            let lit_node = proc_macro2::Literal::string(lit);
            quote!(#lit_node)
        }
        UtilityValue::DynamicExpr(expr, _expr_span) => {
            // 包装为 Silex 动态表达式节点 `$ ( expr )`
            let mut ts = TokenStream::new();
            ts.extend(std::iter::once(TokenTree::Punct(Punct::new(
                '$',
                Spacing::Joint,
            ))));
            ts.extend(std::iter::once(TokenTree::Group(Group::new(
                Delimiter::Parenthesis,
                quote!(#expr),
            ))));
            ts
        }
    };

    CssRule::Declaration(CssDeclaration {
        property: prop,
        values,
        semi_token: Some(syn::token::Semi(rule.span)),
    })
}

fn build_modifier_rule(modifiers: Vec<Modifier>, rules: Vec<UtilityRule>) -> Result<CssRule> {
    let mut inner_declarations = Vec::new();
    for rule in rules {
        inner_declarations.push(convert_rule_to_declaration(&rule));
    }
    let inner_block = CssBlock {
        rules: inner_declarations,
    };

    // 从右往左递归组装修饰符块
    let mut current_block = inner_block;
    let mut is_at_rule = false;
    let at_rule_name = Ident::new("media", Span::call_site());
    let mut at_rule_params = TokenStream::new();

    for modifier in modifiers.into_iter().rev() {
        match modifier {
            Modifier::PseudoClass(pc) => {
                let sel_str = format!("&:{}", pc);
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::PseudoElement(pe) => {
                let sel_str = format!("&::{}", pe);
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Dark => {
                let sel_str = ".dark &";
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Group(state) => {
                let sel_str = format!(".group:{} &", state);
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Peer(state) => {
                let sel_str = format!(".peer:{} ~ &", state);
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::CustomSelector(cs) => {
                let ts: TokenStream = cs.parse().unwrap_or_else(|_| quote!(#cs));
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::MediaBreakpoint(bp) => {
                is_at_rule = true;
                let min_width = match bp.as_str() {
                    "sm" => "640px",
                    "md" => "768px",
                    "lg" => "1024px",
                    "xl" => "1280px",
                    "2xl" => "1536px",
                    _ => "640px",
                };
                let query = format!("(min-width: {})", min_width);
                at_rule_params = query.parse().unwrap();
            }
        }
    }

    if is_at_rule {
        Ok(CssRule::AtRule(CssAtRule {
            name: at_rule_name,
            params: at_rule_params,
            block: current_block,
        }))
    } else {
        // 如果只有一层 Nested 提取出首个 CssRule
        if current_block.rules.len() == 1 {
            Ok(current_block.rules.into_iter().next().unwrap())
        } else {
            let ts: TokenStream = "&".parse().unwrap();
            Ok(CssRule::Nested(CssNested {
                selectors: ts,
                block: current_block,
            }))
        }
    }
}
