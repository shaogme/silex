use crate::css::{
    ast::{CssAtRule, CssBlock, CssDeclaration, CssNested, CssRule},
    config::get_config,
    tw::{
        ast::{Modifier, ModifierList, SpannedModifier, TwInput, TwSegment, UtilityRule, UtilityValue},
        resolver::codegen::{
            keyframes::lookup_keyframe_meta, modifiers::lookup_modifier_meta,
            property_id::CssPropertyId,
        },
    },
};
use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use syn::{Result, token::Semi};

/// 将解析后的 `Vec<UtilityRule>` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_rules(rules: Vec<UtilityRule>) -> Result<CssBlock> {
    let mut root_raw_rules = Vec::new();
    let mut modifier_groups: Vec<(ModifierList, Vec<UtilityRule>)> = Vec::new();
    let mut detected_keyframes: HashSet<String> = HashSet::new();

    for rule in rules {
        // 收集所需 keyframes 动画
        if rule.css_property == CssPropertyId::Animation {
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
            TwSegment::Static(r) => rules.extend(r),
            TwSegment::Conditional {
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
fn modifier_priority(m: &SpannedModifier) -> u32 {
    match &m.modifier {
        Modifier::Child | Modifier::Descendant => 10,
        Modifier::PseudoClass(_) | Modifier::PseudoElement(_) => 20,
        Modifier::DataAttribute { .. } | Modifier::AriaAttribute { .. } => 30,
        Modifier::Group { .. } | Modifier::Peer { .. } => 40,
        Modifier::Has(_) => 50,
        Modifier::Dark => 60,
        Modifier::ContainerQuery { .. } => 70,
        Modifier::MediaBreakpoint(bp) => {
            if let Some(meta) = lookup_modifier_meta(bp.as_str()) {
                meta.priority
            } else {
                let px = get_config()
                    .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()))
                    .and_then(|s| s.strip_suffix("px").and_then(|v| v.parse::<u32>().ok()))
                    .unwrap_or(640);
                1000 + px
            }
        }
        Modifier::CustomSelector(_) => 5000,
    }
}

/// 计算修饰符组的综合排序 Key
fn modifier_group_sort_key(modifiers: &[SpannedModifier]) -> (u32, u32, usize) {
    let max_p = modifiers.iter().map(modifier_priority).max().unwrap_or(0);
    let total_p: u32 = modifiers.iter().map(modifier_priority).sum();
    (max_p, total_p, modifiers.len())
}

/// 编译期 Tailwind Merge: 相同修饰符组下的实用类属性消解 (基于 Bitmask 的高速覆盖计算，支持简写属性与长写属性关联覆盖，Last-wins 覆盖先出者)
pub(crate) fn deduplicate_utility_rules(rules: Vec<UtilityRule>) -> Vec<UtilityRule> {
    let mut covered_masks: HashMap<(ModifierList, u16), u64> = HashMap::new();
    let mut custom_covered: HashSet<(ModifierList, Cow<'static, str>)> = HashSet::new();
    let mut deduped_rev = Vec::new();
    let mut transform_rules_by_modifier: HashMap<ModifierList, Vec<UtilityRule>> = HashMap::new();

    for rule in rules.into_iter().rev() {
        let prop = rule.css_property.clone();

        if prop == CssPropertyId::Transform {
            transform_rules_by_modifier
                .entry(rule.modifiers.clone())
                .or_default()
                .push(rule);
            continue;
        }

        if let CssPropertyId::Custom(ref name) = prop {
            let key = (rule.modifiers.clone(), name.clone());
            if custom_covered.insert(key) {
                deduped_rev.push(rule);
            }
            continue;
        }

        let bitmask = prop.bitmask();
        let key = (rule.modifiers.clone(), bitmask.group_id);
        let current_covered = covered_masks.entry(key).or_insert(0);

        let all_covered = (*current_covered & bitmask.mask) == bitmask.mask;

        if !all_covered {
            *current_covered |= bitmask.mask;
            deduped_rev.push(rule);
        }
    }

    // 按相同的修饰符组合并 transform 规则 (如 translateX(-50%) translateY(-50%))
    for (modifiers, mut t_rules) in transform_rules_by_modifier {
        if t_rules.is_empty() {
            continue;
        }
        t_rules.reverse(); // 恢复原始顺序
        let first = t_rules[0].clone();
        let mut combined_vals = Vec::new();
        for r in &t_rules {
            let val_str = utility_value_to_css_string(&r.value);
            if !val_str.is_empty() {
                combined_vals.push(val_str);
            }
        }
        let merged_rule = UtilityRule {
            modifiers,
            css_property: CssPropertyId::Transform,
            value: UtilityValue::ArbitraryLiteral(combined_vals.join(" ")),
            span: first.span,
        };
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
    let meta = lookup_keyframe_meta(name)?;

    let at_name = Ident::new("keyframes", Span::call_site());
    let params: TokenStream = name.parse().ok()?;

    let mut keyframe_rules = Vec::new();
    for step in meta.steps {
        let decls = step
            .declarations
            .iter()
            .map(|&(prop, val)| {
                let ts: TokenStream = val.parse().unwrap_or_else(|_| quote!(#val));
                (prop, ts)
            })
            .collect();
        keyframe_rules.push(make_nested_rule(step.selector, decls));
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
            semi_token: Some(Semi(Span::call_site())),
        }));
    }

    CssRule::Nested(CssNested {
        selectors,
        block: CssBlock { rules: decl_rules },
    })
}

fn convert_rule_to_declaration(rule: &UtilityRule) -> CssRule {
    let prop = rule.css_property.as_str().to_string();
    let values = match &rule.value {
        UtilityValue::Keyword(kw) => {
            let ts: TokenStream = kw.parse().unwrap_or_else(|_| quote!(#kw));
            ts
        }
        UtilityValue::Numeric(val, unit) => {
            if unit.is_empty() {
                let lit = Literal::f64_unsuffixed(*val);
                quote!(#lit)
            } else {
                let val_str = format!("{}{}", val, unit);
                let lit = Literal::string(&val_str);
                quote!(#lit)
            }
        }
        UtilityValue::HexColor(hex) => {
            let lit = Literal::string(hex);
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
            let lit = Literal::string(&val_str);
            quote!(#lit)
        }
        UtilityValue::ArbitraryLiteral(lit) => {
            let lit_node = Literal::string(lit);
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
        semi_token: Some(Semi(rule.span)),
    })
}

fn build_modifier_rule(
    modifiers: ModifierList,
    rules: Vec<UtilityRule>,
) -> Result<CssRule> {
    let mut inner_declarations = Vec::new();
    for rule in rules {
        inner_declarations.push(convert_rule_to_declaration(&rule));
    }
    let inner_block = CssBlock {
        rules: inner_declarations,
    };

    // 从右往左递归组装修饰符块
    let mut current_block = inner_block;

    for spanned in modifiers.into_iter().rev() {
        let mod_span = spanned.span();
        match spanned.modifier {
            Modifier::PseudoClass(pc) => {
                let pseudo = match pc.as_str() {
                    "first" => "first-child",
                    "last" => "last-child",
                    "only" => "only-child",
                    "odd" => "nth-child(odd)",
                    "even" => "nth-child(even)",
                    "first-of-type" => "first-of-type",
                    "last-of-type" => "last-of-type",
                    "only-of-type" => "only-of-type",
                    other => other,
                };
                let sel_str = format!("&:{}", pseudo);
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
                let dark_mode = get_config()
                    .and_then(|cfg| cfg.theme.dark_mode.as_deref())
                    .unwrap_or("class");

                if dark_mode == "media" {
                    let query = "(prefers-color-scheme: dark)";
                    let at_rule_params: TokenStream = query.parse().unwrap();
                    let at_rule_name = Ident::new("media", mod_span);

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
                let lit = Literal::string(&sel_str);
                let ts = quote!(#lit);
                current_block = CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                };
            }
            Modifier::Peer { state, name } => {
                let sel_str = format_group_peer_selector(false, &state, name.as_deref());
                let lit = Literal::string(&sel_str);
                let ts = quote!(#lit);
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
                let lit = Literal::string(&sel_str);
                let ts = quote!(#lit);
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
                let lit = Literal::string(&sel_str);
                let ts = quote!(#lit);
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
                    let lit = Literal::string(&sel_str);
                    quote!(#lit)
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
                let query = if let Some(meta) = lookup_modifier_meta(bp.as_str()) {
                    meta.css_selector.to_string()
                } else {
                    let custom_bp = get_config()
                        .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()))
                        .map(|s| s.as_str());

                    let min_width = custom_bp.unwrap_or("640px");
                    format!("(min-width: {})", min_width)
                };
                let at_rule_params: TokenStream = query.parse().unwrap();
                let at_rule_name = Ident::new("media", mod_span);

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
                let lit = Literal::string(&query_str);
                let at_rule_params: TokenStream = quote!(#lit);
                let at_rule_name = Ident::new("container", mod_span);

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
    let prefix = if is_group { ".group" } else { ".peer" };
    let base = match name {
        Some(n) => format!("{}\\/{}", prefix, n),
        None => prefix.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use smallvec::smallvec;

    #[test]
    fn test_transform_rules_merging() {
        let rules = vec![
            UtilityRule {
                modifiers: smallvec![],
                css_property: CssPropertyId::Transform,
                value: UtilityValue::ArbitraryLiteral("translateX(-50%)".to_string()),
                span: Span::call_site(),
            },
            UtilityRule {
                modifiers: smallvec![],
                css_property: CssPropertyId::Transform,
                value: UtilityValue::ArbitraryLiteral("translateY(-50%)".to_string()),
                span: Span::call_site(),
            },
        ];

        let deduped = deduplicate_utility_rules(rules);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].css_property, CssPropertyId::Transform);
        if let UtilityValue::ArbitraryLiteral(ref s) = deduped[0].value {
            assert_eq!(s, "translateX(-50%) translateY(-50%)");
        } else {
            panic!("Expected ArbitraryLiteral");
        }
    }

    #[test]
    fn test_responsive_breakpoint_sorting() {
        let lg_rule = UtilityRule {
            modifiers: smallvec![Modifier::MediaBreakpoint("lg".to_string()).into()],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(2.0, "rem"),
            span: Span::call_site(),
        };
        let sm_rule = UtilityRule {
            modifiers: smallvec![Modifier::MediaBreakpoint("sm".to_string()).into()],
            css_property: CssPropertyId::Padding,
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
            modifiers: smallvec![],
            css_property: CssPropertyId::InsetX,
            value: UtilityValue::Numeric(0.0, "px"),
            span: Span::call_site(),
        };
        let left_override = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Left,
            value: UtilityValue::Numeric(1.0, "rem"),
            span: Span::call_site(),
        };

        let deduped = deduplicate_utility_rules(vec![inset_x, left_override]);
        // inset-x 生成了 left 和 right 的覆盖，后续的 left 将正确覆盖 inset-x 中的 left
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].css_property, CssPropertyId::InsetX);
        assert_eq!(deduped[1].css_property, CssPropertyId::Left);
    }

    #[test]
    fn test_transform_rules_merging_respects_modifiers() {
        let base_transform = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Transform,
            value: UtilityValue::ArbitraryLiteral("translate-x-0".to_string()),
            span: Span::call_site(),
        };
        let dark_transform = UtilityRule {
            modifiers: smallvec![Modifier::Dark.into()],
            css_property: CssPropertyId::Transform,
            value: UtilityValue::ArbitraryLiteral("translate-x-full".to_string()),
            span: Span::call_site(),
        };

        let deduped =
            deduplicate_utility_rules(vec![base_transform.clone(), dark_transform.clone()]);
        assert_eq!(
            deduped.len(),
            2,
            "Base transform and dark transform must NOT be merged together into one rule"
        );

        let has_base = deduped
            .iter()
            .any(|r| r.modifiers.is_empty() && r.css_property == CssPropertyId::Transform);
        let has_dark = deduped
            .iter()
            .any(|r| r.modifiers.len() == 1 && r.modifiers[0] == Modifier::Dark && r.css_property == CssPropertyId::Transform);
        assert!(has_base && has_dark);
    }

    #[test]
    fn test_deduplicate_respects_modifiers_for_general_properties() {
        let base_padding = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(1.0, "rem"),
            span: Span::call_site(),
        };
        let hover_padding = UtilityRule {
            modifiers: smallvec![Modifier::PseudoClass("hover".to_string()).into()],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(2.0, "rem"),
            span: Span::call_site(),
        };
        let override_padding = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(1.5, "rem"),
            span: Span::call_site(),
        };

        // 输入顺序：p-4 (base), hover:p-8 (hover), p-6 (override)
        let deduped = deduplicate_utility_rules(vec![
            base_padding,
            hover_padding.clone(),
            override_padding.clone(),
        ]);

        // 应该保留 2 条：hover:p-8 和最后覆盖的 p-6 (override)，而最早的 base p-4 被 override 覆盖
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].modifiers, hover_padding.modifiers);
        assert_eq!(deduped[1].modifiers, override_padding.modifiers);
    }
}
