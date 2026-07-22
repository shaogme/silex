use crate::css::ast::{CssAtRule, CssBlock, CssDeclaration, CssNested, CssRule};
use crate::css::tw::ast::{Modifier, TwInput, UtilityRule, UtilityValue};
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::HashSet;
use syn::Result;

/// 将解析后的 `Vec<UtilityRule>` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_rules(rules: Vec<UtilityRule>) -> Result<CssBlock> {
    let mut root_raw_rules = Vec::new();
    let mut modifier_groups: Vec<(Vec<Modifier>, Vec<UtilityRule>)> = Vec::new();
    let mut detected_keyframes: HashSet<String> = HashSet::new();

    for rule in rules {
        // 收集所需 keyframes 动画
        if rule.css_property == "animation" {
            check_and_collect_keyframes(&rule.value, &mut detected_keyframes);
        }

        if rule.modifiers.is_empty() {
            root_raw_rules.push(rule);
        } else {
            if let Some((_, group_rules)) = modifier_groups
                .iter_mut()
                .find(|(m, _)| m == &rule.modifiers)
            {
                group_rules.push(rule);
            } else {
                modifier_groups.push((rule.modifiers.clone(), vec![rule]));
            }
        }
    }

    // 按修饰符分类与响应式断点 (min-width) 升序排序，保证样式层叠覆盖顺序符合 CSS Specificity 规范
    modifier_groups.sort_by_key(|(m, _)| modifier_group_sort_key(m));

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

/// 计算单修饰符的分类权重
fn modifier_priority(m: &Modifier) -> u32 {
    match m {
        Modifier::Child | Modifier::Descendant => 10,
        Modifier::PseudoClass(_) | Modifier::PseudoElement(_) => 20,
        Modifier::DataAttribute { .. } | Modifier::AriaAttribute { .. } => 30,
        Modifier::Group { .. } | Modifier::Peer { .. } => 40,
        Modifier::Has(_) => 50,
        Modifier::Dark => 60,
        Modifier::ContainerQuery { .. } => 70,
        Modifier::MediaBreakpoint(bp) => {
            let px = match bp.as_str() {
                "sm" => 640,
                "md" => 768,
                "lg" => 1024,
                "xl" => 1280,
                "2xl" => 1536,
                _ => crate::css::config::get_config()
                    .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()))
                    .and_then(|s| s.strip_suffix("px").and_then(|v| v.parse::<u32>().ok()))
                    .unwrap_or(640),
            };
            1000 + px
        }
        Modifier::CustomSelector(_) => 5000,
    }
}

/// 计算修饰符组的综合排序 Key
fn modifier_group_sort_key(modifiers: &[Modifier]) -> (u32, u32, usize) {
    let max_p = modifiers.iter().map(modifier_priority).max().unwrap_or(0);
    let total_p: u32 = modifiers.iter().map(modifier_priority).sum();
    (max_p, total_p, modifiers.len())
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

        // --- Inset 定位类 ---
        "inset" => Some(&["top", "right", "bottom", "left"]),
        "inset-x" => Some(&["left", "right"]),
        "inset-y" => Some(&["top", "bottom"]),

        // --- Border 边框宽/样/色全覆盖简写 ---
        "border" => Some(&[
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ]),

        // --- Border 边框宽度类 ---
        "border-width" => Some(&[
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ]),
        "border-x-width" | "border-x" => Some(&["border-left-width", "border-right-width"]),
        "border-y-width" | "border-y" => Some(&["border-top-width", "border-bottom-width"]),

        // --- Border 边框样式类 ---
        "border-style" => Some(&[
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        ]),
        "border-x-style" => Some(&["border-left-style", "border-right-style"]),
        "border-y-style" => Some(&["border-top-style", "border-bottom-style"]),

        // --- Border 边框颜色类 ---
        "border-color" => Some(&[
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        ]),
        "border-x-color" => Some(&["border-left-color", "border-right-color"]),
        "border-y-color" => Some(&["border-top-color", "border-bottom-color"]),

        // --- Border Radius 圆角类 ---
        "border-radius" => Some(&[
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
        ]),

        // --- Overflow 溢出类 ---
        "overflow" => Some(&["overflow-x", "overflow-y"]),

        // --- Gap 间距类 ---
        "gap" => Some(&["row-gap", "column-gap"]),

        // --- Flex 弹性盒子类 ---
        "flex" => Some(&["flex-grow", "flex-shrink", "flex-basis"]),

        // --- Columns 分栏类 ---
        "columns" => Some(&["column-width", "column-count"]),

        // --- Scroll Margin 类 ---
        "scroll-margin" => Some(&[
            "scroll-margin-top",
            "scroll-margin-right",
            "scroll-margin-bottom",
            "scroll-margin-left",
        ]),
        "scroll-margin-inline" => Some(&["scroll-margin-left", "scroll-margin-right"]),
        "scroll-margin-block" => Some(&["scroll-margin-top", "scroll-margin-bottom"]),

        // --- Scroll Padding 类 ---
        "scroll-padding" => Some(&[
            "scroll-padding-top",
            "scroll-padding-right",
            "scroll-padding-bottom",
            "scroll-padding-left",
        ]),
        "scroll-padding-inline" => Some(&["scroll-padding-left", "scroll-padding-right"]),
        "scroll-padding-block" => Some(&["scroll-padding-top", "scroll-padding-bottom"]),

        _ => None,
    }
}

/// 编译期 Tailwind Merge: 相同修饰符组下的实用类属性消解 (支持简写属性与长写属性关联覆盖，Last-wins 覆盖先出者)
pub(crate) fn deduplicate_utility_rules(rules: Vec<UtilityRule>) -> Vec<UtilityRule> {
    let mut covered_subproperties = HashSet::new();
    let mut deduped_rev = Vec::new();
    let mut transform_rules: Vec<UtilityRule> = Vec::new();

    for rule in rules.into_iter().rev() {
        let prop = rule.css_property.as_str();

        if prop == "transform" {
            transform_rules.push(rule);
            continue;
        }

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

    // 组合合并 transform 规则 (如 translateX(-50%) translateY(-50%))
    if !transform_rules.is_empty() {
        transform_rules.reverse(); // 恢复原始顺序
        let first = transform_rules[0].clone();
        let mut combined_vals = Vec::new();
        for r in &transform_rules {
            let val_str = utility_value_to_css_string(&r.value);
            if !val_str.is_empty() {
                combined_vals.push(val_str);
            }
        }
        let merged_rule = UtilityRule {
            modifiers: first.modifiers,
            css_property: "transform".to_string(),
            value: UtilityValue::ArbitraryLiteral(combined_vals.join(" ")),
            span: first.span,
        };
        // 插入倒序 vector 的头部，翻转后保留在规则列表后方，维持正确的层叠覆盖顺序
        deduped_rev.insert(0, merged_rule);
    }

    deduped_rev.into_iter().rev().collect()
}

fn utility_value_to_css_string(val: &UtilityValue) -> String {
    match val {
        UtilityValue::Keyword(k) => k.to_string(),
        UtilityValue::Numeric(v, unit) => {
            if unit.is_empty() {
                v.to_string()
            } else {
                format!("{}{}", v, unit)
            }
        }
        UtilityValue::HexColor(hex) => hex.clone(),
        UtilityValue::ThemeVar(var, opacity) => match opacity {
            Some(op) => format!(
                "color-mix(in srgb, var(--slx-theme-{}) {}%, transparent)",
                var, op
            ),
            None => format!("var(--slx-theme-{})", var),
        },
        UtilityValue::ArbitraryLiteral(s) => s.clone(),
        UtilityValue::DynamicExpr(expr, _) => quote::quote!(#expr).to_string(),
    }
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
            parse_css_literal_to_tokens(lit)
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
            Modifier::Group { state, name } => {
                let sel_str = format_group_peer_selector(true, &state, name.as_deref());
                let lit = proc_macro2::Literal::string(&sel_str);
                let ts = quote::quote!(#lit);
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Peer { state, name } => {
                let sel_str = format_group_peer_selector(false, &state, name.as_deref());
                let lit = proc_macro2::Literal::string(&sel_str);
                let ts = quote::quote!(#lit);
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Child => {
                let sel_str = "& > *";
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Descendant => {
                let sel_str = "& *";
                let ts: TokenStream = sel_str.parse().unwrap();
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::DataAttribute { key, value } => {
                let sel_str = match value {
                    Some(v) => format!("&[data-{}=\"{}\"]", key, v),
                    None => format!("&[data-{}]", key),
                };
                let lit = proc_macro2::Literal::string(&sel_str);
                let ts = quote::quote!(#lit);
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::AriaAttribute { key, value } => {
                let sel_str = match value {
                    Some(v) => format!("&[aria-{}=\"{}\"]", key, v),
                    None => format!("&[aria-{}]", key),
                };
                let lit = proc_macro2::Literal::string(&sel_str);
                let ts = quote::quote!(#lit);
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Has(target) => {
                let has_target = if let Some(inner) = target
                    .strip_prefix("has-data-[")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    match inner.split_once('=') {
                        Some((k, v)) => format!("[data-{}=\"{}\"]", k, v),
                        None => format!("[data-{}]", inner),
                    }
                } else if let Some(inner) = target
                    .strip_prefix("has-[")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    inner.to_string()
                } else {
                    target.clone()
                };
                let sel_str = format!("&:has({})", has_target);
                let ts: TokenStream = sel_str.parse().unwrap_or_else(|_| {
                    let lit = proc_macro2::Literal::string(&sel_str);
                    quote::quote!(#lit)
                });
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

                let min_width = custom_bp.unwrap_or(match bp.as_str() {
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
                let query_str = match name {
                    Some(n) => format!("{} (min-width: {})", n, min_width),
                    None => format!("(min-width: {})", min_width),
                };
                let lit = proc_macro2::Literal::string(&query_str);
                let at_rule_params: TokenStream = quote::quote!(#lit);
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

fn format_group_peer_selector(is_group: bool, state: &str, name: Option<&str>) -> String {
    let base = match name {
        Some(n) => {
            if is_group {
                format!(".group\\/{}", n)
            } else {
                format!(".peer\\/{}", n)
            }
        }
        None => {
            if is_group {
                ".group".to_string()
            } else {
                ".peer".to_string()
            }
        }
    };
    let connector = if is_group { "&" } else { "~ &" };

    if let Some(inner) = state
        .strip_prefix("data-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        let attr = match inner.split_once('=') {
            Some((k, v)) => format!("[data-{}=\"{}\"]", k, v),
            None => format!("[data-{}]", inner),
        };
        format!("{}{}{}", base, attr, connector)
    } else if let Some(inner) = state
        .strip_prefix("has-data-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        let attr = match inner.split_once('=') {
            Some((k, v)) => format!("[data-{}=\"{}\"]", k, v),
            None => format!("[data-{}]", inner),
        };
        format!("{}:has({}){}", base, attr, connector)
    } else if let Some(inner) = state
        .strip_prefix("has-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        format!("{}:has({}){}", base, inner, connector)
    } else if state.starts_with('[') && state.ends_with(']') {
        let inner = &state[1..state.len() - 1];
        if let Some(rest) = inner.strip_prefix("&:") {
            format!("{}:{} {}", base, rest, connector)
        } else if let Some(rest) = inner.strip_prefix('&') {
            format!("{}{}{}", base, rest, connector)
        } else {
            format!("{}{} {}", base, inner, connector)
        }
    } else {
        format!("{}:{} {}", base, state, connector)
    }
}

fn parse_css_literal_to_tokens(lit: &str) -> TokenStream {
    if let Ok(ts) = lit.parse::<TokenStream>() {
        return ts;
    }
    let proc_lit = proc_macro2::Literal::string(lit);
    quote::quote!(#proc_lit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn test_transform_rules_merging() {
        let rules = vec![
            UtilityRule {
                modifiers: vec![],
                css_property: "transform".to_string(),
                value: UtilityValue::ArbitraryLiteral("translateX(-50%)".to_string()),
                span: Span::call_site(),
            },
            UtilityRule {
                modifiers: vec![],
                css_property: "transform".to_string(),
                value: UtilityValue::ArbitraryLiteral("translateY(-50%)".to_string()),
                span: Span::call_site(),
            },
        ];

        let deduped = deduplicate_utility_rules(rules);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].css_property, "transform");
        if let UtilityValue::ArbitraryLiteral(ref s) = deduped[0].value {
            assert_eq!(s, "translateX(-50%) translateY(-50%)");
        } else {
            panic!("Expected ArbitraryLiteral");
        }
    }

    #[test]
    fn test_parse_css_literal_to_tokens_hex_and_functions() {
        let hex_tokens = parse_css_literal_to_tokens("#ffffff");
        assert_eq!(hex_tokens.to_string(), "# ffffff");

        let fn_tokens = parse_css_literal_to_tokens("translateX(-50%)");
        assert_eq!(fn_tokens.to_string(), "translateX (- 50 %)");
    }

    #[test]
    fn test_responsive_breakpoint_sorting() {
        let lg_rule = UtilityRule {
            modifiers: vec![Modifier::MediaBreakpoint("lg".to_string())],
            css_property: "padding".to_string(),
            value: UtilityValue::Numeric(2.0, "rem"),
            span: Span::call_site(),
        };
        let sm_rule = UtilityRule {
            modifiers: vec![Modifier::MediaBreakpoint("sm".to_string())],
            css_property: "padding".to_string(),
            value: UtilityValue::Numeric(0.5, "rem"),
            span: Span::call_site(),
        };

        // 假定输入顺序为 lg 在前, sm 在后
        let block = build_css_block_from_rules(vec![lg_rule, sm_rule]).unwrap();
        assert_eq!(block.rules.len(), 2);

        // 验证转换后的 AtRule 中，sm (640px) 应该排在 lg (1024px) 之前
        if let CssRule::AtRule(ref at1) = block.rules[0] {
            assert!(at1.params.to_string().contains("640px"));
        } else {
            panic!("Expected AtRule for sm breakpoint first");
        }

        if let CssRule::AtRule(ref at2) = block.rules[1] {
            assert!(at2.params.to_string().contains("1024px"));
        } else {
            panic!("Expected AtRule for lg breakpoint second");
        }
    }

    #[test]
    fn test_extended_atomic_deduplication() {
        let inset_x = UtilityRule {
            modifiers: vec![],
            css_property: "inset-x".to_string(),
            value: UtilityValue::Numeric(0.0, "px"),
            span: Span::call_site(),
        };
        let left_override = UtilityRule {
            modifiers: vec![],
            css_property: "left".to_string(),
            value: UtilityValue::Numeric(1.0, "rem"),
            span: Span::call_site(),
        };

        let deduped = deduplicate_utility_rules(vec![inset_x, left_override]);
        // inset-x 生成了 left 和 right 的覆盖，后续的 left 将正确覆盖 inset-x 中的 left
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].css_property, "inset-x");
        assert_eq!(deduped[1].css_property, "left");
    }
}
