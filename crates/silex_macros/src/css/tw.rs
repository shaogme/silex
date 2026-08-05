pub mod ast;
pub mod codegen;
pub mod functional;
pub mod merge;
pub mod parser;
pub mod resolver;
pub mod variants;
pub mod verbose;

pub use variants::tw_variants_impl;

use ast::{TwInput, TwSegment, UtilityRule};
use codegen::{build_css_block_from_rules, build_css_block_from_tw};
use merge::{WriteSet, cluster};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, Expr, Lit, Result};

/// 一个簇里最多允许的条件分支数。
///
/// 每多一个条件分支，需要预编译的组合数就翻一倍。6 个 ⇒ 64 个类，
/// 已经远超真实写法（实测仓库里最大的簇是 1）。超过就报错而不是悄悄退回
/// 不确定的老行为——那正是这项修复要消灭的东西。
const MAX_CONDITIONS_PER_CLUSTER: u32 = 6;

/// `tw!` 过程宏核心实现
pub fn tw_impl(ts: TokenStream) -> Result<TokenStream> {
    tw_impl_internal(ts, false)
}

/// `tw_verbose!` 过程宏核心实现 (带编译期 CSS 诊断打印)
pub fn tw_verbose_impl(ts: TokenStream) -> Result<TokenStream> {
    tw_impl_internal(ts, true)
}

fn tw_impl_internal(ts: TokenStream, verbose: bool) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let input_str = if verbose {
        ts.to_string()
    } else {
        String::new()
    };
    let mut input: TwInput = syn::parse2(ts)?;
    fold_static_conditions(&mut input);
    let extra_classes = input.extra_classes.clone();
    let span = proc_macro2::Span::call_site();

    let has_conditionals = input
        .segments
        .iter()
        .any(|s| matches!(s, TwSegment::Conditional { .. }));

    if !has_conditionals {
        let css_block = build_css_block_from_tw(input)?;
        let mut compile_result =
            crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)?;

        if !extra_classes.is_empty() {
            let extra_str = extra_classes.join(" ");
            compile_result.class_name = format!("{} {}", compile_result.class_name, extra_str);
        }

        if verbose {
            let block_ts = quote! { #css_block };
            verbose::emit(
                &input_str,
                &[
                    ("Generated CssBlock AST", block_ts.to_string()),
                    ("Compiled Class Name", compile_result.class_name.clone()),
                    ("Static CSS", compile_result.static_css.clone()),
                    ("Component CSS", compile_result.component_css.clone()),
                ],
            );
        }

        return crate::css::generate_css_output(compile_result, span);
    }

    // 处理包含条件分支句段的情形
    let mut inits_tokens = Vec::new();
    let mut cx_items = Vec::new();
    let mut condition_exprs = Vec::<Expr>::new();
    let mut compiled_cache = ::std::collections::HashMap::<u128, String>::new();
    // 条件分支路径此前完全不产出 `tw_verbose!` 诊断——而这条路径恰恰是最需要看
    // "到底编出了哪几个类"的地方（一个簇会展开成 2^k 个组合）
    let mut verbose_sections: Vec<(String, String)> = Vec::new();

    let mut compile_rules_cached = |rules: Vec<UtilityRule>| -> Result<String> {
        if rules.is_empty() {
            return Ok(String::new());
        }
        use silex_hash::css::CssHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher1 = CssHasher::with_seed(0x9e3779b97f4a7c15);
        let mut hasher2 = CssHasher::with_seed(0xbf58476d1ce4e5b9);
        rules.hash(&mut hasher1);
        rules.hash(&mut hasher2);
        let key = ((hasher1.finish() as u128) << 64) | (hasher2.finish() as u128);

        if let Some(cls) = compiled_cache.get(&key) {
            return Ok(cls.clone());
        }
        let css_block = build_css_block_from_rules(rules)?;
        let compile_result =
            crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)?;
        if !compile_result.expressions.is_empty() || !compile_result.dynamic_rules.is_empty() {
            return Err(Error::new(
                span,
                "条件 `tw!` 的 then/else utility 必须是静态 CSS；动态 arbitrary value 请移到条件外的 `css!`/`tw!` 路径。",
            ));
        }
        let cls_name = compile_result.class_name.clone();
        if verbose {
            verbose_sections.push((
                format!("Compiled Class `{cls_name}`"),
                format!(
                    "{}\n{}",
                    compile_result.component_css, compile_result.static_css
                ),
            ));
        }
        inits_tokens.push(compile_result.generate_inits());
        compiled_cache.insert(key, cls_name.clone());
        Ok(cls_name)
    };

    // 报告 §3.5：段与段之间没有编译期 tw-merge，谁覆盖谁只能由注入顺序（= 首次渲染
    // 顺序）决定。把**会互相覆盖**的段并成一簇，簇内按条件分支展开成若干"完整合并后"
    // 的类，冲突就交回给编译期裁决；互不覆盖的段照旧各自成类，类的数量不变。

    // 从静态段中提升纯过渡/动画控制规则（transition-property, duration, timing, delay），
    // 避免其因为属性碰撞（如 bg-background 与 bg-primary 碰撞）被错误打包进互斥的 conditional 组合类中，
    // 确保静态 transition 属性永远单独常驻，从底层保障 DOM 节点类名切换时的 CSS 动画连续性。
    let mut hoisted_transition_rules = Vec::new();
    let segments: Vec<TwSegment> = input
        .segments
        .into_iter()
        .map(|seg| match seg {
            TwSegment::Static(rules) => {
                let (trans_rules, other_rules): (Vec<_>, Vec<_>) =
                    rules.into_iter().partition(is_pure_transition_control_rule);
                hoisted_transition_rules.extend(trans_rules);
                TwSegment::Static(other_rules)
            }
            other => other,
        })
        .collect();

    if !hoisted_transition_rules.is_empty() {
        let trans_cls = compile_rules_cached(hoisted_transition_rules)?;
        if !trans_cls.is_empty() {
            cx_items.push(quote! { #trans_cls });
        }
    }

    let write_sets: Vec<WriteSet> = segments.iter().map(segment_write_set).collect();

    for group in cluster(&write_sets) {
        let conds: Vec<(proc_macro2::Ident, &Expr)> = group
            .iter()
            .filter_map(|&i| match &segments[i] {
                TwSegment::Conditional { condition, .. } => {
                    let index = condition_exprs.len();
                    condition_exprs.push(condition.clone());
                    Some((
                        quote::format_ident!("__slx_cond_value_{}", index),
                        condition,
                    ))
                }
                TwSegment::Static(_) => None,
            })
            .collect();

        // 单段的簇：与今天的产出逐字节一致，没必要绕一圈 match
        if group.len() == 1 {
            match &segments[group[0]] {
                TwSegment::Static(rules) => {
                    let cls = compile_rules_cached(rules.clone())?;
                    if !cls.is_empty() {
                        cx_items.push(quote! { #cls });
                    }
                }
                TwSegment::Conditional {
                    then_rules,
                    else_rules,
                    ..
                } => {
                    let then_cls = compile_rules_cached(then_rules.clone())?;
                    let else_cls = compile_rules_cached(else_rules.clone())?;
                    let condition = &conds[0].0;
                    if !else_cls.is_empty() {
                        cx_items.push(quote! { (#condition, #then_cls, #else_cls) });
                    } else {
                        cx_items.push(quote! { (#condition, #then_cls) });
                    }
                }
            }
            continue;
        }

        // 纯静态的簇：合并成一个类即可，没有需要展开的自由度
        if conds.is_empty() {
            let mut merged = Vec::new();
            for &i in &group {
                if let TwSegment::Static(rules) = &segments[i] {
                    merged.extend(rules.iter().cloned());
                }
            }
            let cls = compile_rules_cached(merged)?;
            if !cls.is_empty() {
                cx_items.push(quote! { #cls });
            }
            continue;
        }

        let k = conds.len() as u32;
        if k > MAX_CONDITIONS_PER_CLUSTER {
            return Err(Error::new(
                span,
                format!(
                    "这个 `tw!` 里有 {k} 个条件分支写到了同一组 CSS 属性，\
                     要让它们的覆盖关系在编译期确定下来需要预编译 2^{k} 个类名，\
                     超过了 {MAX_CONDITIONS_PER_CLUSTER} 个条件分支的上限。\
                     请把互不相干的属性拆到多个 `tw!` 调用里。"
                ),
            ));
        }

        // 位 j = 第 j 个条件分支取 then 分支
        let mut arms = Vec::with_capacity(1usize << k);
        for combo in 0..(1u32 << k) {
            let mut merged = Vec::new();
            let mut cond_seen = 0usize;
            for &i in &group {
                match &segments[i] {
                    TwSegment::Static(rules) => merged.extend(rules.iter().cloned()),
                    TwSegment::Conditional {
                        then_rules,
                        else_rules,
                        ..
                    } => {
                        let taken = combo & (1 << cond_seen) != 0;
                        cond_seen += 1;
                        merged.extend(if taken { then_rules } else { else_rules }.iter().cloned());
                    }
                }
            }
            let cls = compile_rules_cached(merged)?;
            let idx = combo as usize;
            arms.push(quote! { #idx => #cls });
        }

        let index_terms = (0..conds.len()).map(|j| {
            let name = &conds[j].0;
            quote! { ((#name as usize) << #j) }
        });

        cx_items.push(quote! {
            {
                match #(#index_terms)|* {
                    #(#arms,)*
                    _ => "",
                }
            }
        });
    }

    if !extra_classes.is_empty() {
        let extra_str = extra_classes.join(" ");
        cx_items.push(quote! { #extra_str });
    }

    if verbose {
        let sections: Vec<verbose::Section<'_>> = verbose_sections
            .iter()
            .map(|(title, body)| (title.as_str(), body.clone()))
            .collect();
        verbose::emit(&input_str, &sections);
    }

    let condition_sources = condition_exprs.iter().map(|condition| {
        quote! {
            #__silex::css::IntoCssReactive::into_css_reactive(#condition)
        }
    });
    let condition_reads = condition_exprs.iter().enumerate().map(|(index, _)| {
        let name = quote::format_ident!("__slx_cond_value_{}", index);
        quote! {
            let #name = __slx_conditions_for_effect[#index].get();
        }
    });

    Ok(quote! {
        {
            let __slx_conditions: ::std::vec::Vec<#__silex::core::Rx<'_, bool>> =
                ::std::vec![ #(#condition_sources),* ];
            let mut __slx_inputs = #__silex::core::RuntimeInputs::new();
            for __slx_condition in &__slx_conditions {
                __slx_inputs.extend(&__slx_condition.runtime_inputs());
            }

            #__silex::dom::attribute::AttrOp::custom_with_inputs(
                __slx_inputs,
                move |element, owner| {
                    #(#inits_tokens)*
                    let mut __slx_effect_inputs = #__silex::core::RuntimeInputs::new();
                    for __slx_condition in &__slx_conditions {
                        __slx_effect_inputs.extend(&__slx_condition.runtime_inputs());
                    }

                    let __slx_conditions_for_effect = __slx_conditions.clone();
                    let __slx_element = element.clone();
                    let __slx_current_class =
                        ::std::rc::Rc::new(::std::cell::RefCell::new(None::<::std::string::String>));
                    let __slx_current_class_for_effect = __slx_current_class.clone();

                    owner.effect_from(
                        __slx_effect_inputs,
                        ::std::boxed::Box::new(move || {
                            #(#condition_reads)*
                            let __slx_next_class = #__silex::css::cx!(
                                #(#cx_items),*
                            );
                            let mut __slx_current_class =
                                __slx_current_class_for_effect.borrow_mut();
                            if __slx_current_class.as_deref()
                                == Some(__slx_next_class.as_str())
                            {
                                return;
                            }

                            let __slx_old_class = __slx_current_class.as_deref().unwrap_or("");
                            for __slx_token in __slx_next_class.split_whitespace() {
                                if !__slx_old_class.split_whitespace().any(|old| old == __slx_token) {
                                    let _ = __slx_element.class_list().add_1(__slx_token);
                                }
                            }
                            for __slx_token in __slx_old_class.split_whitespace() {
                                if !__slx_next_class
                                    .split_whitespace()
                                    .any(|next| next == __slx_token)
                                {
                                    let _ = __slx_element.class_list().remove_1(__slx_token);
                                }
                            }
                            __slx_current_class.replace(__slx_next_class);
                        }),
                    );

                    let __slx_element_for_cleanup = element.clone();
                    owner.on_cleanup(::std::boxed::Box::new(move || {
                        if let Some(__slx_class) = __slx_current_class.borrow_mut().take() {
                            for __slx_token in __slx_class.split_whitespace() {
                                let _ = __slx_element_for_cleanup
                                    .class_list()
                                    .remove_1(__slx_token);
                            }
                        }
                    }));
                },
            )
        }
    })
}

fn fold_static_conditions(input: &mut TwInput) {
    for segment in &mut input.segments {
        let replacement = match segment {
            TwSegment::Conditional {
                condition,
                then_rules,
                else_rules,
            } => static_bool(condition).map(|value| {
                if value {
                    std::mem::take(then_rules)
                } else {
                    std::mem::take(else_rules)
                }
            }),
            TwSegment::Static(_) => None,
        };
        if let Some(rules) = replacement {
            *segment = TwSegment::Static(rules);
        }
    }
}

fn static_bool(expr: &Expr) -> Option<bool> {
    let Expr::Lit(expr) = expr else { return None };
    let Lit::Bool(value) = &expr.lit else {
        return None;
    };
    Some(value.value)
}

/// 一个段可能写到的属性覆盖面：条件分支要把两条分支都算进来
fn segment_write_set(seg: &TwSegment) -> WriteSet {
    match seg {
        TwSegment::Static(rules) => WriteSet::of(rules),
        TwSegment::Conditional {
            then_rules,
            else_rules,
            ..
        } => {
            let mut set = WriteSet::of(then_rules);
            set.merge_from(&WriteSet::of(else_rules));
            set
        }
    }
}

/// 判断是否为纯过渡/动画控制相关的 CSS 属性（不受具体尺寸/颜色状态变动影响）
fn is_pure_transition_control_rule(rule: &UtilityRule) -> bool {
    use crate::css::tw::resolver::codegen::property_id::CssPropertyId;
    matches!(
        rule.css_property,
        CssPropertyId::TransitionProperty
            | CssPropertyId::TransitionDuration
            | CssPropertyId::TransitionTimingFunction
            | CssPropertyId::TransitionDelay
            | CssPropertyId::TransitionBehavior
            | CssPropertyId::Transition
            | CssPropertyId::VarTwDuration
            | CssPropertyId::VarTwEase
            | CssPropertyId::Animation
            | CssPropertyId::AnimationName
            | CssPropertyId::AnimationDuration
            | CssPropertyId::AnimationTimingFunction
            | CssPropertyId::WillChange
    )
}

#[cfg(test)]
mod tests;
