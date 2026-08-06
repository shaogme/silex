use crate::css::compiler::{CssCompiler, DynamicRule};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, FnArg, GenericArgument, Generics, Ident, PathArguments, Result, Token, Type,
    Visibility,
};

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
    pub is_unsafe: bool,
}

impl Parse for StyledComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        attrs.retain(|attr| {
            if attr.path().is_ident("standalone") {
                return false;
            }
            true
        });

        let vis: Visibility = input.parse()?;
        let is_unsafe = input.peek(Token![unsafe]);
        if is_unsafe {
            input.parse::<Token![unsafe]>()?;
        }
        let name: Ident = input.parse()?;

        // Peek if we have generics
        let mut generics = Generics::default();
        if input.peek(Token![<]) {
            let fork = input.fork();
            let _: Result<Generics> = fork.parse();
            if fork.peek(Token![<]) {
                generics = input.parse()?;
            }
        }

        if !input.peek(Token![<]) {
            return Err(input.error("Expected `<` followed by a tag name or component name"));
        }
        input.parse::<Token![<]>()?;
        let tag: Ident = input.parse()?;
        if !input.peek(Token![>]) {
            return Err(input.error("Expected `>`"));
        }
        input.parse::<Token![>]>()?;

        let props_content;
        syn::parenthesized!(props_content in input);
        let props = props_content.parse_terminated(FnArg::parse, Token![,])?;

        if input.peek(Token![where]) {
            generics.where_clause = Some(input.parse()?);
        }

        let css_content;
        syn::braced!(css_content in input);

        let mut css_block = TokenStream::new();
        let mut variants = Vec::new();

        while !css_content.is_empty() {
            if css_content.peek(Ident)
                && css_content.peek2(Token![:])
                && css_content.peek3(syn::token::Brace)
            {
                let ident: Ident = css_content.fork().parse()?;
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
                            if prop_variants_content.peek(syn::LitStr) {
                                let lit: syn::LitStr = prop_variants_content.parse()?;
                                #[cfg(feature = "tw")]
                                {
                                    let raw_str = lit.value();
                                    let anchor = crate::css::tw::parser::TokenAnchor::from_lit_str(
                                        &raw_str, &lit,
                                    );
                                    let rules = crate::css::tw::parser::parse_class_list(
                                        &anchor,
                                        &mut Vec::new(),
                                    )?;
                                    let css_block =
                                        crate::css::tw::codegen::build_css_block_from_rules(rules)?;
                                    let ts = quote::quote! { #css_block };
                                    group_variants.push((variant_name, ts));
                                }
                                #[cfg(not(feature = "tw"))]
                                {
                                    return Err(syn::Error::new(
                                        lit.span(),
                                        "Inline Tailwind string variants require the `tw` feature flag to be enabled in `silex_macros`.",
                                    ));
                                }
                            } else {
                                let variant_css;
                                syn::braced!(variant_css in prop_variants_content);
                                group_variants
                                    .push((variant_name, variant_css.parse::<TokenStream>()?));
                            }
                            if prop_variants_content.peek(Token![,]) {
                                let _: Token![,] = prop_variants_content.parse()?;
                            }
                        }
                        if variants_content.peek(Token![,]) {
                            let _: Token![,] = variants_content.parse()?;
                        }
                        variants.push(VariantGroup {
                            prop_name,
                            variants: group_variants,
                        });
                    }
                    continue;
                }
            }
            css_block.extend(std::iter::once(
                css_content.parse::<proc_macro2::TokenTree>()?,
            ));
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
            is_unsafe,
        })
    }
}

pub fn styled_impl(input: TokenStream) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let parsed: StyledComponent = syn::parse2(input)?;
    let tag = &parsed.tag;
    let name = &parsed.name;

    let compile_result = CssCompiler::compile_with_prefix(
        parsed.css_block,
        tag.span(),
        parsed.is_unsafe,
        "slx-st-",
    )?;

    let scope = styled_scope_lifetime(&parsed.generics);
    if (!compile_result.expressions.is_empty()
        || !compile_result.dynamic_rules.is_empty()
        || !parsed.variants.is_empty())
        && scope.ident == "static"
        && !has_scope_lifetime(&parsed.generics)
    {
        return Err(syn::Error::new(
            name.span(),
            "styled! 的动态 CSS 或 variants 必须声明 `<'scope>`；宏不会创建隐式 Runtime。",
        ));
    }

    // 静态声明的编译期类型断言：属性名与「一眼能定型」的字面量取值
    let mut assertions = crate::css::generate_static_assertions(&compile_result.assertions)?;
    let base_style_inits = compile_result.generate_inits();
    let layer = compile_result.layer;

    let mut var_decls = Vec::new();
    let mut style_bindings = Vec::new();
    let mut dynamic_rule_descriptors = Vec::new();

    // 1. Process base dynamic values
    process_dynamic_entries(
        &compile_result.expressions,
        &compile_result.class_name,
        tag.span(),
        &mut var_decls,
        &mut style_bindings,
        "",
    )?;

    // 2. Process base dynamic rules
    for (idx, rule) in compile_result.dynamic_rules.into_iter().enumerate() {
        expand_dynamic_rule(
            idx,
            rule,
            &compile_result.class_name,
            tag.span(),
            &mut dynamic_rule_descriptors,
            &mut var_decls,
            None,
        )?;
    }

    // 3. Process Variants
    let mut variant_injections = Vec::new();
    let mut prop_sig_bindings = Vec::new();
    let mut variant_group_descriptors = Vec::new();

    for (group_index, group) in parsed.variants.iter().enumerate() {
        let prop = &group.prop_name;
        let sig_ident = quote::format_ident!("{}_sig", prop);
        prop_sig_bindings.push(quote! {
            let #sig_ident = #prop.clone().into_rx();
        });

        let mut variant_classes = Vec::new();
        for (v_name, v_css) in &group.variants {
            let res = CssCompiler::compile_with_prefix(
                v_css.clone(),
                v_name.span(),
                parsed.is_unsafe,
                "slx-st-",
            )?;
            let v_class = res.class_name.clone();
            assertions.extend(crate::css::generate_static_assertions(&res.assertions)?);
            variant_injections.push(res.generate_inits());

            process_dynamic_entries(
                &res.expressions,
                &v_class,
                v_name.span(),
                &mut var_decls,
                &mut style_bindings,
                &format!("_{}_{}", prop, v_name),
            )?;

            for (idx, rule) in res.dynamic_rules.into_iter().enumerate() {
                expand_dynamic_rule(
                    idx,
                    rule,
                    &v_class,
                    v_name.span(),
                    &mut dynamic_rule_descriptors,
                    &mut var_decls,
                    Some((group_index, &sig_ident, &v_name.to_string().to_lowercase())),
                )?;
            }

            let v_name_lower = v_name.to_string().to_lowercase();
            variant_classes.push(quote! { (#v_name_lower, #v_class) });
        }

        variant_group_descriptors.push(quote! {
            #__silex::css::StyledVariantGroup::new(
                #sig_ident,
                ::std::vec![ #(#variant_classes),* ],
            )
        });
    }

    // Component Props logic
    let mut all_fn_args = parsed.props.clone();
    let existing_props: std::collections::HashSet<_> = parsed
        .props
        .iter()
        .filter_map(|a| {
            if let syn::FnArg::Typed(pt) = a
                && let syn::Pat::Ident(pi) = &*pt.pat
            {
                return Some(pi.ident.clone());
            }
            None
        })
        .collect();

    for v in &parsed.variants {
        if !existing_props.contains(&v.prop_name) {
            let p = &v.prop_name;
            all_fn_args.push(syn::parse_quote! {
                #[prop(into)] #[chain] #p:
                    #__silex::core::reactivity::Signal<#scope, ::std::string::String>
            });
        }
    }

    for arg in &mut all_fn_args {
        if let syn::FnArg::Typed(arg) = arg {
            normalize_scoped_type(&mut arg.ty, &scope);
        }
    }

    let has_children = existing_props.contains(&quote::format_ident!("children"));
    let children_binding = if has_children {
        quote! { children }
    } else {
        quote! { () }
    };
    let style_prop_binding = if existing_props.contains(&quote::format_ident!("style")) {
        quote! { .style(style.clone()) }
    } else {
        quote! {}
    };

    let tag_str = tag.to_string();
    let return_type = get_tag_return_type(
        &tag_str,
        tag.span(),
        parsed.generics.where_clause.as_ref(),
        &scope,
    );
    let filtered_attrs: Vec<_> = parsed
        .attrs
        .into_iter()
        .filter(|a| !a.path().is_ident("theme"))
        .collect();

    let vis = parsed.vis;
    let generics = parsed.generics;
    let class_name = &compile_result.class_name;

    let has_dynamic_bindings = !style_bindings.is_empty()
        || !dynamic_rule_descriptors.is_empty()
        || !variant_group_descriptors.is_empty();
    let immediate_style_inits = if has_dynamic_bindings {
        quote! {}
    } else {
        quote! {
            #base_style_inits
            #(#variant_injections)*
        }
    };
    let deferred_style_attribute = if has_dynamic_bindings {
        quote! {
            .apply(#__silex::dom::attribute::AttrOp::custom_with_inputs(
                #__silex::core::RuntimeInputs::new(),
                move |_, _| {
                    #base_style_inits
                    #(#variant_injections)*
                },
            ))
        }
    } else {
        quote! {}
    };
    let styled_variant_binding =
        if dynamic_rule_descriptors.is_empty() && variant_group_descriptors.is_empty() {
            quote! {}
        } else {
            quote! {
                .apply(#__silex::css::StyledVariantBinding::new(
                    #layer,
                    ::std::vec![ #(#dynamic_rule_descriptors),* ],
                    ::std::vec![ #(#variant_group_descriptors),* ],
                ).into_op())
            }
        };

    let node_init = if is_void_tag(&tag_str) {
        quote! { #__silex::html::#tag() }
    } else {
        quote! { #__silex::html::#tag(#children_binding) }
    };

    let fn_body: syn::Block = syn::parse_quote! {
        {
            #assertions

            #(#var_decls)*
            #(#prop_sig_bindings)*

            #immediate_style_inits

            #node_init
                .class(#class_name)
                #style_prop_binding
                #deferred_style_attribute
                .apply(#__silex::dom::attribute::AttrOp::CombinedStyles(#__silex::dom::attribute::CombinedStyles {
                    statics: ::std::vec![],
                    properties: ::std::vec![ #(#style_bindings),* ],
                    sheets: ::std::vec![],
                }))
                #styled_variant_binding
        }
    };

    let item_fn = syn::ItemFn {
        attrs: filtered_attrs,
        vis,
        sig: syn::Signature {
            constness: None,
            asyncness: None,
            unsafety: None,
            abi: None,
            fn_token: syn::token::Fn::default(),
            ident: name.clone(),
            generics,
            paren_token: syn::token::Paren::default(),
            inputs: all_fn_args.into_iter().collect(),
            variadic: None,
            output: syn::ReturnType::Type(
                syn::token::RArrow::default(),
                Box::new(syn::parse2(return_type)?),
            ),
        },
        block: Box::new(fn_body),
    };

    let component_tokens = crate::component::generate_component(item_fn)?;

    Ok(quote! {
        #component_tokens
    })
}

fn process_dynamic_entries(
    entries: &[(String, TokenStream)],
    class_name: &str,
    span: Span,
    var_decls: &mut Vec<TokenStream>,
    style_bindings: &mut Vec<TokenStream>,
    suffix: &str,
) -> Result<()> {
    let __silex = crate::crate_path::silex();
    for (i, (prop, expr)) in entries.iter().enumerate() {
        let var_ident = quote::format_ident!("dyn_var{}_{}", suffix, i);
        let prop_type = crate::css::get_prop_type(prop, span)?;
        var_decls.push(quote! {
            let #var_ident = #__silex::css::make_property_val::<#prop_type, _>((#expr).clone());
        });
        let var_name = format!("--{}-{}", class_name, i);
        style_bindings.push(quote! { (::std::borrow::Cow::Borrowed(#var_name), #var_ident) });
    }
    Ok(())
}

fn expand_dynamic_rule(
    idx: usize,
    rule: DynamicRule,
    class_name: &str,
    span: Span,
    descriptors: &mut Vec<TokenStream>,
    var_decls: &mut Vec<TokenStream>,
    variant_info: Option<(usize, &Ident, &str)>, // (group_index, sig_ident, name_lower)
) -> Result<()> {
    let __silex = crate::crate_path::silex();
    let parts = crate::css::compiler::template_parts_tokens(&rule.template);
    let mut getters = Vec::new();

    let suffix = if let Some((_, sig, name)) = variant_info {
        format!("_{}_{}", sig, name)
    } else {
        String::new()
    };

    for (expr_idx, (prop, expr)) in rule.expressions.iter().enumerate() {
        let var_id = quote::format_ident!("rule_var{}_{}_{}", suffix, idx, expr_idx);
        let prop_ty = crate::css::get_prop_type(prop, span)?;
        var_decls.push(quote! {
            let #var_id = #__silex::css::make_property_val::<#prop_ty, _>((#expr).clone());
        });
        getters.push(var_id);
    }

    let (variant_group, variant_key) = match variant_info {
        Some((group_index, _, value)) => (quote! { Some(#group_index) }, quote! { Some(#value) }),
        None => (quote! { None }, quote! { None }),
    };
    descriptors.push(quote! {
        #__silex::css::StyledDynamicRule::new(
            #variant_group,
            #variant_key,
            #class_name,
            #parts,
            ::std::vec![ #(#getters),* ],
        )
    });
    Ok(())
}

fn has_scope_lifetime(generics: &Generics) -> bool {
    generics.params.iter().any(|param| {
        matches!(
            param,
            syn::GenericParam::Lifetime(lifetime) if lifetime.lifetime.ident == "scope"
        )
    })
}

fn styled_scope_lifetime(generics: &Generics) -> syn::Lifetime {
    generics
        .params
        .iter()
        .find_map(|param| match param {
            syn::GenericParam::Lifetime(lifetime) if lifetime.lifetime.ident == "scope" => {
                Some(lifetime.lifetime.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| syn::Lifetime::new("'static", Span::call_site()))
}

fn normalize_scoped_type(ty: &mut Type, scope: &syn::Lifetime) {
    let Type::Path(type_path) = ty else { return };

    for segment in &mut type_path.path.segments {
        if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
            for argument in &mut arguments.args {
                if let GenericArgument::Type(inner) = argument {
                    normalize_scoped_type(inner, scope);
                }
            }
        }
    }

    let Some(segment) = type_path.path.segments.last_mut() else {
        return;
    };
    let name = segment.ident.to_string();
    let needs_lifetime = matches!(
        name.as_str(),
        "AnyView" | "Rx" | "Signal" | "ReadSignal" | "RwSignal" | "Memo" | "StoredValue"
    );
    if !needs_lifetime {
        return;
    }

    match &mut segment.arguments {
        PathArguments::None if name == "AnyView" => {
            let arguments: syn::AngleBracketedGenericArguments = syn::parse_quote!(<#scope>);
            segment.arguments = PathArguments::AngleBracketed(arguments);
        }
        PathArguments::AngleBracketed(arguments)
            if !matches!(arguments.args.first(), Some(GenericArgument::Lifetime(_))) =>
        {
            arguments
                .args
                .insert(0, GenericArgument::Lifetime(scope.clone()));
        }
        _ => {}
    }
}

fn get_tag_return_type(
    tag: &str,
    span: Span,
    where_clause: Option<&syn::WhereClause>,
    scope: &syn::Lifetime,
) -> TokenStream {
    let __silex = crate::crate_path::silex();
    if tag.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        let name = match tag {
            "a" => "A".to_string(),
            "data" => "DataTag".to_string(),
            "option" => "OptionTag".to_string(),
            "param" => "ParamTag".to_string(),
            "time" => "TimeTag".to_string(),
            _ => {
                let mut c = tag.chars();
                c.next().unwrap().to_uppercase().collect::<String>() + c.as_str()
            }
        };
        let ident = Ident::new(&name, span);
        quote! { #__silex::dom::element::TypedElement<#scope, #__silex::html::#ident> }
    } else {
        quote! {
            impl #__silex::dom::attribute::AttributeBuilder<#scope>
                + #__silex::dom::view::View<#scope>
                + #__silex::dom::view::ApplyAttributes<#scope>
                + #scope
                #where_clause
        }
    }
}

// --- global! ---

pub struct GlobalStyle {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Option<Ident>,
    pub generics: Generics,
    pub params: Punctuated<FnArg, Token![,]>,
    pub css_block: TokenStream,
    pub is_unsafe: bool,
}

impl Parse for GlobalStyle {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let is_unsafe = input.peek(Token![unsafe]);
        if is_unsafe {
            input.parse::<Token![unsafe]>()?;
        }

        // `body { ... }` is a CSS nested rule, not a named macro declaration.
        // A named global has either visibility, generics, or an argument list,
        // so those tokens provide an unambiguous header lookahead.
        let has_header = input.peek(Token![pub])
            || (input.peek(Ident) && (input.peek2(Token![<]) || input.peek2(syn::token::Paren)));
        let mut vis = Visibility::Inherited;
        let mut name = None;
        let mut generics = Generics::default();
        let mut params = Punctuated::new();
        let css_block;

        if has_header {
            vis = input.parse()?;
            name = Some(input.parse()?);
            if input.peek(Token![<]) {
                generics = input.parse()?;
            }
            if input.peek(syn::token::Paren) {
                let params_content;
                syn::parenthesized!(params_content in input);
                params = params_content.parse_terminated(FnArg::parse, Token![,])?;
            }
            if input.peek(Token![where]) {
                generics.where_clause = Some(input.parse()?);
            }
            let css_content;
            syn::braced!(css_content in input);
            css_block = css_content.parse()?;
        } else {
            css_block = input.parse()?;
        }

        Ok(GlobalStyle {
            attrs,
            vis,
            name,
            generics,
            params,
            css_block,
            is_unsafe,
        })
    }
}

pub fn global_impl(input: TokenStream) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let parsed: GlobalStyle = syn::parse2(input)?;

    let GlobalStyle {
        attrs,
        vis,
        name,
        generics,
        mut params,
        css_block,
        is_unsafe,
    } = parsed;
    let c_name = name.unwrap_or_else(|| quote::format_ident!("GlobalStyles"));
    let block: crate::css::ast::CssBlock = syn::parse2(css_block.clone())?;
    if !has_scope_lifetime(&generics) {
        crate::css::reject_dynamic_global(
            &block,
            c_name.span(),
            "global! 的零参数形式只接受纯静态 CSS；动态内容请改用显式的 `<'scope>` source 参数。",
            "global! 的零参数形式只接受纯静态 CSS；动态选择器请改用显式的 `<'scope>` source 参数。",
        )?;
    }
    let res = CssCompiler::compile_global(css_block, c_name.span(), is_unsafe)?;
    let has_dynamic = !res.expressions.is_empty() || !res.dynamic_rules.is_empty();

    if has_dynamic && params.is_empty() {
        return Err(syn::Error::new(
            c_name.span(),
            "global! 的动态形式必须声明显式 source 参数；宏不会从 CSS 表达式推断函数签名。",
        ));
    }

    if !has_dynamic {
        return generate_static_global(
            &__silex, attrs, vis, c_name, generics, params, is_unsafe, &res,
        );
    }

    let scope = generics
        .params
        .iter()
        .find_map(|param| match param {
            syn::GenericParam::Lifetime(lifetime) if lifetime.lifetime.ident == "scope" => {
                Some(lifetime.lifetime.clone())
            }
            _ => None,
        })
        .expect("dynamic global scope was checked above");

    for arg in &mut params {
        if let FnArg::Typed(arg) = arg {
            normalize_scoped_type(&mut arg.ty, &scope);
        }
    }

    let assertions = crate::css::generate_static_assertions(&res.assertions)?;
    let static_id = &res.static_id;
    let static_css = &res.static_css;
    let style_id = &res.style_id;
    let component_css = &res.component_css;
    let layer = res.layer;
    let mut getter_decls = Vec::new();
    let mut replacement_getters = Vec::new();
    for (index, (property, expression)) in res.expressions.iter().enumerate() {
        let getter = quote::format_ident!("__slx_global_value_{index}");
        let prop_type = crate::css::get_prop_type(property, c_name.span())?;
        let pattern = format!("var(--slx-dyn-{index})");
        getter_decls.push(quote! {
            let #getter = #__silex::css::make_property_val::<#prop_type, _>(#expression);
        });
        replacement_getters.push(quote! {
            (
                ::std::string::String::from(#pattern),
                #getter.clone(),
            )
        });
    }

    let has_dynamic_placeholder = |css: &str| css.contains("var(--slx-dyn-");
    let mut static_styles = Vec::new();
    let mut bindings = Vec::new();

    if !static_css.is_empty() {
        if has_dynamic_placeholder(static_css) {
            bindings.push(quote! {
                #__silex::css::GlobalStyleBinding::new(
                    #static_id,
                    &[#__silex::css::CssPart::Lit(#static_css)],
                    ::std::vec![],
                    ::std::vec![ #(#replacement_getters),* ],
                )
            });
        } else {
            static_styles.push(quote! { (#static_id, #static_css) });
        }
    }

    if !component_css.is_empty() {
        if has_dynamic_placeholder(component_css) {
            bindings.push(quote! {
                #__silex::css::GlobalStyleBinding::new(
                    #style_id,
                    &[#__silex::css::CssPart::Lit(#component_css)],
                    ::std::vec![],
                    ::std::vec![ #(#replacement_getters),* ],
                )
            });
        } else {
            static_styles.push(quote! { (#style_id, #component_css) });
        }
    }

    for (index, rule) in res.dynamic_rules.iter().enumerate() {
        let parts = crate::css::compiler::template_parts_tokens(&rule.template);
        let mut positional = Vec::new();
        for (expression_index, (_, expression)) in rule.expressions.iter().enumerate() {
            let getter = quote::format_ident!("__slx_global_selector_{index}_{expression_index}");
            getter_decls.push(quote! {
                let #getter = #__silex::css::IntoCssReactive::into_css_reactive(#expression)
                    .map(|value| value.to_string());
            });
            positional.push(quote! { #getter.clone() });
        }
        let style_id = format!("{}-dynamic-{}", res.style_id, index);
        bindings.push(quote! {
            #__silex::css::GlobalStyleBinding::new(
                #style_id,
                #parts,
                ::std::vec![ #(#positional),* ],
                ::std::vec![ #(#replacement_getters),* ],
            ).with_layer(#layer)
        });
    }

    let mut fn_generics = generics.clone();
    let where_clause = fn_generics.where_clause.take();

    Ok(quote! {
        #(#attrs)*
        #[allow(non_snake_case, unused_variables)]
        #vis fn #c_name #fn_generics(
            #params
        ) -> #__silex::css::GlobalStyleView<#scope> #where_clause {
            #assertions
            #(#getter_decls)*
            #__silex::css::GlobalStyleView::new(
                ::std::vec![ #(#static_styles),* ],
                ::std::vec![ #(#bindings),* ],
            )
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn generate_static_global(
    __silex: &TokenStream,
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    generics: Generics,
    params: Punctuated<FnArg, Token![,]>,
    is_unsafe: bool,
    res: &crate::css::compiler::CssCompileResult,
) -> Result<TokenStream> {
    let assertions = crate::css::generate_static_assertions(&res.assertions)?;
    let static_inits = res.generate_inits();
    let _ = is_unsafe;
    let mut fn_generics = generics;
    let where_clause = fn_generics.where_clause.take();

    Ok(quote! {
        #(#attrs)*
        #[allow(non_snake_case, unused_variables)]
        #vis fn #name #fn_generics(
            #params
        ) -> impl #__silex::dom::view::View<'static>
            + #__silex::dom::view::ApplyAttributes<'static>
            + 'static
            #where_clause
        {
            #assertions
            #static_inits
            use #__silex::dom::view::View;
            #__silex::dom::view::View::into_any(())
        }
    })
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
            | "circle"
            | "ellipse"
            | "line"
            | "path"
            | "polygon"
            | "polyline"
            | "rect"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "tw")]
    fn test_styled_inline_tailwind_variants() {
        let input = quote::quote! {
            Card<'scope><button>(id: String, children: AnyView<'scope>) {
                padding: 1rem;

                variants: {
                    theme_mode: {
                        light: "bg-white text-slate-900",
                        dark: "bg-slate-800 text-white",
                    }
                }
            }
        };
        let res = styled_impl(input).unwrap();
        let code = res.to_string();
        assert!(code.contains("Card"));
    }

    #[test]
    fn dynamic_styled_styles_are_injected_by_an_owner_bound_attribute() {
        let input = quote::quote! {
            Panel<'scope><div>(
                children: AnyView<'scope>,
                color: Signal<'scope, Hex>,
            ) {
                color: $(color);
            }
        };
        let code = styled_impl(input).unwrap().to_string();
        let custom_pos = code
            .find("custom_with_inputs")
            .expect("dynamic styled output has an owner-bound attribute");
        let inject_pos = code
            .find("inject_style")
            .expect("dynamic styled injects CSS");
        assert!(inject_pos > custom_pos, "{code}");
    }

    #[test]
    fn dynamic_styled_rules_use_one_runtime_binding() {
        let input = quote::quote! {
            Panel<'scope><div>(
                children: AnyView<'scope>,
                selector: Signal<'scope, String>,
            ) {
                $selector { color: red; }
            }
        };
        let code = styled_impl(input).unwrap().to_string();
        assert!(code.contains("StyledVariantBinding"), "{code}");
        assert!(!code.contains("DynamicStyleManager :: new"), "{code}");
    }
}
