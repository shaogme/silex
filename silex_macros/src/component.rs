use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat};

pub fn generate_component(input_fn: ItemFn) -> syn::Result<TokenStream2> {
    let fn_name = input_fn.sig.ident.clone();
    let props_name = format_ident!("{}Props", fn_name);
    let hidden_name = format_ident!("__silex_render_{}", fn_name);
    let vis = input_fn.vis.clone();
    let generics = input_fn.sig.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut field_defs = Vec::new();
    let mut prop_arg_names = Vec::new();

    for arg in input_fn.sig.inputs.iter() {
        let fn_arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r.self_token,
                    "Component functions cannot have `self` parameter",
                ));
            }
        };

        let pat = &fn_arg.pat;
        let ty = &fn_arg.ty;
        let attrs = &fn_arg.attrs;

        let param_name = match pat.as_ref() {
            Pat::Ident(ident) => ident.ident.clone(),
            _ => {
                return Err(syn::Error::new_spanned(
                    pat,
                    "Component parameters must be simple identifiers",
                ));
            }
        };

        field_defs.push(quote! {
            #(#attrs)*
            pub #param_name: #ty
        });
        prop_arg_names.push(param_name);
    }

    let mut hidden_fn = input_fn.clone();
    hidden_fn.sig.ident = hidden_name;
    hidden_fn.vis = syn::Visibility::Inherited;
    hidden_fn.sig.inputs = syn::parse_quote!(props: #props_name #ty_generics);
    hidden_fn
        .attrs
        .push(syn::parse_quote!(#[allow(non_snake_case)]));

    let mut hidden_stmts: Vec<syn::Stmt> = Vec::new();
    let destructure: syn::Stmt = syn::parse2(quote! {
        let #props_name { #(#prop_arg_names),* } = props;
    })?;
    hidden_stmts.push(destructure);
    hidden_stmts.extend(hidden_fn.block.stmts);
    hidden_fn.block.stmts = hidden_stmts;

    Ok(quote! {
        #[derive(::silex::macros::PropsBuilder)]
        #vis struct #props_name #impl_generics #where_clause {
            #(#field_defs,)*
        }

        #hidden_fn
    })
}
