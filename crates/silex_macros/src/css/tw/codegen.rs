use crate::css::ast::{CssAtRule, CssBlock, CssDeclaration, CssNested, CssRule};
use crate::css::tw::ast::{Modifier, TwInput, UtilityRule, UtilityValue};
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::{HashMap, HashSet};
use syn::Result;

/// 将解析后的 `Vec<UtilityRule>` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_rules(rules: Vec<UtilityRule>) -> Result<CssBlock> {
    let mut root_raw_rules = Vec::new();
    let mut modifier_groups: HashMap<Vec<Modifier>, Vec<UtilityRule>> = HashMap::new();
    let mut detected_keyframes: HashSet<String> = HashSet::new();

    for rule in rules {
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

    // 4. 零冗余 DCE 剪裁 Pass (Prune Unused Keyframes)
    prune_unused_keyframes(&mut root_rules, &detected_keyframes);

    Ok(CssBlock { rules: root_rules })
}

/// 将解析后的 `TwInput` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_tw(input: TwInput) -> Result<CssBlock> {
    let mut rules = Vec::new();
    for seg in input.segments {
        match seg {
            crate::css::tw::ast::TwSegment::Static(r) => rules.extend(r),
            crate::css::tw::ast::TwSegment::Conditional {
                then_rules,
                else_rules,
                ..
            } => {
                rules.extend(then_rules);
                rules.extend(else_rules);
            }
        }
    }
    build_css_block_from_rules(rules)
}

/// 获取指定 CSS 属性拆解后的原子子属性（用于简写属性与长写属性关联覆盖消解）
fn get_atomic_subproperties(prop: &str) -> Option<&'static [&'static str]> {
    match prop {
        // --- Padding 边距类 ---
        "padding" => Some(&[
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ]),
        "padding-inline" => Some(&["padding-left", "padding-right"]),
        "padding-block" => Some(&["padding-top", "padding-bottom"]),

        // --- Margin 外边距类 ---
        "margin" => Some(&["margin-top", "margin-right", "margin-bottom", "margin-left"]),
        "margin-inline" => Some(&["margin-left", "margin-right"]),
        "margin-block" => Some(&["margin-top", "margin-bottom"]),

        // --- Border 边框宽度类 ---
        "border-width" => Some(&[
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ]),

        // --- Border 边框样式类 ---
        "border-style" => Some(&[
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        ]),

        // --- Border 边框颜色类 ---
        "border-color" => Some(&[
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ]),

        // --- Border Radius 圆角类 ---
        "border-radius" => Some(&[
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
        ]),

        // --- Inset 定位类 ---
        "inset" => Some(&["top", "right", "bottom", "left"]),

        // --- Overflow 溢出类 ---
        "overflow" => Some(&["overflow-x", "overflow-y"]),

        // --- Gap 间距类 ---
        "gap" => Some(&["row-gap", "column-gap"]),

        // --- Flex 弹性盒子类 ---
        "flex" => Some(&["flex-grow", "flex-shrink", "flex-basis"]),

        _ => None,
    }
}

/// 编译期 Tailwind Merge: 相同修饰符组下的实用类属性消解 (支持简写属性与长写属性关联覆盖，Last-wins 覆盖先出者)
pub(crate) fn deduplicate_utility_rules(rules: Vec<UtilityRule>) -> Vec<UtilityRule> {
    let mut covered_subproperties = HashSet::new();
    let mut deduped_rev = Vec::new();

    for rule in rules.into_iter().rev() {
        let prop = rule.css_property.as_str();
        let subprops: &[&str] = match get_atomic_subproperties(prop) {
            Some(subs) => subs,
            None => std::slice::from_ref(&prop),
        };

        // 检查该规则包含的所有原子子属性在相同的修饰符组下是否已被完全覆盖
        let all_covered = subprops
            .iter()
            .all(|p| covered_subproperties.contains(&(rule.modifiers.clone(), p.to_string())));

        if !all_covered {
            for &p in subprops {
                covered_subproperties.insert((rule.modifiers.clone(), p.to_string()));
            }
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
        UtilityValue::ThemeVar(var, opacity) => {
            let val_str = match opacity {
                Some(op) => format!(
                    "color-mix(in srgb, var(--slx-theme-{}) {}%, transparent)",
                    var, op
                ),
                None => format!("var(--slx-theme-{})", var),
            };
            let lit = proc_macro2::Literal::string(&val_str);
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
                let dark_mode = crate::css::config::get_config()
                    .and_then(|cfg| cfg.theme.dark_mode.as_deref())
                    .unwrap_or("class");

                if dark_mode == "media" {
                    let query = "(prefers-color-scheme: dark)";
                    let at_rule_params: TokenStream = query.parse().unwrap();
                    let at_rule_name = Ident::new("media", Span::call_site());

                    let selector_ts: TokenStream = "&".parse().unwrap();
                    let nested_block = CssBlock {
                        rules: vec![CssRule::Nested(CssNested {
                            selectors: selector_ts,
                            block: current_block,
                        })],
                    };

                    let at_rule = CssAtRule {
                        name: at_rule_name,
                        params: at_rule_params,
                        block: nested_block,
                    };

                    current_block = CssBlock {
                        rules: vec![CssRule::AtRule(at_rule)],
                    };
                } else {
                    let sel_str = ".dark &, &.dark";
                    let ts: TokenStream = sel_str.parse().unwrap();
                    current_block = CssBlock {
                        rules: vec![CssRule::Nested(CssNested {
                            selectors: ts,
                            block: current_block,
                        })],
                    };
                }
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
                let custom_bp = crate::css::config::get_config()
                    .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()))
                    .map(|s| s.as_str());

                let min_width = custom_bp.unwrap_or_else(|| match bp.as_str() {
                    "sm" => "640px",
                    "md" => "768px",
                    "lg" => "1024px",
                    "xl" => "1280px",
                    "2xl" => "1536px",
                    _ => "640px",
                });
                let query = format!("(min-width: {})", min_width);
                let at_rule_params: TokenStream = query.parse().unwrap();
                let at_rule_name = Ident::new("media", Span::call_site());

                let selector_ts: TokenStream = "&".parse().unwrap();
                let nested_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: selector_ts,
                        block: current_block,
                    })],
                };

                let at_rule = CssAtRule {
                    name: at_rule_name,
                    params: at_rule_params,
                    block: nested_block,
                };

                current_block = CssBlock {
                    rules: vec![CssRule::AtRule(at_rule)],
                };
            }
            Modifier::ContainerQuery { name, min_width } => {
                let query = match name {
                    Some(n) => format!("{} (min-width: {})", n, min_width),
                    None => format!("(min-width: {})", min_width),
                };
                let at_rule_params: TokenStream = query.parse().unwrap();
                let at_rule_name = Ident::new("container", Span::call_site());

                let selector_ts: TokenStream = "&".parse().unwrap();
                let nested_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: selector_ts,
                        block: current_block,
                    })],
                };

                let at_rule = CssAtRule {
                    name: at_rule_name,
                    params: at_rule_params,
                    block: nested_block,
                };

                current_block = CssBlock {
                    rules: vec![CssRule::AtRule(at_rule)],
                };
            }
        }
    }

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

/// 零冗余死代码剪裁 (DCE): 递归收集 block 中所有实际引用的 animation 名称，剔除多余的 @keyframes 规则
pub fn prune_unused_keyframes(rules: &mut Vec<CssRule>, detected_keyframes: &HashSet<String>) {
    let mut used = detected_keyframes.clone();
    collect_used_animations(rules, &mut used);

    rules.retain(|rule| {
        if let CssRule::AtRule(at_rule) = rule
            && at_rule.name == "keyframes"
        {
            let name_param = at_rule.params.to_string();
            return used.iter().any(|u| name_param.contains(u));
        }
        true
    });
}

fn collect_used_animations(rules: &[CssRule], used: &mut HashSet<String>) {
    for rule in rules {
        match rule {
            CssRule::Declaration(decl) => {
                if decl.property == "animation" || decl.property == "animation-name" {
                    let val_str = decl.values.to_string();
                    for name in &["spin", "ping", "pulse", "bounce"] {
                        if val_str.contains(name) {
                            used.insert((*name).to_string());
                        }
                    }
                }
            }
            CssRule::Nested(nested) => {
                collect_used_animations(&nested.block.rules, used);
            }
            CssRule::AtRule(at_rule) => {
                if at_rule.name != "keyframes" {
                    collect_used_animations(&at_rule.block.rules, used);
                }
            }
            CssRule::Unsafe(_) => {}
            CssRule::Apply(ap) => {
                for name in &["spin", "ping", "pulse", "bounce"] {
                    if ap.classes.contains(name) {
                        used.insert((*name).to_string());
                    }
                }
            }
        }
    }
}
