use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Result, Token, parse2};

pub enum Directive {
    Scope,
}

pub struct RenderInput {
    pub directives: Vec<Directive>,
    pub body: Vec<syn::Stmt>,
}

impl Parse for RenderInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut directives = Vec::new();

        while input.peek(Token![use]) {
            input.parse::<Token![use]>()?;
            let ident: syn::Ident = input.parse()?;
            if ident == "scope" {
                directives.push(Directive::Scope);
                input.parse::<Token![;]>()?;
            } else {
                return Err(syn::Error::new_spanned(
                    ident,
                    "expected `scope`",
                ));
            }
        }

        let body = input.call(syn::Block::parse_within)?;

        Ok(RenderInput { directives, body })
    }
}

pub fn render_impl(input: TokenStream2) -> Result<TokenStream2> {
    let RenderInput { directives, body } = parse2(input)?;
    let mut result = quote! { move || { #(#body)* } };
    for directive in directives.into_iter().rev() {
        match directive {
            Directive::Scope => {
                result = quote! {
                    ::silex::dom::view::logic::ScopeView::new(#result)
                };
            }
        }
    }

    Ok(result)
}
