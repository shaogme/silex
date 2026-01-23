use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitStr, Result, Token};

// --- style! { ... } implementation ---

struct StyleProp {
    key: String,
    value: Expr,
}

impl Parse for StyleProp {
    fn parse(input: ParseStream) -> Result<Self> {
        let key_str = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            let ident = input.parse::<Ident>()?;
            ident.to_string().replace('_', "-")
        };

        input.parse::<Token![:]>()?;
        let value = input.parse::<Expr>()?;

        Ok(StyleProp {
            key: key_str,
            value,
        })
    }
}

pub fn style_impl(input: TokenStream) -> Result<TokenStream> {
    let parser = Punctuated::<StyleProp, Token![,]>::parse_terminated;
    let props = parser.parse2(input)?;

    let mut expanded_props = Vec::new();
    for prop in props {
        let key = prop.key;
        let value = prop.value;
        // Here we can optionally wrap `value` in `value.to_string()` if we wanted to support numbers
        // but for now, we assume user provides compatible types (String, &str)
        expanded_props.push(quote! { (#key, #value) });
    }

    if expanded_props.is_empty() {
        return Ok(quote! { () });
    }

    let tuple_body = quote! { ( #(#expanded_props),* ) };

    // Wrap in group
    Ok(quote! {
        silex::dom::attribute::group( #tuple_body )
    })
}

// --- classes! [...] implementation ---
// Syntax: classes![ "a", "b", "c" => cond ]

enum ClassItem {
    Simple(Expr),
    Conditional(Expr, Expr), // class => condition
}

impl Parse for ClassItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr = input.parse::<Expr>()?;
        if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            let cond = input.parse::<Expr>()?;
            Ok(ClassItem::Conditional(expr, cond))
        } else {
            Ok(ClassItem::Simple(expr))
        }
    }
}

pub fn classes_impl(input: TokenStream) -> Result<TokenStream> {
    let parser = Punctuated::<ClassItem, Token![,]>::parse_terminated;
    let items = parser.parse2(input)?;

    let mut expanded_items = Vec::new();
    for item in items {
        match item {
            ClassItem::Simple(expr) => {
                // simple "class"
                expanded_items.push(quote! { #expr });
            }
            ClassItem::Conditional(cls, cond) => {
                // "class" => condition  ->  ("class", condition)
                expanded_items.push(quote! { (#cls, #cond) });
            }
        }
    }

    if expanded_items.is_empty() {
        return Ok(quote! { () });
    }

    let tuple_body = quote! { ( #(#expanded_items),* ) };

    Ok(quote! {
        silex::dom::attribute::group( #tuple_body )
    })
}
