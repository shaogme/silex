use crate::css::compiler::{CssCompileResult, CssCompiler};
use crate::css::tw::ast::UtilityRule;
use crate::css::tw::codegen::{build_css_block_from_rules, deduplicate_utility_rules};
use crate::css::tw::parser::parse_modifiers_and_body;
use crate::css::tw::resolver::resolve_utility;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, braced, bracketed};

/// 复合变体判断规则与追加的 Class
#[derive(Debug, Clone)]
pub struct CompoundVariant {
    pub conditions: BTreeMap<String, String>,
    pub rules: Vec<UtilityRule>,
}

/// `tw_variants!` 宏根 AST 节点
#[derive(Debug, Clone)]
pub struct TwVariantsInput {
    pub base_rules: Vec<UtilityRule>,
    /// variant_name -> (option_name -> rules) 保留用户定义时的真实顺序
    pub variants: Vec<(String, BTreeMap<String, Vec<UtilityRule>>)>,
    /// variant_name -> default_option_name
    pub default_variants: BTreeMap<String, String>,
    pub compound_variants: Vec<CompoundVariant>,
}

fn parse_class_rules(lit: &LitStr) -> Result<Vec<UtilityRule>> {
    let raw_str = lit.value();
    let span = lit.span();
    let mut rules = Vec::new();
    for token in raw_str.split_whitespace() {
        let (modifiers, body_token) = parse_modifiers_and_body(token);
        let mut resolved = resolve_utility(modifiers, body_token, span)?;
        rules.append(&mut resolved);
    }
    Ok(rules)
}

fn parse_key_str(input: ParseStream) -> Result<String> {
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        Ok(lit.value())
    } else if input.peek(Ident) {
        let ident: Ident = input.parse()?;
        Ok(ident.to_string())
    } else {
        Err(input.error("Expected identifier or string literal as key"))
    }
}

impl Parse for TwVariantsInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut base_rules = Vec::new();
        let mut variants = Vec::new();
        let mut default_variants = BTreeMap::new();
        let mut compound_variants = Vec::new();

        while !input.is_empty() {
            let key = parse_key_str(input)?;
            input.parse::<Token![:]>()?;

            match key.as_str() {
                "base" => {
                    let lit: LitStr = input.parse()?;
                    base_rules = parse_class_rules(&lit)?;
                }
                "variants" => {
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        let var_name = parse_key_str(&content)?;
                        content.parse::<Token![:]>()?;

                        let opts_content;
                        braced!(opts_content in content);
                        let mut opt_map = BTreeMap::new();

                        while !opts_content.is_empty() {
                            let opt_name = parse_key_str(&opts_content)?;
                            opts_content.parse::<Token![:]>()?;
                            let lit: LitStr = opts_content.parse()?;
                            let rules = parse_class_rules(&lit)?;
                            opt_map.insert(opt_name, rules);

                            if opts_content.peek(Token![,]) {
                                let _: Token![,] = opts_content.parse()?;
                            }
                        }

                        variants.push((var_name, opt_map));

                        if content.peek(Token![,]) {
                            let _: Token![,] = content.parse()?;
                        }
                    }
                }
                "default_variants" => {
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        let var_name = parse_key_str(&content)?;
                        content.parse::<Token![:]>()?;
                        let def_opt = parse_key_str(&content)?;
                        default_variants.insert(var_name, def_opt);

                        if content.peek(Token![,]) {
                            let _: Token![,] = content.parse()?;
                        }
                    }
                }
                "compound_variants" => {
                    let content;
                    bracketed!(content in input);
                    while !content.is_empty() {
                        let item_content;
                        braced!(item_content in content);
                        let mut conds = BTreeMap::new();
                        let mut rules = Vec::new();

                        while !item_content.is_empty() {
                            let item_key = parse_key_str(&item_content)?;
                            item_content.parse::<Token![:]>()?;

                            if item_key == "class" || item_key == "css" {
                                let lit: LitStr = item_content.parse()?;
                                rules = parse_class_rules(&lit)?;
                            } else {
                                let val = parse_key_str(&item_content)?;
                                conds.insert(item_key, val);
                            }

                            if item_content.peek(Token![,]) {
                                let _: Token![,] = item_content.parse()?;
                            }
                        }

                        compound_variants.push(CompoundVariant {
                            conditions: conds,
                            rules,
                        });

                        if content.peek(Token![,]) {
                            let _: Token![,] = content.parse()?;
                        }
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        format!("Unknown key '{}' in tw_variants! macro", key),
                    ));
                }
            }

            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(TwVariantsInput {
            base_rules,
            variants,
            default_variants,
            compound_variants,
        })
    }
}

/// 笛卡尔积组合信息
struct VariantCombination {
    /// variant_name -> option_name
    options: BTreeMap<String, String>,
    compile_result: CssCompileResult,
}

fn generate_combinations(input: &TwVariantsInput, span: Span) -> Result<Vec<VariantCombination>> {
    let variant_names: Vec<String> = input.variants.iter().map(|(k, _)| k.clone()).collect();

    let mut tuples: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];

    for var_name in &variant_names {
        let opts = &input
            .variants
            .iter()
            .find(|(k, _)| k == var_name)
            .unwrap()
            .1;
        let mut next_tuples = Vec::new();

        for tuple in &tuples {
            for opt_name in opts.keys() {
                let mut new_tuple = tuple.clone();
                new_tuple.insert(var_name.clone(), opt_name.clone());
                next_tuples.push(new_tuple);
            }
        }

        tuples = next_tuples;
    }

    let mut combinations = Vec::with_capacity(tuples.len());
    let mut compile_cache: BTreeMap<String, CssCompileResult> = BTreeMap::new();

    for tuple in tuples {
        let mut combined_rules = input.base_rules.clone();

        // 1. 附加选中变体的规则
        for (var_name, opts) in &input.variants {
            if let Some(opt_name) = tuple.get(var_name)
                && let Some(rules) = opts.get(opt_name)
            {
                combined_rules.extend(rules.clone());
            }
        }

        // 2. 附加符合条件的复合变体规则
        for cv in &input.compound_variants {
            let mut matches_all = true;
            for (k, v) in &cv.conditions {
                if tuple.get(k) != Some(v) {
                    matches_all = false;
                    break;
                }
            }
            if matches_all {
                combined_rules.extend(cv.rules.clone());
            }
        }

        // 3. 编译期 deduplicate_utility_rules 智能消解冲突 (Last-wins 策略)
        let deduped = deduplicate_utility_rules(combined_rules);

        // 4. 构建 CssBlock 并使用 Rule/CssBlock 去重缓存，避免重复编译相同 CSS
        let css_block = build_css_block_from_rules(deduped)?;
        let block_ts = quote! { #css_block };
        let cache_key = block_ts.to_string();

        let compile_result = if let Some(cached) = compile_cache.get(&cache_key) {
            cached.clone()
        } else {
            let res = CssCompiler::compile_with_prefix(block_ts, span, false, "slx-twv-")?;
            compile_cache.insert(cache_key, res.clone());
            res
        };

        combinations.push(VariantCombination {
            options: tuple,
            compile_result,
        });
    }

    Ok(combinations)
}

/// `tw_variants!` 过程宏核心入口实现
pub fn tw_variants_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site();
    let input: TwVariantsInput = syn::parse2(ts)?;

    let variant_names: Vec<String> = input.variants.iter().map(|(k, _)| k.clone()).collect();
    let combinations = generate_combinations(&input, span)?;

    // 1. 生成 CSS 初始化注入 Token (基于 Class Name 去重，避免多组合重复注入同一样式 Block)
    let mut inits_tokens = Vec::new();
    let mut seen_classes = std::collections::BTreeSet::new();
    for comb in &combinations {
        if seen_classes.insert(comb.compile_result.class_name.clone()) {
            inits_tokens.push(comb.compile_result.generate_inits());
        }
    }

    // 2. 确定默认组合及 Class
    let default_tuple: BTreeMap<String, String> = variant_names
        .iter()
        .map(|k| {
            let def_val = input.default_variants.get(k).cloned().unwrap_or_else(|| {
                input
                    .variants
                    .iter()
                    .find(|(name, _)| name == k)
                    .and_then(|(_, opts)| opts.keys().next().cloned())
                    .unwrap_or_default()
            });
            (k.clone(), def_val)
        })
        .collect();

    let default_class_name = combinations
        .iter()
        .find(|c| c.options == default_tuple)
        .map(|c| c.compile_result.class_name.as_str())
        .unwrap_or("");

    // 3. 生成静态数组查表表项 (替代重复的大量 match_arms 分支)
    let table_entries = combinations.iter().map(|comb| {
        let cls_name = &comb.compile_result.class_name;
        let tuple_pats: Vec<&str> = variant_names
            .iter()
            .map(|k| comb.options.get(k).map(|s| s.as_str()).unwrap_or(""))
            .collect();

        quote! {
            (&[#(#tuple_pats),*], #cls_name)
        }
    });

    // 4. 生成变体字段
    let struct_fields = variant_names.iter().map(|name| {
        let ident = format_ident!("{}", name);
        quote! { pub #ident: ::std::option::Option<&'static str> }
    });

    let struct_field_inits = variant_names.iter().map(|name| {
        let ident = format_ident!("{}", name);
        quote! { #ident: None }
    });

    // 为每个变体属性生成匹配方法
    let pos_args: Vec<Ident> = variant_names
        .iter()
        .map(|n| format_ident!("{}", n))
        .collect();

    let pos_arg_types = variant_names
        .iter()
        .map(|_| quote! { impl ::std::convert::AsRef<str> });

    let pos_arg_opt_types = variant_names
        .iter()
        .map(|_| quote! { ::std::option::Option<impl ::std::convert::AsRef<str>> });

    let pos_arg_eval = variant_names.iter().enumerate().map(|(i, _)| {
        let arg_ident = &pos_args[i];
        let def_val = default_tuple
            .get(&variant_names[i])
            .map(|s| s.as_str())
            .unwrap_or("");
        quote! {
            let #arg_ident = #arg_ident.as_ref();
            let #arg_ident = if #arg_ident.is_empty() { #def_val } else { #arg_ident };
        }
    });

    let pos_arg_opt_eval = variant_names.iter().enumerate().map(|(i, _)| {
        let arg_ident = &pos_args[i];
        let def_val = default_tuple
            .get(&variant_names[i])
            .map(|s| s.as_str())
            .unwrap_or("");
        quote! {
            let #arg_ident = #arg_ident.as_ref().map(|s| s.as_ref()).unwrap_or(#def_val);
        }
    });

    Ok(quote! {
        {
            #(#inits_tokens)*

            static VARIANTS_TABLE: &[(&[&'static str], &'static str)] = &[
                #(#table_entries),*
            ];

            #[derive(Clone, Debug)]
            struct TwVariantsHelper {
                #(#struct_fields,)*
                pub extra_classes: ::std::option::Option<::std::string::String>,
            }

            impl TwVariantsHelper {
                pub fn new() -> Self {
                    Self {
                        #(#struct_field_inits,)*
                        extra_classes: None,
                    }
                }

                pub fn extra(mut self, extra: impl ::std::convert::AsRef<str>) -> Self {
                    self.extra_classes = Some(extra.as_ref().to_string());
                    self
                }

                fn lookup(&self, key: &[&str]) -> &'static str {
                    VARIANTS_TABLE
                        .iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, cls)| *cls)
                        .unwrap_or(#default_class_name)
                }

                pub fn get(&self, #(#pos_args: #pos_arg_types),*) -> ::std::string::String {
                    #(#pos_arg_eval)*
                    let base_cls = self.lookup(&[#(#pos_args),*]);
                    if let Some(ref extra) = self.extra_classes {
                        format!("{} {}", base_cls, extra)
                    } else {
                        base_cls.to_string()
                    }
                }

                pub fn get_opt(&self, #(#pos_args: #pos_arg_opt_types),*) -> ::std::string::String {
                    #(#pos_arg_opt_eval)*
                    let base_cls = self.lookup(&[#(#pos_args),*]);
                    if let Some(ref extra) = self.extra_classes {
                        format!("{} {}", base_cls, extra)
                    } else {
                        base_cls.to_string()
                    }
                }
            }

            TwVariantsHelper::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_tw_variants_basic() {
        let ts = quote! {
            base: "font-semibold rounded-lg",
            variants: {
                intent: {
                    primary: "bg-indigo-600 text-white",
                    secondary: "bg-slate-200 text-slate-800"
                },
                size: {
                    sm: "text-xs px-3 py-1.5",
                    md: "text-sm px-4 py-2"
                }
            },
            default_variants: {
                intent: "primary",
                size: "md"
            }
        };

        let res = tw_variants_impl(ts);
        assert!(res.is_ok(), "tw_variants_impl error: {:?}", res.err());
        let code = res.unwrap().to_string();
        assert!(code.contains("TwVariantsHelper"));
        assert!(code.contains("inject_style"));
    }

    #[test]
    fn test_tw_variants_deduplication_and_compound() {
        let ts = quote! {
            base: "px-4 py-2 bg-slate-100",
            variants: {
                size: {
                    sm: "px-2 py-1",
                    lg: "px-6 py-3"
                },
                intent: {
                    primary: "bg-indigo-600 text-white",
                    danger: "bg-rose-600 text-white"
                }
            },
            default_variants: {
                size: "sm",
                intent: "primary"
            },
            compound_variants: [
                {
                    intent: "danger",
                    size: "lg",
                    class: "shadow-xl scale-105"
                }
            ]
        };

        let res = tw_variants_impl(ts);
        assert!(res.is_ok(), "tw_variants_impl error: {:?}", res.err());
        let code = res.unwrap().to_string();
        assert!(code.contains("TwVariantsHelper"));
        assert!(code.contains("get"));
        assert!(code.contains("get_opt"));
        assert!(code.contains("extra"));
    }

    #[test]
    fn test_tw_variants_dark_mode_preservation() {
        let ts = quote! {
            base: "font-semibold",
            variants: {
                intent: {
                    primary: "bg-indigo-600 dark:bg-indigo-500 text-white"
                }
            }
        };

        let res = tw_variants_impl(ts);
        assert!(res.is_ok(), "tw_variants_impl error: {:?}", res.err());
        let code = res.unwrap().to_string();
        assert!(code.contains("#6366f1") || code.contains("#4f46e5"));
        assert!(code.contains("dark"));
    }

    #[test]
    fn test_tw_variants_table_and_caching() {
        let ts = quote! {
            base: "font-bold",
            variants: {
                intent: {
                    primary: "text-red-500",
                    alias_primary: "text-red-500"
                }
            }
        };

        let res = tw_variants_impl(ts);
        assert!(res.is_ok(), "tw_variants_impl error: {:?}", res.err());
        let code = res.unwrap().to_string();
        assert!(code.contains("VARIANTS_TABLE"));
        assert!(code.contains("lookup"));
        // Both primary and alias_primary produce identical CSS rule sets,
        // so generate_inits() is called ONCE for the deduplicated class (which emits static + component inject_style calls).
        let inject_count = code.matches("inject_style").count();
        assert_eq!(inject_count, 2, "Expected exactly 2 inject_style calls for 1 deduplicated class (static + component style injection)");
    }
}
