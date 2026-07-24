use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, braced, bracketed};

/// 复合变体结构输入
#[derive(Debug, Clone)]
pub struct CompoundVariantInput {
    pub conditions: BTreeMap<String, String>,
    pub class_str: String,
}

/// `tw_variants!` 宏 AST 输入结构
#[derive(Debug, Clone)]
pub struct TwVariantsMacroInput {
    pub base_str: String,
    /// variant_name -> Vec<(option_name, class_str)>
    pub variants: Vec<(String, Vec<(String, String)>)>,
    /// variant_name -> default_option_name
    pub default_variants: BTreeMap<String, String>,
    pub compound_variants: Vec<CompoundVariantInput>,
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

impl Parse for TwVariantsMacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut base_str = String::new();
        let mut variants = Vec::new();
        let mut default_variants = BTreeMap::new();
        let mut compound_variants = Vec::new();

        while !input.is_empty() {
            let key = parse_key_str(input)?;
            input.parse::<Token![:]>()?;

            match key.as_str() {
                "base" => {
                    let lit: LitStr = input.parse()?;
                    base_str = lit.value();
                }
                "variants" => {
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        let var_name = parse_key_str(&content)?;
                        content.parse::<Token![:]>()?;

                        let opts_content;
                        braced!(opts_content in content);
                        let mut opts_vec = Vec::new();

                        while !opts_content.is_empty() {
                            let opt_name = parse_key_str(&opts_content)?;
                            opts_content.parse::<Token![:]>()?;
                            let lit: LitStr = opts_content.parse()?;
                            opts_vec.push((opt_name, lit.value()));

                            if opts_content.peek(Token![,]) {
                                let _: Token![,] = opts_content.parse()?;
                            }
                        }

                        variants.push((var_name, opts_vec));

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
                        let mut class_str = String::new();

                        while !item_content.is_empty() {
                            let item_key = parse_key_str(&item_content)?;
                            item_content.parse::<Token![:]>()?;

                            if item_key == "class" || item_key == "css" {
                                let lit: LitStr = item_content.parse()?;
                                class_str = lit.value();
                            } else {
                                let val = parse_key_str(&item_content)?;
                                conds.insert(item_key, val);
                            }

                            if item_content.peek(Token![,]) {
                                let _: Token![,] = item_content.parse()?;
                            }
                        }

                        compound_variants.push(CompoundVariantInput {
                            conditions: conds,
                            class_str,
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

        Ok(TwVariantsMacroInput {
            base_str,
            variants,
            default_variants,
            compound_variants,
        })
    }
}

fn to_pascal_case(s: &str, span: Span) -> Ident {
    let clean = s.trim();
    if clean.is_empty() {
        return Ident::new("Default", span);
    }
    let mut parts = Vec::new();
    for chunk in clean.split(&['-', '_', ' '][..]) {
        if chunk.is_empty() {
            continue;
        }
        let mut chars = chunk.chars();
        if let Some(first) = chars.next() {
            let mut uppercase_part = first.to_uppercase().to_string();
            uppercase_part.push_str(chars.as_str());
            parts.push(uppercase_part);
        }
    }
    let mut res = parts.join("");
    if res.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        res = format!("Val{}", res);
    }
    Ident::new(&res, span)
}

/// `tw_variants!` 过程宏核心入口实现
///
/// 解析 CVA 风格 DSL 并展开为对 `declare_variants!` 声明式宏的调用及兼容 helper 方法。
pub fn tw_variants_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site();
    let input: TwVariantsMacroInput = syn::parse2(ts)?;

    let base_str = &input.base_str;

    let var_decls: Vec<_> = input
        .variants
        .iter()
        .map(|(var_name, opts)| {
            let var_name_ident = format_ident!("{}", var_name);
            let var_type_ident = format_ident!("TwVariant_{}", var_name);
            let def_opt_str = input
                .default_variants
                .get(var_name)
                .cloned()
                .unwrap_or_else(|| opts.first().map(|(k, _)| k.clone()).unwrap_or_default());
            let def_opt_ident = to_pascal_case(&def_opt_str, span);

            let opt_entries = opts.iter().map(|(opt_name, opt_cls)| {
                let opt_ident = to_pascal_case(opt_name, span);
                quote! {
                    #opt_ident => ::silex::macros::tw!(#opt_cls)
                }
            });

            quote! {
                pub #var_name_ident : #var_type_ident [default = #def_opt_ident] = {
                    #(#opt_entries),*
                }
            }
        })
        .collect();

    let compound_entries: Vec<_> = input
        .compound_variants
        .iter()
        .map(|cv| {
            let cond_checks = cv.conditions.iter().map(|(var_name, opt_val)| {
                let var_ident = format_ident!("{}", var_name);
                let var_type_ident = format_ident!("TwVariant_{}", var_name);
                let opt_ident = to_pascal_case(opt_val, span);
                quote! {
                    #var_ident == #var_type_ident :: #opt_ident
                }
            });

            let cmp_cls = &cv.class_str;
            quote! {
                ( #(#cond_checks),* ) => ::silex::macros::tw!(#cmp_cls)
            }
        })
        .collect();

    let get_params: Vec<_> = input
        .variants
        .iter()
        .map(|(var_name, _)| {
            let var_ident = format_ident!("{}", var_name);
            quote! { #var_ident: impl ::std::convert::AsRef<str> }
        })
        .collect();

    let get_inits: Vec<_> = input
        .variants
        .iter()
        .map(|(var_name, _)| {
            let var_ident = format_ident!("{}", var_name);
            let var_type_ident = format_ident!("TwVariant_{}", var_name);
            quote! { #var_ident: #var_type_ident::from(#var_ident) }
        })
        .collect();

    let get_opt_params: Vec<_> = input
        .variants
        .iter()
        .map(|(var_name, _)| {
            let var_ident = format_ident!("{}", var_name);
            quote! { #var_ident: ::std::option::Option<impl ::std::convert::AsRef<str>> }
        })
        .collect();

    let get_opt_inits: Vec<_> = input
        .variants
        .iter()
        .map(|(var_name, _)| {
            let var_ident = format_ident!("{}", var_name);
            let var_type_ident = format_ident!("TwVariant_{}", var_name);
            quote! {
                #var_ident: #var_ident.map(#var_type_ident::from).unwrap_or_default()
            }
        })
        .collect();

    let compound_block = if compound_entries.is_empty() {
        quote! {}
    } else {
        quote! {
            compound_variants: [
                #(#compound_entries),*
            ]
        }
    };

    Ok(quote! {
        {
            ::silex::css::declare_variants! {
                pub struct TwVariantsHelper {
                    base: ::silex::macros::tw!(#base_str),
                    variants: {
                        #(#var_decls),*
                    },
                    #compound_block
                }
            }

            #[allow(dead_code)]
            impl TwVariantsHelper {
                pub fn get(&self, #(#get_params),*) -> ::std::string::String {
                    let config = Self {
                        #(#get_inits),*
                    };
                    ::silex::css::cx!(config)
                }

                pub fn get_opt(&self, #(#get_opt_params),*) -> ::std::string::String {
                    let config = Self {
                        #(#get_opt_inits),*
                    };
                    ::silex::css::cx!(config)
                }
            }

            TwVariantsHelper::new()
        }
    })
}
