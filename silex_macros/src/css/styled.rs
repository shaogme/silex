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

fn token_stream_to_css(ts: proc_macro2::TokenStream) -> String {
    let mut out = String::new();
    let mut prev_tt: Option<proc_macro2::TokenTree> = None;

    for tt in ts {
        let mut space_before = false;

        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (proc_macro2::TokenTree::Ident(_), proc_macro2::TokenTree::Ident(_)) => {
                    space_before = true
                }
                (proc_macro2::TokenTree::Ident(_), proc_macro2::TokenTree::Literal(_)) => {
                    space_before = true
                }
                (proc_macro2::TokenTree::Literal(_), proc_macro2::TokenTree::Ident(_)) => {
                    space_before = true
                }
                (proc_macro2::TokenTree::Literal(_), proc_macro2::TokenTree::Literal(_)) => {
                    space_before = true
                }
                _ => {}
            }
        }

        if space_before {
            out.push(' ');
        }

        match &tt {
            proc_macro2::TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => ('(', ')'),
                    proc_macro2::Delimiter::Brace => ('{', '}'),
                    proc_macro2::Delimiter::Bracket => ('[', ']'),
                    proc_macro2::Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                out.push_str(&token_stream_to_css(g.stream()));
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
            }
            proc_macro2::TokenTree::Punct(p) => {
                out.push(p.as_char());
            }
            proc_macro2::TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
            }
            proc_macro2::TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
            }
        }
        prev_tt = Some(tt);
    }
    out
}

pub fn styled_impl(input: TokenStream) -> Result<TokenStream> {
    let parsed: StyledComponent = syn::parse2(input)?;

    let vis = parsed.vis;
    let name = parsed.name;
    let tag = parsed.tag;
    let props = parsed.props;

    let css_str = token_stream_to_css(parsed.css_block);
    let css_str = css_str.replace("$(", "$ (");
    let css_str = css_str.replace("$ (", "$(");

    let compile_result = CssCompiler::compile(&css_str, tag.span())?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let final_css = compile_result.final_css;
    let expressions = compile_result.expressions;
    let hash = compile_result.hash;

    let var_decls: Vec<TokenStream> = expressions
        .iter()
        .enumerate()
        .map(|(i, expr_ts)| {
            let var_ident = quote::format_ident!("var_{}", i);
            if let Ok(expr) = syn::parse2::<syn::Expr>(expr_ts.clone()) {
                match &expr {
                    syn::Expr::Path(path) if path.path.get_ident().is_some() => {
                        quote! { let #var_ident = #expr; }
                    }
                    _ => {
                        quote! { let #var_ident = ::silex::rx!(#expr); }
                    }
                }
            } else {
                quote! { let #var_ident = ::silex::rx!(#expr_ts); }
            }
        })
        .collect();

    let style_bindings: Vec<TokenStream> = expressions
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let var_name = format!("--slx-{:x}-{}", hash, i);
            let var_ident = quote::format_ident!("var_{}", i);
            quote! {
                .style((#var_name, #var_ident))
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
            let css_str = token_stream_to_css(variant_css.clone());
            let css_str = css_str.replace("$(", "$ (");
            let css_str = css_str.replace("$ (", "$(");

            let compile_result = CssCompiler::compile(&css_str, variant_name.span())?;
            if !compile_result.expressions.is_empty() {
                return Err(syn::Error::new(
                    variant_name.span(),
                    "Dynamic expressions $(...) are not supported inside variant blocks. Variants must be static.",
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

    let expanded = quote! {
        #[::silex::prelude::component]
        #vis fn #name(
            #props
        ) -> impl ::silex::dom::View {
            #(#var_decls)*
            #(#prop_sig_bindings)*

            ::silex::css::inject_style(#style_id, #final_css);
            #(#variant_injections)*

            ::silex::html::#tag(#children_binding)
                .class(#class_name)
                #(#style_bindings)*
                #(#variant_class_bindings)*
        }
    };

    Ok(expanded)
}
