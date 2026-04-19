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
        let key = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            input.parse::<Ident>()?.to_string().replace('_', "-")
        };
        input.parse::<Token![:]>()?;
        Ok(StyleProp {
            key,
            value: input.parse()?,
        })
    }
}

pub fn style_impl(input: TokenStream) -> Result<TokenStream> {
    let props = Punctuated::<StyleProp, Token![,]>::parse_terminated.parse2(input)?;
    if props.is_empty() {
        return Ok(quote! { ::silex::dom::attribute::AttributeGroup::default() });
    }

    let items = props.into_iter().map(|p| {
        let (k, v) = (p.key, p.value);
        quote! { ::silex::dom::attribute::ApplyToDom::into_op((#k, #v), ::silex::dom::attribute::OwnedApplyTarget::Style) }
    });

    Ok(quote! { ::silex::dom::attribute::AttributeGroup(vec![ #(#items),* ]) })
}

// --- classes! [...] implementation ---

enum ClassItem {
    Simple(Expr),
    Conditional(Expr, Expr),
}

impl Parse for ClassItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr = input.parse::<Expr>()?;
        if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            Ok(ClassItem::Conditional(expr, input.parse()?))
        } else {
            Ok(ClassItem::Simple(expr))
        }
    }
}

pub fn classes_impl(input: TokenStream) -> Result<TokenStream> {
    let items = Punctuated::<ClassItem, Token![,]>::parse_terminated.parse2(input)?;
    if items.is_empty() {
        return Ok(quote! { ::silex::dom::attribute::AttributeGroup::default() });
    }

    let expanded = items.into_iter().map(|item| {
        let val = match item {
            ClassItem::Simple(e) => quote! { #e },
            ClassItem::Conditional(cls, cond) => quote! { (#cls, #cond) },
        };
        quote! { ::silex::dom::attribute::ApplyToDom::into_op(#val, ::silex::dom::attribute::OwnedApplyTarget::Class) }
    });

    Ok(quote! { ::silex::dom::attribute::AttributeGroup(vec![ #(#expanded),* ]) })
}
