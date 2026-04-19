use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{Expr, Result, Token};

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
