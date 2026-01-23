use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

pub fn derive_store_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = input.ident;
    let store_name = format_ident!("{}Store", name);

    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "Store derive only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "Store derive only supports structs",
            ));
        }
    };

    let struct_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {
            pub #name: ::silex::core::reactivity::RwSignal<#ty>
        }
    });

    let new_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: ::silex::core::reactivity::RwSignal::new(source.#name)
        }
    });

    let get_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: self.#name.get()
        }
    });

    let expanded = quote! {
        #[derive(Clone, Copy)]
        pub struct #store_name {
            #(#struct_fields),*
        }

        impl #store_name {
            pub fn new(source: #name) -> Self {
                Self {
                    #(#new_fields),*
                }
            }

            pub fn get(&self) -> #name {
                #name {
                    #(#get_fields),*
                }
            }
        }
    };

    Ok(expanded)
}
