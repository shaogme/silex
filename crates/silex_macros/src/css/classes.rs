use crate::crate_path::silex_view;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{Expr, Result, Token};

// --- classes! [...] implementation ---

#[allow(clippy::large_enum_variant)]
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
    let __view = silex_view();
    let items = Punctuated::<ClassItem, Token![,]>::parse_terminated.parse2(input)?;
    if items.is_empty() {
        return Ok(quote! { #__view::attributes::AttributeGroup::default() });
    }

    let expanded = items.into_iter().map(|item| {
        let val = match item {
            ClassItem::Simple(e) => quote! { #e },
            ClassItem::Conditional(cls, cond) => quote! { (#cls, #cond) },
        };
        quote! {
            #__view::attributes::ApplyToDom::into_op(
                #__view::attributes::IntoStorable::into_storable(#val),
                #__view::attributes::ApplyTarget::Class,
            )
        }
    });

    Ok(quote! { #__view::attributes::AttributeGroup::new(vec![ #(#expanded),* ]) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn classes_converts_inputs_through_into_storable() {
        let output = classes_impl(quote! { "active" => condition })
            .unwrap()
            .to_string();
        assert!(output.contains("IntoStorable"), "{output}");
        assert!(output.contains("into_storable"), "{output}");
    }
}
