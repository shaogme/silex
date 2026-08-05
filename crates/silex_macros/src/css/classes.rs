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
    let __silex = crate::crate_path::silex();
    let items = Punctuated::<ClassItem, Token![,]>::parse_terminated.parse2(input)?;
    if items.is_empty() {
        return Ok(quote! { #__silex::dom::attribute::AttributeGroup::default() });
    }

    let expanded = items.into_iter().map(|item| {
        let val = match item {
            ClassItem::Simple(e) => quote! { #e },
            ClassItem::Conditional(cls, cond) => quote! { (#cls, #cond) },
        };
        quote! {
            #__silex::dom::attribute::ApplyToDom::into_op(
                #__silex::dom::attribute::IntoStorable::into_storable(#val),
                #__silex::dom::attribute::ApplyTarget::Class,
            )
        }
    });

    Ok(quote! { #__silex::dom::attribute::AttributeGroup(vec![ #(#expanded),* ]) })
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
