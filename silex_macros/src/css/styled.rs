use crate::css::compiler::CssCompiler;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{FnArg, Ident, Result, Token, Visibility};

/// A variant group, representing `prop_name: { variant1: { ... }, variant2: { ... } }`
pub struct VariantGroup {
    pub prop_name: Ident,
    pub variants: Vec<(Ident, TokenStream)>,
}

/// Represents the syntax tree for a `styled!` macro call.
pub struct StyledComponent {
    pub vis: Visibility,
    pub name: Ident,
    pub tag: Ident,
    pub props: Punctuated<FnArg, Token![,]>,
    pub css_block: TokenStream,
    pub variants: Vec<VariantGroup>,
}

impl Parse for StyledComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        // 1. Parse Visibility and Name
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        // 2. Parse Tag: <button>
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
            vis,
            name,
            tag,
            props,
            css_block,
            variants,
        })
    }
}

pub fn styled_impl(input: TokenStream) -> Result<TokenStream> {
    let parsed: StyledComponent = syn::parse2(input)?;

    let vis = parsed.vis;
    let name = parsed.name;
    let tag = parsed.tag;
    let props = parsed.props;

    let compile_result = CssCompiler::compile(parsed.css_block, tag.span())?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let final_css = compile_result.final_css;
    let expressions = compile_result.expressions;
    let hash = compile_result.hash;
    let dynamic_rules = compile_result.dynamic_rules;

    let var_decls: Vec<TokenStream> = expressions
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

    let style_bindings: Vec<TokenStream> = expressions
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let var_name = format!("--slx-{:x}-{}", hash, i);
            let var_ident = quote::format_ident!("var_{}", i);
            quote! {
                .style((#var_name, move || #var_ident()))
            }
        })
        .collect();

    let mut variant_injections = Vec::new();
    let mut variant_class_bindings = Vec::new();
    let mut prop_sig_bindings = Vec::new();

    for group in &parsed.variants {
        let prop = &group.prop_name;
        let sig_ident = quote::format_ident!("{}_sig", prop);

        prop_sig_bindings.push(quote! {
            let #sig_ident = ::silex::prelude::IntoSignal::into_signal(#prop.clone());
        });

        let mut match_arms = Vec::new();

        for (variant_name, variant_css) in &group.variants {
            let compile_result = CssCompiler::compile(variant_css.clone(), variant_name.span())?;
            if !compile_result.expressions.is_empty() {
                return Err(syn::Error::new(
                    variant_name.span(),
                    "Dynamic expressions $(...) are not supported inside variant blocks. Variants must be static.",
                ));
            }
            if !compile_result.dynamic_rules.is_empty() {
                return Err(syn::Error::new(
                    variant_name.span(),
                    "Dynamic rules $(...) are not supported inside variant blocks. Variants must be static.",
                ));
            }

            let class_name = compile_result.class_name;
            let style_id = compile_result.style_id;
            let final_css = compile_result.final_css;

            variant_injections.push(quote! {
                ::silex::css::inject_style(#style_id, #final_css);
            });

            let variant_name_lower = variant_name.to_string().to_lowercase();
            match_arms.push(quote! {
                #variant_name_lower => #class_name,
            });
        }

        variant_class_bindings.push(quote! {
            .class(move || {
                let val = ::std::string::ToString::to_string(&#sig_ident.get()).to_lowercase();
                match val.as_str() {
                    #(#match_arms)*
                    _ => "",
                }
            })
        });
    }

    let mut has_children = false;
    for arg in &props {
        if let syn::FnArg::Typed(pat_type) = arg
            && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
            && pat_ident.ident == "children"
        {
            has_children = true;
        }
    }

    let children_binding = if has_children {
        quote! { children }
    } else {
        quote! { () }
    };

    let mut dynamic_rule_effects = Vec::new();
    if !dynamic_rules.is_empty() {
        dynamic_rule_effects.push(quote! {
            static INSTANCE_COUNTER: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
            let instance_id = INSTANCE_COUNTER.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
            let dyn_style_id = format!("{}-dyn-{}", #class_name, instance_id);
            let manager = ::std::rc::Rc::new(::silex::css::DynamicStyleManager::new(&dyn_style_id));
        });

        let mut dyn_var_decls = Vec::new();
        let mut template_blocks = Vec::new();

        for (rule_idx, rule) in dynamic_rules.iter().enumerate() {
            let template = &rule.template;
            let mut eval_vars = Vec::new();

            for (expr_idx, (prop, expr_ts)) in rule.expressions.iter().enumerate() {
                let var_ident = quote::format_ident!("dyn_var_{}_{}", rule_idx, expr_idx);
                let prop_type = crate::css::get_prop_type(prop, tag.span())?;

                dyn_var_decls.push(quote! { let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr_ts); });

                let clone_ident = quote::format_ident!("dyn_var_{}_{}_clone", rule_idx, expr_idx);
                dyn_var_decls.push(quote! { let #clone_ident = #var_ident.clone(); });
                eval_vars.push(clone_ident);
            }

            template_blocks.push(quote! {
                {
                    let mut result_rule = ::std::string::ToString::to_string(#template);
                    let vals = [ #( #eval_vars() ),* ];
                    for val in vals {
                        if let Some(pos) = result_rule.find("{}") {
                            result_rule.replace_range(pos..pos+2, &val);
                        }
                    }
                    combined_css.push_str(&result_rule);
                    combined_css.push('\n');
                }
            });
        }

        dynamic_rule_effects.push(quote! {
            #(#dyn_var_decls)*
            ::silex::core::reactivity::Effect::new(move |_| {
                let mut combined_css = ::std::string::String::new();
                #(#template_blocks)*
                manager.update(&combined_css);
            });
        });
    }

    let expanded = quote! {
        #[::silex::prelude::component]
        #vis fn #name(
            #props
        ) -> impl ::silex::dom::View {
            #(#var_decls)*
            #(#prop_sig_bindings)*

            ::silex::css::inject_style(#style_id, #final_css);
            #(#variant_injections)*

            #(#dynamic_rule_effects)*

            ::silex::html::#tag(#children_binding)
                .class(#class_name)
                #(#style_bindings)*
                #(#variant_class_bindings)*
        }
    };

    Ok(expanded)
}
