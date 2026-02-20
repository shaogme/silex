use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct CloneItem {
    should_inner_clone: bool,
    ident: Ident,
}

impl Parse for CloneItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let should_inner_clone = if input.peek(Token![@]) {
            let _: Token![@] = input.parse()?;
            true
        } else {
            false
        };

        let ident: Ident = input.parse()?;

        Ok(CloneItem {
            should_inner_clone,
            ident,
        })
    }
}

struct CloneInput {
    items: Punctuated<CloneItem, Token![,]>,
    body: Option<Expr>,
}

impl Parse for CloneInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Punctuated::new();

        loop {
            if input.is_empty() {
                break;
            }
            if input.peek(Token![=>]) {
                break;
            }

            items.push_value(input.parse()?);

            if input.peek(Token![,]) {
                items.push_punct(input.parse()?);
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

        Ok(CloneInput { items, body })
    }
}

pub fn clone_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let mut input: CloneInput = syn::parse2(input)?;

    let outer_clones = input.items.iter().map(|item| {
        let ident = &item.ident;
        quote! {
            let #ident = #ident.clone();
        }
    });

    if let Some(ref mut body) = input.body {
        let inner_clones: Vec<_> = input
            .items
            .iter()
            .filter(|item| item.should_inner_clone)
            .map(|item| {
                let ident = &item.ident;
                quote! {
                    let #ident = #ident.clone();
                }
            })
            .collect();

        if !inner_clones.is_empty()
            && let Expr::Closure(closure) = body
        {
            let old_body = &closure.body;
            let new_body_tokens = quote! {
                {
                    #(#inner_clones)*
                    #old_body
                }
            };

            // Parse the new body back into an Expr to properly insert it into the closure
            match syn::parse2::<Expr>(new_body_tokens) {
                Ok(new_expr) => {
                    *closure.body = new_expr;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(quote! {
            {
                #(#outer_clones)*
                #body
            }
        })
    } else {
        Ok(quote! {
            #(#outer_clones)*
        })
    }
}
