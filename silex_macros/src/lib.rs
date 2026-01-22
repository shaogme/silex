use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let store_name = format_ident!("{}Store", name);

    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Store derive only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Store derive only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let struct_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {
            pub #name: ::silex::reactivity::RwSignal<#ty>
        }
    });

    let new_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: ::silex::reactivity::create_rw_signal(source.#name)
        }
    });

    let get_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: self.#name.get().expect("Store element missing")
        }
    });

    let expanded = quote! {
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

    TokenStream::from(expanded)
}
