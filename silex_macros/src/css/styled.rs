use crate::css::compiler::CssCompiler;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, FnArg, Generics, Ident, Result, Token, Visibility};

/// A variant group, representing `prop_name: { variant1: { ... }, variant2: { ... } }`
pub struct VariantGroup {
    pub prop_name: Ident,
    pub variants: Vec<(Ident, TokenStream)>,
}

/// Represents the syntax tree for a `styled!` macro call.
pub struct StyledComponent {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub generics: Generics,
    pub tag: Ident,
    pub props: Punctuated<FnArg, Token![,]>,
    pub css_block: TokenStream,
    pub variants: Vec<VariantGroup>,
}

impl Parse for StyledComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        // Peek if we have generics: Stack<T> <div> vs Stack <div>
        let mut generics = Generics::default();
        if input.peek(Token![<]) {
            let fork = input.fork();
            let _gen: Result<Generics> = fork.parse();
            if fork.peek(Token![<]) {
                // If there's another '<' after the first block, the first was generics
                generics = input.parse()?;
            }
        }

        // 2. Parse Tag: <div>
        if !input.peek(Token![<]) {
            return Err(input.error("Expected `<` followed by a tag name or component name"));
        }
        input.parse::<Token![<]>()?;
        let tag: Ident = input.parse()?;
        if !input.peek(Token![>]) {
            return Err(input.error("Expected `>`"));
        }
        input.parse::<Token![>]>()?;

        // 3. Parse Props: (...)
        let props_content;
        syn::parenthesized!(props_content in input);
        let props = props_content.parse_terminated(FnArg::parse, Token![,])?;

        // 3.5 Parse Where Clause
        if input.peek(Token![where]) {
            generics.where_clause = Some(input.parse()?);
        }

        // 4. Parse CSS Block and Variants: {...}
        let css_content;
        syn::braced!(css_content in input);

        let mut css_block = proc_macro2::TokenStream::new();
        let mut variants = Vec::new();

        while !css_content.is_empty() {
            // Check for `variants: {`
            let is_variants = css_content.peek(Ident)
                && css_content.peek2(Token![:])
                && css_content.peek3(syn::token::Brace);
            if is_variants {
                let fork = css_content.fork();
                let ident: Ident = fork.parse()?;
                if ident == "variants" {
                    css_content.parse::<Ident>()?; // variants
                    css_content.parse::<Token![:]>()?; // :
                    let variants_content;
                    syn::braced!(variants_content in css_content);

                    while !variants_content.is_empty() {
                        let prop_name: Ident = variants_content.parse()?;
                        let _colon: Token![:] = variants_content.parse()?;
                        let prop_variants_content;
                        syn::braced!(prop_variants_content in variants_content);

                        let mut group_variants = Vec::new();
                        while !prop_variants_content.is_empty() {
                            let variant_name: Ident = prop_variants_content.parse()?;
                            let _colon2: Token![:] = prop_variants_content.parse()?;
                            let variant_css;
                            syn::braced!(variant_css in prop_variants_content);
                            group_variants
                                .push((variant_name, variant_css.parse::<TokenStream>()?));
                        }

                        variants.push(VariantGroup {
                            prop_name,
                            variants: group_variants,
                        });
                    }
                    continue;
                }
            }

            let tt: proc_macro2::TokenTree = css_content.parse()?;
            css_block.extend(std::iter::once(tt));
        }

        Ok(StyledComponent {
            attrs,
            vis,
            name,
            generics,
            tag,
            props,
            css_block,
            variants,
        })
    }
}

pub fn styled_impl(input: TokenStream) -> Result<TokenStream> {
    let parsed: StyledComponent = syn::parse2(input)?;
    let attrs = &parsed.attrs;
    let vis = &parsed.vis;
    let name = &parsed.name;
    let tag = &parsed.tag;
    let props = &parsed.props;
    let css_block = parsed.css_block;
    let variants = &parsed.variants;
    let generics = &parsed.generics;

    let compile_result = CssCompiler::compile(css_block, tag.span())?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let final_css = compile_result.final_css;
    let expressions = compile_result.expressions;
    let dynamic_rules = compile_result.dynamic_rules;
    let theme_refs = compile_result.theme_refs;

    let mut var_decls: Vec<TokenStream> = expressions
        .iter()
        .enumerate()
        .map(|(i, (prop, expr_ts))| -> Result<TokenStream> {
            let var_ident = quote::format_ident!("var_{}", i);
            let prop_type = crate::css::get_prop_type(prop, tag.span())?;
            Ok(quote! {
                let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr_ts);
            })
        })
        .collect::<Result<Vec<TokenStream>>>()?;

    let mut style_bindings: Vec<TokenStream> = expressions
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let var_name = format!("--{}-{}", class_name, i);
            let var_ident = quote::format_ident!("var_{}", i);
            quote! {
                .style((#var_name, #var_ident()))
            }
        })
        .collect();

    let theme_assertions: Vec<TokenStream> = theme_refs
        .iter()
        .map(|(prop, key)| -> Result<TokenStream> {
            let prop_type = if prop == "any" {
                quote! { ::silex::css::types::props::Any }
            } else {
                crate::css::get_prop_type(prop, tag.span())?
            };

            let mut theme_name = quote! { Theme };
            for attr in attrs {
                if attr.path().is_ident("theme")
                    && let Ok(nested) = attr.parse_args::<syn::Ident>()
                {
                    theme_name = quote! { #nested };
                }
            }

            let key_path: Vec<TokenStream> = key
                .split('.')
                .map(|s| {
                    let id = quote::format_ident!("{}", s);
                    quote! { #id }
                })
                .collect();

            Ok(quote! {
                const _: () = {
                    fn assert_valid<V: ::silex::css::types::ValidFor<#prop_type>>(_: &V) {}
                    #[allow(non_upper_case_globals, unused_variables)]
                    let _ = |t: &#theme_name| {
                        assert_valid(&t #(.#key_path)*);
                    };
                };
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut variant_injections = Vec::new();
    let mut variant_class_bindings = Vec::new();
    let mut prop_sig_bindings = Vec::new();
    let mut dynamic_rule_inits = Vec::new();
    let mut dynamic_rule_classes = Vec::new();

    for group in &parsed.variants {
        let prop = &group.prop_name;
        let sig_ident = quote::format_ident!("{}_sig", prop);

        prop_sig_bindings.push(quote! {
            let #sig_ident = ::silex::prelude::IntoRx::into_rx(#prop.clone());
        });

        let mut match_arms = Vec::new();

        for (variant_name, variant_css) in &group.variants {
            let compile_result = CssCompiler::compile(variant_css.clone(), variant_name.span())?;
            let variant_class_name = compile_result.class_name;
            let variant_style_id = compile_result.style_id;
            let variant_final_css = compile_result.final_css;

            // --- 动态变量求值与注入 ---
            for (i, (prop_for_expr, expr_ts)) in compile_result.expressions.into_iter().enumerate()
            {
                // 确保每一个 variant 的动态变量名不冲突
                let var_ident = quote::format_ident!("dyn_var_{}_{}_{}", prop, variant_name, i);
                let prop_type = crate::css::get_prop_type(&prop_for_expr, variant_name.span())?;

                var_decls.push(quote! {
                    let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr_ts);
                });

                let var_name = format!("--{}-{}", variant_class_name, i);
                // 无条件注入动态内联样式
                style_bindings.push(quote! {
                    .style((#var_name, #var_ident()))
                });
            }

            variant_injections.push(quote! {
                ::silex::css::inject_style(#variant_style_id, #variant_final_css);
            });

            // 处理变体中的动态规则（支持变体内的 $(...) 选择器等）
            for (rule_idx, rule) in compile_result.dynamic_rules.into_iter().enumerate() {
                let template = &rule.template;
                let mut dyn_var_decls = Vec::new();
                let mut eval_vars = Vec::new();

                for (expr_idx, (p, expr_ts)) in rule.expressions.iter().enumerate() {
                    let var_ident = quote::format_ident!(
                        "dyn_rule_var_{}_{}_{}_{}",
                        prop,
                        variant_name,
                        rule_idx,
                        expr_idx
                    );
                    let prop_type = crate::css::get_prop_type(p, variant_name.span())?;
                    dyn_var_decls.push(quote! {
                        let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr_ts);
                    });
                    eval_vars.push(var_ident);
                }

                let manager_ident =
                    quote::format_ident!("manager_{}_{}_{}", prop, variant_name, rule_idx);
                dynamic_rule_inits.push(quote! {
                    #(#dyn_var_decls)*
                    let #manager_ident = ::std::rc::Rc::new(::std::cell::RefCell::new(Some(::silex::css::DynamicStyleManager::new())));
                    let manager_cleanup = #manager_ident.clone();
                    ::silex::core::reactivity::on_cleanup(move || {
                        if let Ok(mut opt_mgr) = manager_cleanup.try_borrow_mut() {
                            let _ = opt_mgr.take();
                        }
                    });
                });

                let variant_name_lower = variant_name.to_string().to_lowercase();

                // 将 variant class 绑定到元素的 dynamic class
                dynamic_rule_classes.push(quote! {
                    .class({
                        let manager = #manager_ident.clone();
                        let sig = #sig_ident.clone();
                        ::silex::prelude::rx! {
                            if sig.get() != #variant_name_lower {
                                return "".to_string();
                            }

                            let mut resolved_rule = ::std::string::ToString::to_string(#template);
                            #(
                                let val = #eval_vars();
                                if let Some(pos) = resolved_rule.find("{}") {
                                    resolved_rule.replace_range(pos..pos + 2, &val);
                                }
                            )*

                            let hash_val = ::silex::hash::css::hash_one(&resolved_rule);
                            let mut buf = [0u8; 13];
                            let b36 = ::silex::hash::css::encode_base36(hash_val, &mut buf);
                            let dyn_class = format!("{}-dyn-{}", #variant_class_name, b36);

                            if let Ok(mut opt) = manager.try_borrow_mut() {
                                if let Some(mgr) = opt.as_mut() {
                                    let rule_with_dyn_class = resolved_rule.replace(#variant_class_name, &dyn_class);
                                    mgr.update(&dyn_class, &rule_with_dyn_class);
                                }
                            }

                            dyn_class
                        }
                    })
                });
            }

            let variant_name_str = variant_name.to_string();
            let variant_name_lower = variant_name_str.to_lowercase();
            match_arms.push(quote! {
                v if ::std::string::ToString::to_string(&v).to_lowercase() == #variant_name_lower => #variant_class_name,
            });
        }

        variant_class_bindings.push(quote! {
            .class(::silex::prelude::rx! {
                let val = #sig_ident.get();
                match val {
                    #(#match_arms)*
                    _ => "",
                }
            })
        });
    }

    let mut has_children = false;
    let mut style_prop = None;
    let mut existing_prop_names = std::collections::HashSet::new();
    for arg in props {
        if let syn::FnArg::Typed(pat_type) = arg
            && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
        {
            let name = &pat_ident.ident;
            existing_prop_names.insert(name.clone());
            if name == "children" {
                has_children = true;
            }
            if name == "style" {
                style_prop = Some(name.clone());
            }
        }
    }

    let children_binding = if has_children {
        quote! { children }
    } else {
        quote! { () }
    };

    let style_prop_binding = if let Some(ident) = style_prop {
        quote! { .style(#ident) }
    } else {
        quote! {}
    };

    for (rule_idx, rule) in dynamic_rules.iter().enumerate() {
        let template = &rule.template;
        let mut dyn_var_decls = Vec::new();
        let mut eval_vars = Vec::new();

        for (expr_idx, (prop, expr_ts)) in rule.expressions.iter().enumerate() {
            let var_ident = quote::format_ident!("dyn_var_{}_{}", rule_idx, expr_idx);
            let prop_type = crate::css::get_prop_type(prop, tag.span())?;

            dyn_var_decls.push(quote! {
                let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr_ts);
            });
            eval_vars.push(var_ident);
        }

        let manager_ident = quote::format_ident!("manager_{}", rule_idx);
        dynamic_rule_inits.push(quote! {
            #(#dyn_var_decls)*
            let #manager_ident = ::std::rc::Rc::new(::std::cell::RefCell::new(Some(::silex::css::DynamicStyleManager::new())));
            let manager_cleanup = #manager_ident.clone();
            ::silex::core::reactivity::on_cleanup(move || {
                if let Ok(mut opt_mgr) = manager_cleanup.try_borrow_mut() {
                    let _ = opt_mgr.take();
                }
            });
        });

        dynamic_rule_classes.push(quote! {
            .class({
                let manager = #manager_ident.clone();
                ::silex::prelude::rx! {
                    let mut resolved_rule = ::std::string::ToString::to_string(#template);
                    #(
                        let val = #eval_vars();
                        if let Some(pos) = resolved_rule.find("{}") {
                            resolved_rule.replace_range(pos..pos + 2, &val);
                        }
                    )*
                    let hash_val = ::silex::hash::css::hash_one(&resolved_rule);
                    let mut buf = [0u8; 13];
                    let b36 = ::silex::hash::css::encode_base36(hash_val, &mut buf);
                    let dyn_class = format!("{}-dyn-{}", #class_name, b36);

                    if let Ok(mut opt) = manager.try_borrow_mut() {
                        if let Some(mgr) = opt.as_mut() {
                            let rule_with_dyn_class = resolved_rule.replace(#class_name, &dyn_class);
                            mgr.update(&dyn_class, &rule_with_dyn_class);
                        }
                    }

                    dyn_class
                }
            })
        });
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Create a new Punctuated list for all props to avoid trailing comma issues
    let mut all_fn_args = props.clone();

    // Ensure it doesn't have a trailing comma before we push more args
    if !all_fn_args.empty_or_trailing()
        && variants
            .iter()
            .any(|v| !existing_prop_names.contains(&v.prop_name))
    {
        // all_fn_args.push_punctuation(Token![,](Span::call_site()));
        // Punctuated handles separator automatically when pushing
    }

    for v in variants {
        if !existing_prop_names.contains(&v.prop_name) {
            let prop = &v.prop_name;
            let arg: syn::FnArg = syn::parse_quote! {
                #[prop(into, default)]
                #prop: ::silex::core::reactivity::Signal<::std::string::String>
            };
            all_fn_args.push(arg);
        }
    }

    // --- 启发式精准类型恢复（面向原生标签）与自动透传 ---
    let tag_str = tag.to_string();
    let is_native_tag = tag_str.chars().next().unwrap().is_ascii_lowercase();

    let return_type = if is_native_tag {
        let pascal_name = match tag_str.as_str() {
            "a" => "A".to_string(),
            "data" => "DataTag".to_string(),
            "option" => "OptionTag".to_string(),
            "param" => "ParamTag".to_string(),
            "time" => "TimeTag".to_string(),
            _ => {
                let mut c = tag_str.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        };
        let pascal_ident = syn::Ident::new(&pascal_name, tag.span());
        quote! { ::silex::dom::element::TypedElement<::silex::dom::element::tags::#pascal_ident> }
    } else {
        quote! { impl ::silex::dom::attribute::AttributeBuilder + ::silex::dom::view::View #where_clause }
    };

    let component_struct_name = quote::format_ident!("{}Component", name);
    let mut extra_impls = quote! {};
    if is_native_tag {
        if matches!(
            tag_str.as_str(),
            "button"
                | "input"
                | "form"
                | "select"
                | "textarea"
                | "option"
                | "optgroup"
                | "fieldset"
        ) {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::FormAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if matches!(tag_str.as_str(), "a" | "area" | "link") {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::AnchorAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if tag_str == "label" {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::LabelAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if matches!(
            tag_str.as_str(),
            "img" | "video" | "audio" | "source" | "iframe" | "embed" | "object"
        ) {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::MediaAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if matches!(tag_str.as_str(), "dialog" | "details") {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::OpenAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if matches!(tag_str.as_str(), "td" | "th") {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::TableCellAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
        if tag_str == "th" {
            extra_impls.extend(quote! {
                impl #impl_generics ::silex::html::TableHeaderAttributes for #component_struct_name #ty_generics #where_clause {}
            });
        }
    }

    let expanded = quote! {
        #(#attrs)*
        #[::silex::macros::component]
        #vis fn #name #impl_generics (
            #all_fn_args
        ) -> #return_type {
            #(#var_decls)*
            #(#prop_sig_bindings)*
            #(#theme_assertions)*

            ::silex::css::inject_style(#style_id, #final_css);
            #(#variant_injections)*

            #(#dynamic_rule_inits)*

            ::silex::html::#tag(#children_binding)
                .class(#class_name)
                #style_prop_binding
                #(#style_bindings)*
                #(#variant_class_bindings)*
                #(#dynamic_rule_classes)*
        }

        #extra_impls
    };

    Ok(expanded)
}
