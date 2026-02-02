use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct CloneInput {
    idents: Punctuated<Ident, Token![,]>,
    body: Option<Expr>,
}

impl Parse for CloneInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut idents = Punctuated::new();

        loop {
            if input.is_empty() {
                break;
            }
            if input.peek(Token![=>]) {
                break;
            }

            let ident: Ident = input.parse()?;
            idents.push_value(ident);

            if input.peek(Token![,]) {
                idents.push_punct(input.parse()?);
            } else {
                break;
            }
        }

        let body = if input.peek(Token![=>]) {
            let _arrow: Token![=>] = input.parse()?;
            Some(input.parse()?)
        } else {
            None
        };

        Ok(CloneInput { idents, body })
    }
}

pub fn clone_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let input: CloneInput = syn::parse2(input)?;

    let idents = input.idents.iter();
    let clones = idents.map(|ident| {
        quote! {
            let #ident = #ident.clone();
        }
    });

    if let Some(body) = input.body {
        Ok(quote! {
            {
                #(#clones)*
                #body
            }
        })
    } else {
        Ok(quote! {
            #(#clones)*
        })
    }
}
