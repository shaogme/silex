use crate::css::compiler::{CssCompileResult, CssCompiler, DynamicRule};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{
    Attribute, FnArg, GenericArgument, Generics, Ident, Pat, PathArguments, Result, Token, Type,
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
    let explicit_error_handler = find_error_handler_parameter(&parsed.props).ok_or_else(|| {
        syn::Error::new(
            name.span(),
            "styled! components must declare an explicit error_handler parameter",
        )
    })?;
    let dynamic_error_handler = quote! { #explicit_error_handler };

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
    let mut static_value_decls = Vec::new();
    let mut static_value_inits = Vec::new();
    let (base_static_values_ident, base_static_values_tokens) =
        if compile_result.static_expressions.is_empty() {
            (None, None)
        } else {
            let (decls, values) = crate::css::generate_static_value_bindings(
                &compile_result.static_expressions,
                tag.span(),
                "__slx_styled_base_static",
            )?;
            static_value_decls.push(decls);
            (
                Some(quote::format_ident!("__slx_styled_base_values")),
                Some(crate::css::static_values_tokens(&values)),
            )
        };
    let base_style_inits = if let Some(values) = &base_static_values_ident {
        crate::css::generate_static_style_inits(&compile_result, Some(values))
    } else {
        compile_result.generate_inits()
    };
    if let (Some(ident), Some(values)) = (&base_static_values_ident, &base_static_values_tokens) {
        static_value_inits.push(quote! {
            let #ident: ::std::vec::Vec<::std::string::String> = #values;
        });
    }
    let mut deferred_style_descriptors = Vec::new();
    let mut deferred_style_templates = Vec::new();
    append_style_descriptors(
        &compile_result,
        base_static_values_ident.as_ref(),
        &mut deferred_style_descriptors,
        &mut deferred_style_templates,
    );
    let layer = compile_result.layer;

    let mut var_decls = Vec::new();
    let mut style_bindings = Vec::new();
    let mut style_getters = Vec::new();
    let mut dynamic_rule_descriptors = Vec::new();

    // 1. Process base dynamic values
    process_dynamic_entries(DynamicEntryExpansion {
        entries: &compile_result.expressions,
        class_name: &compile_result.class_name,
        span: tag.span(),
        var_decls: &mut var_decls,
        style_bindings: &mut style_bindings,
        style_getters: &mut style_getters,
        suffix: "",
        error_handler: dynamic_error_handler.clone(),
    })?;

    // 2. Process base dynamic rules
    for (idx, rule) in compile_result.dynamic_rules.into_iter().enumerate() {
        expand_dynamic_rule(DynamicRuleExpansion {
            idx,
            rule,
            class_name: &compile_result.class_name,
            span: tag.span(),
            descriptors: &mut dynamic_rule_descriptors,
            var_decls: &mut var_decls,
            variant_info: None,
            static_values: base_static_values_ident.as_ref(),
            error_handler: dynamic_error_handler.clone(),
        })?;
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
            let (variant_static_values_ident, variant_static_values_tokens) =
                if res.static_expressions.is_empty() {
                    (None, None)
                } else {
                    let (decls, values) = crate::css::generate_static_value_bindings(
                        &res.static_expressions,
                        v_name.span(),
                        &format!("__slx_styled_{}_{}_static", prop, v_name),
                    )?;
                    static_value_decls.push(decls);
                    (
                        Some(quote::format_ident!(
                            "__slx_styled_{}_{}_values",
                            prop,
                            v_name
                        )),
                        Some(crate::css::static_values_tokens(&values)),
                    )
                };
            if let (Some(ident), Some(values)) =
                (&variant_static_values_ident, &variant_static_values_tokens)
            {
                static_value_inits.push(quote! {
                    let #ident: ::std::vec::Vec<::std::string::String> = #values;
                });
            }
            variant_injections.push(if let Some(values) = &variant_static_values_ident {
                crate::css::generate_static_style_inits(&res, Some(values))
            } else {
                res.generate_inits()
            });
            append_style_descriptors(
                &res,
                variant_static_values_ident.as_ref(),
                &mut deferred_style_descriptors,
                &mut deferred_style_templates,
            );

            process_dynamic_entries(DynamicEntryExpansion {
                entries: &res.expressions,
                class_name: &v_class,
                span: v_name.span(),
                var_decls: &mut var_decls,
                style_bindings: &mut style_bindings,
                style_getters: &mut style_getters,
                suffix: &format!("_{}_{}", prop, v_name),
                error_handler: dynamic_error_handler.clone(),
            })?;

            for (idx, rule) in res.dynamic_rules.into_iter().enumerate() {
                expand_dynamic_rule(DynamicRuleExpansion {
                    idx,
                    rule,
                    class_name: &v_class,
                    span: v_name.span(),
                    descriptors: &mut dynamic_rule_descriptors,
                    var_decls: &mut var_decls,
                    variant_info: Some(DynamicVariantInfo {
                        group_index,
                        signature: &sig_ident,
                        name_lower: v_name.to_string().to_lowercase(),
                    }),
                    static_values: variant_static_values_ident.as_ref(),
                    error_handler: dynamic_error_handler.clone(),
                })?;
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

    let needs_result = !var_decls.is_empty();
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
    let styled_variant_binding = if !has_dynamic_bindings {
        quote! {}
    } else {
        quote! {
            .apply(#__silex::css::StyledVariantBinding::new(
                #layer,
                ::std::vec![ #(#dynamic_rule_descriptors),* ],
                ::std::vec![ #(#variant_group_descriptors),* ],
            )
            .with_static_styles(
                ::std::vec![ #(#deferred_style_descriptors),* ],
                ::std::vec![ #(#style_getters.clone()),* ],
            )
            .with_static_templates(::std::vec![ #(#deferred_style_templates),* ])
            .into_op())
        }
    };

    let node_init = if is_void_tag(&tag_str) {
        quote! { #__silex::html::#tag() }
    } else {
        quote! { #__silex::html::#tag(#children_binding) }
    };

    let node_view = quote! {
        #node_init
            .class(#class_name)
            #style_prop_binding
            .apply(#__silex::dom::attribute::AttrOp::CombinedStyles(#__silex::dom::attribute::CombinedStyles {
                statics: ::std::vec![],
                properties: ::std::vec![ #(#style_bindings),* ],
                sheets: ::std::vec![],
            }))
            #styled_variant_binding
    };

    let node_return = if needs_result {
        quote! { ::core::result::Result::Ok(#node_view) }
    } else {
        node_view.clone()
    };

    let fn_body: syn::Block = syn::parse_quote! {
        {
            #assertions

            #(#static_value_decls)*
            #(#static_value_inits)*
            #(#var_decls)*
            #(#prop_sig_bindings)*

            #immediate_style_inits

            #node_return
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
                Box::new(if needs_result {
                    syn::parse_quote!(#__silex::core::SilexResult<#return_type>)
                } else {
                    syn::parse2(return_type)?
                }),
            ),
        },
        block: Box::new(fn_body),
    };

    let component_tokens = crate::component::generate_component(item_fn)?;

    Ok(quote! {
        #component_tokens
    })
}

struct DynamicEntryExpansion<'a> {
    entries: &'a [(String, TokenStream)],
    class_name: &'a str,
    span: Span,
    var_decls: &'a mut Vec<TokenStream>,
    style_bindings: &'a mut Vec<TokenStream>,
    style_getters: &'a mut Vec<Ident>,
    suffix: &'a str,
    error_handler: TokenStream,
}

fn process_dynamic_entries(input: DynamicEntryExpansion<'_>) -> Result<()> {
    let DynamicEntryExpansion {
        entries,
        class_name,
        span,
        var_decls,
        style_bindings,
        style_getters,
        suffix,
        error_handler,
    } = input;
    let __silex = crate::crate_path::silex();
    for (i, (prop, expr)) in entries.iter().enumerate() {
        let var_ident = quote::format_ident!("dyn_var{}_{}", suffix, i);
        let prop_type = crate::css::get_prop_type(prop, span)?;
        var_decls.push(quote! {
            let #var_ident = #__silex::css::make_property_val::<#prop_type, _>(
                (#expr).clone(),
                #error_handler,
            )?;
        });
        style_getters.push(var_ident.clone());
        let var_name = format!("--{}-{}", class_name, i);
        style_bindings.push(quote! { (::std::borrow::Cow::Borrowed(#var_name), #var_ident) });
    }
    Ok(())
}

fn append_style_descriptors(
    result: &crate::css::compiler::CssCompileResult,
    static_values: Option<&Ident>,
    descriptors: &mut Vec<TokenStream>,
    templates: &mut Vec<TokenStream>,
) {
    let __silex = crate::crate_path::silex();
    if !result.static_css.is_empty() {
        let style_id = &result.static_id;
        let css = &result.static_css;
        if let Some(values) = static_values {
            templates.push(quote! {
                #__silex::css::StaticStyleTemplate::new(
                    #style_id,
                    #css,
                    #values.clone(),
                )
            });
        } else {
            descriptors.push(quote! { (#style_id, #css) });
        }
    }
    if !result.component_css.is_empty() {
        let style_id = &result.style_id;
        let css = &result.component_css;
        if let Some(values) = static_values {
            templates.push(quote! {
                #__silex::css::StaticStyleTemplate::new(
                    #style_id,
                    #css,
                    #values.clone(),
                )
            });
        } else {
            descriptors.push(quote! { (#style_id, #css) });
        }
    }
}

struct DynamicVariantInfo<'a> {
    group_index: usize,
    signature: &'a Ident,
    name_lower: String,
}

struct DynamicRuleExpansion<'a> {
    idx: usize,
    rule: DynamicRule,
    class_name: &'a str,
    span: Span,
    descriptors: &'a mut Vec<TokenStream>,
    var_decls: &'a mut Vec<TokenStream>,
    variant_info: Option<DynamicVariantInfo<'a>>,
    static_values: Option<&'a Ident>,
    error_handler: TokenStream,
}

fn expand_dynamic_rule(input: DynamicRuleExpansion<'_>) -> Result<()> {
    let DynamicRuleExpansion {
        idx,
        rule,
        class_name,
        span,
        descriptors,
        var_decls,
        variant_info,
        static_values,
        error_handler,
    } = input;
    let __silex = crate::crate_path::silex();
    let parts = crate::css::compiler::template_parts_tokens(&rule.template);
    let mut getters = Vec::new();

    let suffix = if let Some(info) = variant_info.as_ref() {
        format!("_{}_{}", info.signature, info.name_lower)
    } else {
        String::new()
    };

    for (expr_idx, (prop, expr)) in rule.expressions.iter().enumerate() {
        let var_id = quote::format_ident!("rule_var{}_{}_{}", suffix, idx, expr_idx);
        let prop_ty = crate::css::get_prop_type(prop, span)?;
        var_decls.push(quote! {
            let #var_id = #__silex::css::make_property_val::<#prop_ty, _>(
                (#expr).clone(),
                #error_handler,
            )?;
        });
        getters.push(var_id);
    }

    let (variant_group, variant_key) = match variant_info {
        Some(info) => {
            let group_index = info.group_index;
            let name_lower = info.name_lower;
            (quote! { Some(#group_index) }, quote! { Some(#name_lower) })
        }
        None => (quote! { None }, quote! { None }),
    };
    let static_value_call = match static_values {
        Some(values) => quote! { .with_static_values(#values.clone()) },
        None => quote! {},
    };
    descriptors.push(quote! {
        #__silex::css::StyledDynamicRule::new(
            #variant_group,
            #variant_key,
            #class_name,
            #parts,
            ::std::vec![ #(#getters),* ],
        )
        #static_value_call
    });
    Ok(())
}

fn has_scope_lifetime(generics: &Generics) -> bool {
    scope_lifetime(generics).is_some()
}

fn scope_lifetime(generics: &Generics) -> Option<syn::Lifetime> {
    generics.params.iter().find_map(|param| match param {
        syn::GenericParam::Lifetime(lifetime) if lifetime.lifetime.ident == "scope" => {
            Some(lifetime.lifetime.clone())
        }
        _ => None,
    })
}

struct ScopeLifetimeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ScopeLifetimeVisitor {
    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if lifetime.ident == "scope" {
            self.found = true;
        }
    }
}

fn has_scope_parameter(params: &Punctuated<FnArg, Token![,]>) -> bool {
    params.iter().any(|param| {
        let FnArg::Typed(arg) = param else {
            return false;
        };
        let mut visitor = ScopeLifetimeVisitor { found: false };
        visitor.visit_type(&arg.ty);
        visitor.found
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
    pub name: Ident,
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

        let vis: Visibility = input.parse()?;
        if matches!(vis, Visibility::Inherited) {
            return Err(input.error(
                "global! requires an explicit visibility and name; use `pub Name { ... }`",
            ));
        }
        let name: Ident = input.parse()?;
        let mut generics = Generics::default();
        let mut params = Punctuated::new();
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
        let css_block = css_content.parse()?;

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
    let c_name = name;
    let scope = scope_lifetime(&generics).ok_or_else(|| {
        syn::Error::new(
            c_name.span(),
            "global! requires an explicit `<'scope>` lifetime parameter.",
        )
    })?;
    if !has_scope_parameter(&params) {
        return Err(syn::Error::new(
            c_name.span(),
            "global! requires an explicit parameter bound to 'scope; use `scope: Scope<'scope>` or a scoped source parameter.",
        ));
    }
    let res = CssCompiler::compile_global(css_block, c_name.span(), is_unsafe)?;
    let has_dynamic = !res.expressions.is_empty() || !res.dynamic_rules.is_empty();

    if !has_dynamic {
        return generate_static_global(StaticGlobalExpansion {
            silex: &__silex,
            attrs,
            vis,
            name: c_name,
            generics,
            params,
            scope,
            result: &res,
        });
    }

    let error_handler = find_error_handler_parameter(&params).ok_or_else(|| {
        syn::Error::new(
            c_name.span(),
            "global! 的动态 CSS 必须声明显式 ErrorReporter 参数",
        )
    })?;

    for arg in &mut params {
        if let FnArg::Typed(arg) = arg {
            normalize_scoped_type(&mut arg.ty, &scope);
        }
    }

    let assertions = crate::css::generate_static_assertions(&res.assertions)?;
    let (static_value_decls, static_value_ids) = crate::css::generate_static_value_bindings(
        &res.static_expressions,
        c_name.span(),
        "__slx_global_static",
    )?;
    let static_values_ident = quote::format_ident!("__slx_global_static_values");
    let static_value_tokens = crate::css::static_values_tokens(&static_value_ids);
    let static_replacement_tokens = static_value_ids.iter().enumerate().map(|(index, value)| {
        let pattern = format!("var(--slx-static-{index})");
        quote! {
            (::std::string::String::from(#pattern), #value.to_string())
        }
    });
    let static_binding_methods = if static_value_ids.is_empty() {
        quote! {}
    } else {
        quote! {
            .with_static_values(#static_values_ident.clone())
            .with_static_replacements(::std::vec![ #(#static_replacement_tokens),* ])
        }
    };
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
            let #getter = #__silex::css::make_property_val::<#prop_type, _>(
                #expression,
                #error_handler,
            )?;
        });
        replacement_getters.push(quote! {
            (
                ::std::string::String::from(#pattern),
                #getter.clone(),
            )
        });
    }

    let has_dynamic_placeholder =
        |css: &str| css.contains("var(--slx-dyn-") || css.contains("var(--slx-static-");
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
                ) #static_binding_methods
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
                ) #static_binding_methods
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
            let source =
                quote::format_ident!("__slx_global_selector_source_{index}_{expression_index}");
            getter_decls.push(quote! {
                let #source = #__silex::css::IntoCssReactive::into_css_reactive(#expression);
                let #getter = #source.map(|value| value.to_string(), #error_handler)?;
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
            ).with_layer(#layer) #static_binding_methods
        });
    }

    let mut fn_generics = generics.clone();
    let where_clause = fn_generics.where_clause.take();

    Ok(quote! {
        #(#attrs)*
        #[allow(non_snake_case, unused_variables)]
        #vis fn #c_name #fn_generics(
            #params
        ) -> #__silex::core::SilexResult<impl #__silex::dom::view::View<#scope>
            + #__silex::dom::view::ApplyAttributes<#scope>
            + #scope>
            #where_clause
        {
            #assertions
            #static_value_decls
            let #static_values_ident: ::std::vec::Vec<::std::string::String> =
                #static_value_tokens;
            #(#getter_decls)*
            Ok(#__silex::css::GlobalStyleView::new(
                ::std::vec![ #(#static_styles),* ],
                ::std::vec![ #(#bindings),* ],
            ))
        }
    })
}

fn find_error_handler_parameter(params: &Punctuated<FnArg, Token![,]>) -> Option<Ident> {
    params.iter().find_map(|param| {
        let FnArg::Typed(arg) = param else {
            return None;
        };
        let Pat::Ident(pattern) = arg.pat.as_ref() else {
            return None;
        };
        let Type::Path(type_path) = arg.ty.as_ref() else {
            return None;
        };
        let is_error_handler = type_path.path.segments.last().is_some_and(|segment| {
            segment.ident == "ErrorReporter" || segment.ident == "ErrorHandler"
        });
        is_error_handler.then(|| pattern.ident.clone())
    })
}

struct StaticGlobalExpansion<'a> {
    silex: &'a TokenStream,
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    generics: Generics,
    params: Punctuated<FnArg, Token![,]>,
    scope: syn::Lifetime,
    result: &'a CssCompileResult,
}

fn generate_static_global(input: StaticGlobalExpansion<'_>) -> Result<TokenStream> {
    let StaticGlobalExpansion {
        silex: __silex,
        attrs,
        vis,
        name,
        generics,
        params,
        scope,
        result: res,
    } = input;
    let assertions = crate::css::generate_static_assertions(&res.assertions)?;
    let static_values_ident = quote::format_ident!("__slx_global_static_values");
    let (static_value_decls, static_value_ids) = crate::css::generate_static_value_bindings(
        &res.static_expressions,
        name.span(),
        "__slx_global_static",
    )?;
    let static_value_tokens = crate::css::static_values_tokens(&static_value_ids);
    let static_inits = if res.static_expressions.is_empty() {
        res.generate_inits()
    } else {
        crate::css::generate_static_style_inits(res, Some(&static_values_ident))
    };
    let mut fn_generics = generics;
    let where_clause = fn_generics.where_clause.take();

    Ok(quote! {
        #(#attrs)*
        #[allow(non_snake_case, unused_variables)]
        #vis fn #name #fn_generics(
            #params
        ) -> impl #__silex::dom::view::View<#scope>
            + #__silex::dom::view::ApplyAttributes<#scope>
            + #scope
            #where_clause
        {
            #assertions
            #static_value_decls
            let #static_values_ident: ::std::vec::Vec<::std::string::String> =
                #static_value_tokens;
            #static_inits
            use #__silex::dom::view::ViewFactory;
            #__silex::dom::view::ViewFactory::into_any(())
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
            Card<'scope><button>(
                error_handler: ErrorReporter<'scope>,
                id: String,
                children: AnyView<'scope>,
            ) {
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
                error_handler: ErrorReporter<'scope>,
                children: AnyView<'scope>,
                color: Signal<'scope, Hex>,
            ) {
                color: $(color);
            }
        };
        let code = styled_impl(input).unwrap().to_string();
        assert!(code.contains("StyledVariantBinding"), "{code}");
        assert!(code.contains("with_static_styles"), "{code}");
        assert!(code.contains("dyn_var_0"), "{code}");
        assert!(!code.contains("inject_style"), "{code}");
    }

    #[test]
    fn dynamic_styled_styles_use_explicit_error_handler_without_owner() {
        let input = quote::quote! {
            Panel<'scope><div>(
                children: AnyView<'scope>,
                error_handler: ErrorReporter<'scope>,
                color: Signal<'scope, Hex>,
            ) {
                color: $(color);
            }
        };
        let code = styled_impl(input).unwrap().to_string();
        assert!(code.contains("error_handler"), "{code}");
        assert!(!code.contains("__silex_owner"), "{code}");
    }

    #[test]
    fn dynamic_styled_rules_use_one_runtime_binding() {
        let input = quote::quote! {
            Panel<'scope><div>(
                error_handler: ErrorReporter<'scope>,
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

    #[test]
    fn mixed_global_styles_use_static_replacements() {
        let input = quote::quote! {
            pub Global<'scope>(
                error_handler: ErrorReporter<'scope>,
                color: Signal<'scope, Hex>,
            ) {
                body {
                    color: $(static AppTheme::PRIMARY);
                    border-color: $(color);
                }
            }
        };
        let code = global_impl(input).unwrap().to_string();
        assert!(code.contains("with_static_replacements"), "{code}");
        assert!(code.contains("with_static_values"), "{code}");
    }

    #[test]
    fn global_requires_explicit_name() {
        let input = quote::quote! {
            body {
                body { color: red; }
            }
        };
        let error = match syn::parse2::<GlobalStyle>(input) {
            Ok(_) => panic!("an unnamed global must be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("explicit visibility and name"));
    }

    #[test]
    fn explicit_visibility_and_name_are_preserved() {
        let input = quote::quote! {
            pub(crate) GlobalStyles<'scope>(scope: Scope<'scope>) {
                body { color: red; }
            }
        };
        let parsed: GlobalStyle = syn::parse2(input.clone()).unwrap();

        assert_eq!(parsed.name, "GlobalStyles");
        assert!(matches!(parsed.vis, Visibility::Restricted(_)));

        let code = global_impl(input).unwrap().to_string();
        assert!(code.contains("GlobalStyles"), "{code}");
        assert!(code.contains("pub (crate) fn"), "{code}");
        assert!(code.contains("'scope"), "{code}");
        assert!(!code.contains("'static"), "{code}");
    }

    #[test]
    fn global_requires_scope_lifetime_and_parameter_binding() {
        let missing_lifetime = quote::quote! {
            pub Global(scope: Scope<'scope>) {
                body { color: red; }
            }
        };
        let error = match global_impl(missing_lifetime) {
            Ok(_) => panic!("a global without an explicit scope lifetime must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("explicit `<'scope>` lifetime"));

        let missing_parameter = quote::quote! {
            pub Global<'scope> {
                body { color: red; }
            }
        };
        let error = match global_impl(missing_parameter) {
            Ok(_) => panic!("a global without a scoped parameter must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("parameter bound to 'scope"));
    }
}
