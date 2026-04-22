use crate::css::compiler::{CssCompiler, DynamicRule};
use proc_macro2::{Span, TokenStream};
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
    pub is_unsafe: bool,
    pub standalone: Option<usize>,
}

impl Parse for StyledComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        let mut standalone = None;
        attrs.retain(|attr| {
            if attr.path().is_ident("standalone")
                && let syn::Meta::NameValue(nv) = &attr.meta
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit),
                    ..
                }) = &nv.value
                && let Ok(val) = lit.base10_parse::<usize>()
            {
                standalone = Some(val);
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
            standalone,
        })
    }
}

pub fn styled_impl(input: TokenStream) -> Result<TokenStream> {
    let parsed: StyledComponent = syn::parse2(input)?;
    let tag = &parsed.tag;
    let name = &parsed.name;

    let compile_result = CssCompiler::compile(parsed.css_block, tag.span(), parsed.is_unsafe)?;

    let mut var_decls = Vec::new();
    let mut style_bindings = Vec::new();
    let mut dynamic_rule_inits = Vec::new();
    let mut dynamic_rule_classes = Vec::new();

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
            &mut dynamic_rule_inits,
            &mut dynamic_rule_classes,
            None,
        )?;
    }

    // 3. Process Variants
    let mut variant_injections = Vec::new();
    let mut variant_class_bindings = Vec::new();
    let mut prop_sig_bindings = Vec::new();

    for group in &parsed.variants {
        let prop = &group.prop_name;
        let sig_ident = quote::format_ident!("{}_sig", prop);
        prop_sig_bindings.push(quote! {
            let #sig_ident = ::silex::prelude::IntoRx::into_rx(#prop.clone());
        });

        let mut match_arms = Vec::new();
        for (v_name, v_css) in &group.variants {
            let res = CssCompiler::compile(v_css.clone(), v_name.span(), parsed.is_unsafe)?;
            let v_class = res.class_name;
            let v_style_id = res.style_id;
            let v_static_css = res.static_css;
            let v_component_css = res.component_css;

            let v_static_id = res.static_id;
            variant_injections.push(quote! {
                if !#v_static_css.is_empty() {
                    ::silex::css::inject_style(#v_static_id, #v_static_css);
                }
                if !#v_component_css.is_empty() {
                    ::silex::css::inject_style(#v_style_id, #v_component_css);
                }
            });

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
                    &mut dynamic_rule_inits,
                    &mut dynamic_rule_classes,
                    Some((&sig_ident, &v_name.to_string().to_lowercase())),
                )?;
            }

            let v_name_lower = v_name.to_string().to_lowercase();
            match_arms.push(quote! {
                v if ::std::string::ToString::to_string(&v).to_lowercase() == #v_name_lower => #v_class,
            });
        }

        variant_class_bindings.push(quote! {
            .class(::silex::prelude::rx! {
                match #sig_ident.get() {
                    #(#match_arms)*
                    _ => "",
                }
            })
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
            all_fn_args.push(syn::parse_quote! { #[prop(into, default)] #p: ::silex::core::reactivity::Signal<::std::string::String> });
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
    let return_type =
        get_tag_return_type(&tag_str, tag.span(), parsed.generics.where_clause.as_ref());
    let extra_impls = get_extra_tag_impls(&tag_str, name, &parsed.generics);

    let filtered_attrs: Vec<_> = parsed
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("theme"))
        .collect();

    let component_attr = if let Some(n) = parsed.standalone {
        quote! { #[::silex::macros::component(standalone = #n)] }
    } else {
        quote! { #[::silex::macros::component] }
    };
    let vis = &parsed.vis;
    let (impl_generics, _, _) = parsed.generics.split_for_impl();
    let static_css = &compile_result.static_css;
    let component_css = &compile_result.component_css;
    let style_id = &compile_result.style_id;
    let class_name = &compile_result.class_name;
    let static_id = &compile_result.static_id;

    Ok(quote! {
        #(#filtered_attrs)*
        #component_attr
        #vis fn #name #impl_generics (#all_fn_args) -> #return_type {
            const __STATIC_CSS: &str = #static_css;
            const __COMPONENT_CSS: &str = #component_css;

            #(#var_decls)*
            #(#prop_sig_bindings)*

            if !__STATIC_CSS.is_empty() {
                ::silex::css::inject_style(#static_id, __STATIC_CSS);
            }
            if !__COMPONENT_CSS.is_empty() {
                ::silex::css::inject_style(#style_id, __COMPONENT_CSS);
            }

            #(#variant_injections)*
            #(#dynamic_rule_inits)*

            ::silex::html::#tag(#children_binding)
                .class(#class_name)
                #style_prop_binding
                .apply(::silex::dom::attribute::AttrOp::CombinedStyles {
                    statics: ::std::vec![],
                    properties: ::std::vec![ #(#style_bindings),* ],
                    sheets: ::std::vec![],
                })
                #(#variant_class_bindings)*
                #(#dynamic_rule_classes)*
        }
        #extra_impls
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
    for (i, (prop, expr)) in entries.iter().enumerate() {
        let var_ident = quote::format_ident!("dyn_var{}_{}", suffix, i);
        let prop_type = crate::css::get_prop_type(prop, span)?;
        var_decls.push(quote! {
            let #var_ident = ::silex::css::make_dynamic_val_for::<#prop_type, _>((#expr).clone());
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
    inits: &mut Vec<TokenStream>,
    classes: &mut Vec<TokenStream>,
    variant_info: Option<(&Ident, &str)>, // (sig_ident, name_lower)
) -> Result<()> {
    let template = &rule.template;
    let mut eval_vars = Vec::new();
    let mut rule_var_decls = Vec::new();

    let suffix = if let Some((sig, name)) = variant_info {
        format!("_{}_{}", sig, name)
    } else {
        String::new()
    };

    for (expr_idx, (prop, expr)) in rule.expressions.iter().enumerate() {
        let var_id = quote::format_ident!("rule_var{}_{}_{}", suffix, idx, expr_idx);
        let prop_ty = crate::css::get_prop_type(prop, span)?;
        rule_var_decls.push(
            quote! { let #var_id = ::silex::css::make_dynamic_val_for::<#prop_ty, _>(#expr); },
        );
        eval_vars.push(var_id);
    }

    let mgr_id = quote::format_ident!("mgr{}_{}", suffix, idx);
    inits.push(quote! {
        #(#rule_var_decls)*
        let #mgr_id = ::std::rc::Rc::new(::std::cell::RefCell::new(Some(::silex::css::DynamicStyleManager::new())));
        let cleanup = #mgr_id.clone();
        ::silex::core::reactivity::on_cleanup(move || { if let Ok(mut o) = cleanup.try_borrow_mut() { o.take(); } });
    });

    let rx_body = if let Some((sig, val)) = variant_info {
        quote! {
            if #sig.get() != #val { return "".to_string(); }
            let mut res = ::std::string::ToString::to_string(#template);
            #( if let Some(p) = res.find("{}") { res.replace_range(p..p+2, &#eval_vars.get()); } )*
            let hash = ::silex::hash::css::hash_one(&res);
            let mut buf = [0u8; 13];
            let dyn_class = format!("{}-dyn-{}", #class_name, ::silex::hash::css::encode_base36(hash, &mut buf));
            if let Ok(mut o) = #mgr_id.try_borrow_mut() {
                if let Some(m) = o.as_mut() { m.update(&dyn_class, &res.replace(#class_name, &dyn_class)); }
            }
            dyn_class
        }
    } else {
        quote! {
            let mut res = ::std::string::ToString::to_string(#template);
            #( if let Some(p) = res.find("{}") { res.replace_range(p..p+2, &#eval_vars.get()); } )*
            let hash = ::silex::hash::css::hash_one(&res);
            let mut buf = [0u8; 13];
            let dyn_class = format!("{}-dyn-{}", #class_name, ::silex::hash::css::encode_base36(hash, &mut buf));
            if let Ok(mut o) = #mgr_id.try_borrow_mut() {
                if let Some(m) = o.as_mut() { m.update(&dyn_class, &res.replace(#class_name, &dyn_class)); }
            }
            dyn_class
        }
    };

    classes.push(
        quote! { .class({ let manager = #mgr_id.clone(); ::silex::prelude::rx! { #rx_body } }) },
    );
    Ok(())
}

fn get_tag_return_type(
    tag: &str,
    span: Span,
    where_clause: Option<&syn::WhereClause>,
) -> TokenStream {
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
        quote! { ::silex::dom::element::TypedElement<::silex::dom::element::tags::#ident> }
    } else {
        quote! { impl ::silex::dom::attribute::AttributeBuilder + ::silex::dom::view::Mount + ::silex::dom::view::MountRef + ::silex::dom::view::ApplyAttributes #where_clause }
    }
}

fn get_extra_tag_impls(tag: &str, name: &Ident, generics: &Generics) -> TokenStream {
    let mut items = TokenStream::new();
    let comp = quote::format_ident!("{}Component", name);
    let (impl_gen, ty_gen, where_c) = generics.split_for_impl();

    let traits = match tag {
        "button" | "input" | "form" | "select" | "textarea" | "option" | "optgroup"
        | "fieldset" => vec!["FormAttributes"],
        "a" | "area" | "link" => vec!["AnchorAttributes"],
        "label" => vec!["LabelAttributes"],
        "img" | "video" | "audio" | "source" | "iframe" | "embed" | "object" => {
            vec!["MediaAttributes"]
        }
        "dialog" | "details" => vec!["OpenAttributes"],
        "td" | "th" => {
            if tag == "th" {
                vec!["TableCellAttributes", "TableHeaderAttributes"]
            } else {
                vec!["TableCellAttributes"]
            }
        }
        _ => vec![],
    };

    for t in traits {
        let tid = quote::format_ident!("{}", t);
        items.extend(quote! { impl #impl_gen ::silex::html::#tid for #comp #ty_gen #where_c {} });
    }
    items
}

// --- global! ---

pub struct GlobalStyle {
    pub attrs: Vec<Attribute>,
    pub name: Option<Ident>,
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

        if input.peek(Ident) && input.peek2(syn::token::Brace) {
            let name = input.parse()?;
            let content;
            syn::braced!(content in input);
            return Ok(GlobalStyle {
                attrs,
                name: Some(name),
                css_block: content.parse()?,
                is_unsafe,
            });
        }
        Ok(GlobalStyle {
            attrs,
            name: None,
            css_block: input.parse()?,
            is_unsafe,
        })
    }
}

pub fn global_impl(input: TokenStream) -> Result<TokenStream> {
    let parsed: GlobalStyle = syn::parse2(input)?;

    let filtered_attrs: Vec<_> = parsed.attrs.iter().collect();

    let component_attr = quote! { #[::silex::macros::component] };

    let c_name = parsed
        .name
        .unwrap_or_else(|| quote::format_ident!("GlobalStyles"));
    let res = CssCompiler::compile_global(parsed.css_block, c_name.span(), parsed.is_unsafe)?;

    let mut inits = Vec::new();
    let mut logics = Vec::new();

    // 1. Process top-level expressions using template replacement (eliminates --dyn-N proxy)
    if !res.expressions.is_empty() {
        let mut evals = Vec::new();
        let mut r_decls = Vec::new();
        let mut idxs = Vec::new();
        for (i, (prop, expr)) in res.expressions.iter().enumerate() {
            let vid = quote::format_ident!("global_var_{}", i);
            let pty = crate::css::get_prop_type(prop, c_name.span())?;
            r_decls
                .push(quote! { let #vid = ::silex::css::make_dynamic_val_for::<#pty, _>(#expr); });
            evals.push(vid);
            idxs.push(i);
        }

        // Combine static (at-rules) and component (rules) CSS into one template
        let template = format!("{}\n{}", res.static_css, res.component_css);
        let mid = quote::format_ident!("global_root_manager");
        let sid = &res.style_id;

        inits.push(quote! {
            #(#r_decls)*
            let #mid = ::std::rc::Rc::new(::std::cell::RefCell::new(Some(::silex::css::DynamicStyleManager::new())));
            let cleanup = #mid.clone();
            ::silex::core::reactivity::on_cleanup(move || { if let Ok(mut o) = cleanup.try_borrow_mut() { o.take(); } });
        });

        logics.push(quote! {{
            let manager = #mid.clone();
            ::silex::prelude::Effect::new(move |_| {
                let mut res = ::std::string::ToString::to_string(#template);
                #(
                    let pid = format!("var(--slx-dyn-{})", #idxs);
                    res = res.replace(&pid, &#evals.get());
                )*
                if let Ok(mut o) = manager.try_borrow_mut() { if let Some(m) = o.as_mut() { m.update(#sid, &res); } }
            });
        }});
    } else {
        // Purely static injection
        let s_css = &res.static_css;
        let c_css = &res.component_css;
        let sid = &res.style_id;
        let static_id = &res.static_id;

        if !s_css.is_empty() {
            inits.push(quote! { ::silex::css::inject_style(#static_id, #s_css); });
        }
        if !c_css.is_empty() {
            inits.push(quote! { ::silex::css::inject_style(#sid, #c_css); });
        }
    }

    // 2. Process nested dynamic rules (selectors with $)
    for (idx, rule) in res.dynamic_rules.iter().enumerate() {
        let template = &rule.template;
        let mut evals = Vec::new();
        let mut r_decls = Vec::new();
        let mut e_idxs = Vec::new();
        for (ei, (p, ex)) in rule.expressions.iter().enumerate() {
            let vid = quote::format_ident!("dyn_var_{}_{}", idx, ei);
            let pty = crate::css::get_prop_type(p, c_name.span())?;
            r_decls.push(quote! { let #vid = ::silex::css::make_dynamic_val_for::<#pty, _>(#ex); });
            evals.push(vid);
            e_idxs.push(ei);
        }
        let mid = quote::format_ident!("manager_{}", idx);
        inits.push(quote! {
            #(#r_decls)*
            let #mid = ::std::rc::Rc::new(::std::cell::RefCell::new(Some(::silex::css::DynamicStyleManager::new())));
            let cleanup = #mid.clone();
            ::silex::core::reactivity::on_cleanup(move || { if let Ok(mut o) = cleanup.try_borrow_mut() { o.take(); } });
        });
        let sid = &res.style_id;
        logics.push(quote! {{
            let manager = #mid.clone();
            ::silex::prelude::Effect::new(move |_| {
                let mut res = ::std::string::ToString::to_string(#template);
                #(
                    let pid_val = format!("var(--slx-dyn-{})", #e_idxs);
                    let pid_sel = format!("._slx_dyn_{}", #e_idxs);
                    res = res.replace(&pid_val, &#evals.get());
                    res = res.replace(&pid_sel, &#evals.get());
                )*
                let rid = format!("{}-dyn-{}", #sid, #idx);
                if let Ok(mut o) = manager.try_borrow_mut() { if let Some(m) = o.as_mut() { m.update(&rid, &res); } }
            });
        }});
    }

    Ok(quote! {
        #(#filtered_attrs)*
        #component_attr
        pub fn #c_name() -> impl ::silex::dom::view::Mount + ::silex::dom::view::MountRef + ::silex::dom::view::ApplyAttributes {
            #(#inits)*
            #(#logics)*
            use ::silex::dom::view::View;
            ::silex::dom::view::View::into_any(())
        }
    })
}
