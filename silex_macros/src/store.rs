use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Meta, Result};

pub fn derive_store_impl(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let store_name = format_ident!("{}Store", name);
    let vis = &input.vis;

    let mut hook_name: Option<syn::Ident> = None;
    let mut err_msg: Option<String> = None;

    // Parse attributes
    for attr in &input.attrs {
        if attr.path().is_ident("store") {
            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("name") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }) = nv.value
                        {
                            hook_name = Some(syn::Ident::new(&lit_str.value(), lit_str.span()));
                        }
                    } else if nv.path.is_ident("err_msg") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }) = nv.value
                        {
                            err_msg = Some(lit_str.value());
                        }
                    }
                }
            }
        }
    }

    // Default hook name if not provided: use_{snake_case_name}
    let hook_fn_name = hook_name.unwrap_or_else(|| {
        let snake_name = to_snake_case(&name.to_string());
        format_ident!("use_{}", snake_name)
    });

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
            pub #name: ::silex::prelude::RwSignal<#ty>
        }
    });

    let new_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: ::silex::prelude::RwSignal::new(source.#name)
        }
    });

    let get_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            #name: self.#name.get()
        }
    });

    let panic_msg = err_msg.unwrap_or_else(|| format!("Context for {} not found", store_name));

    let expanded = quote! {
        /// Generated Store struct wrapping fields in RwSignal
        #[derive(Clone, Copy)]
        #vis struct #store_name {
            #(#struct_fields),*
        }

        impl #store_name {
            /// Create a new Store from the initial state
            pub fn new(source: #name) -> Self {
                Self {
                    #(#new_fields),*
                }
            }

            /// Get a snapshot of the current state
            pub fn get(&self) -> #name {
                #name {
                    #(#get_fields),*
                }
            }
        }

        impl ::silex::store::Store for #store_name {
            fn get() -> Self {
                 ::silex::prelude::use_context::<Self>().expect(#panic_msg)
            }
        }

        #vis fn #hook_fn_name() -> #store_name {
            <#store_name as ::silex::store::Store>::get()
        }
    };

    Ok(expanded)
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.char_indices() {
        if c.is_uppercase() {
            if i != 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
