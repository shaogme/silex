use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, Meta, Result, Type};

#[derive(Clone)]
struct PersistFieldConfig {
    backend: syn::Ident,
    codec: syn::LitStr,
    key: Option<String>,
}

pub fn derive_store_impl(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let store_name = format_ident!("{}Store", name);
    let vis = &input.vis;

    let mut hook_name: Option<syn::Ident> = None;
    let mut err_msg: Option<String> = None;
    let mut persist_prefix: Option<String> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("store") {
            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("name")
                        && let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }) = nv.value
                    {
                        hook_name = Some(syn::Ident::new(&lit_str.value(), lit_str.span()));
                    } else if nv.path.is_ident("err_msg")
                        && let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }) = nv.value
                    {
                        err_msg = Some(lit_str.value());
                    }
                }
            }
        } else if attr.path().is_ident("persist")
            && let Meta::List(list) = &attr.meta
        {
            let nested = list.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            )?;
            for meta in nested {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident("prefix")
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) = nv.value
                {
                    persist_prefix = Some(lit_str.value());
                }
            }
        }
    }

    let hook_fn_name = hook_name.unwrap_or_else(|| {
        let snake_name = to_snake_case(&name.to_string());
        format_ident!("use_{}", snake_name)
    });

    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Store derive only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Store derive only supports structs",
            ));
        }
    };

    let struct_fields = fields
        .iter()
        .map(|field| {
            let name = &field.ident;
            let ty = &field.ty;
            match parse_field_persist(field)? {
                Some(_) => Ok(quote! { pub #name: ::silex::prelude::Persistent<#ty> }),
                None => Ok(quote! { pub #name: ::silex::prelude::RwSignal<#ty> }),
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let new_fields = fields
        .iter()
        .map(|field| build_field_initializer(field, persist_prefix.as_deref()))
        .collect::<Result<Vec<_>>>()?;

    let get_fields = fields.iter().map(|f| {
        let name = &f.ident;
        quote! { #name: self.#name.get() }
    });

    let panic_msg = err_msg.unwrap_or_else(|| format!("Context for {} not found", store_name));

    Ok(quote! {
        /// Generated Store struct wrapping fields in reactive handles
        #[derive(Clone, Copy)]
        #vis struct #store_name {
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

        impl ::silex::store::Store for #store_name {
            fn get() -> Self {
                 ::silex::prelude::use_context::<Self>().expect(#panic_msg)
            }
        }

        #vis fn #hook_fn_name() -> #store_name {
            <#store_name as ::silex::store::Store>::get()
        }
    })
}

fn build_field_initializer(field: &Field, persist_prefix: Option<&str>) -> Result<TokenStream> {
    let name = field.ident.as_ref().expect("named field");
    let ty = &field.ty;

    if let Some(config) = parse_field_persist(field)? {
        let key = config.key.unwrap_or_else(|| name.to_string());
        let full_key = if let Some(prefix) = persist_prefix {
            format!("{}{}", prefix, key)
        } else {
            key
        };
        let backend_method = config.backend;
        let codec_tokens = codec_builder_tokens(ty, &config.codec)?;
        Ok(quote! {
            #name: ::silex::prelude::Persistent::builder(#full_key)
                .#backend_method()
                #codec_tokens
                .default(source.#name)
                .build()
        })
    } else {
        Ok(quote! {
            #name: ::silex::prelude::RwSignal::new(source.#name)
        })
    }
}

fn parse_field_persist(field: &Field) -> Result<Option<PersistFieldConfig>> {
    let mut config = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("persist") {
            continue;
        }

        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        )?;

        let mut backend: Option<syn::Ident> = None;
        let mut codec: Option<syn::LitStr> = None;
        let mut key: Option<String> = None;

        for meta in nested {
            match meta {
                Meta::Path(path) if path.is_ident("local") => {
                    backend = Some(format_ident!("local"))
                }
                Meta::Path(path) if path.is_ident("session") => {
                    backend = Some(format_ident!("session"))
                }
                Meta::Path(path) if path.is_ident("query") => {
                    backend = Some(format_ident!("query"))
                }
                Meta::NameValue(nv) if nv.path.is_ident("key") => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) = nv.value
                    {
                        key = Some(lit_str.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "persist key must be a string literal",
                        ));
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("codec") => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) = nv.value
                    {
                        codec = Some(lit_str);
                    } else {
                        return Err(syn::Error::new_spanned(
                            nv,
                            "persist codec must be a string literal",
                        ));
                    }
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unsupported #[persist(...)] option",
                    ));
                }
            }
        }

        let backend = backend.ok_or_else(|| {
            syn::Error::new_spanned(
                attr,
                "#[persist(...)] requires one backend: local, session, or query",
            )
        })?;
        let codec = codec.ok_or_else(|| {
            syn::Error::new_spanned(
                attr,
                "#[persist(...)] requires codec = \"string\" | \"parse\" | \"json\"",
            )
        })?;

        config = Some(PersistFieldConfig {
            backend,
            codec,
            key,
        });
    }

    Ok(config)
}

fn codec_builder_tokens(ty: &Type, codec: &syn::LitStr) -> Result<TokenStream> {
    match codec.value().as_str() {
        "string" => Ok(quote!(.string())),
        "parse" => Ok(quote!(.parse::<#ty>())),
        "json" => Ok(quote!(.json::<#ty>())),
        _ => Err(syn::Error::new_spanned(
            codec,
            "unsupported codec, expected string|parse|json",
        )),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn store_macro_emits_persistent_fields_for_persist_attributes() {
        let input: DeriveInput = parse_quote! {
            #[store(name = "use_settings")]
            #[persist(prefix = "settings-")]
            pub struct Settings {
                #[persist(local, codec = "string")]
                pub theme: String,
                #[persist(query, key = "page", codec = "parse")]
                pub page: u32,
                pub username: String,
            }
        };

        let expanded = derive_store_impl(input).unwrap().to_string();

        assert!(expanded.contains("pub theme : :: silex :: prelude :: Persistent < String >"));
        assert!(expanded.contains("pub page : :: silex :: prelude :: Persistent < u32 >"));
        assert!(expanded.contains("pub username : :: silex :: prelude :: RwSignal < String >"));
        assert!(expanded.contains(
            ":: silex :: prelude :: Persistent :: builder (\"settings-theme\") . local () . string () . default (source . theme) . build ()"
        ));
        assert!(expanded.contains(
            ":: silex :: prelude :: Persistent :: builder (\"settings-page\") . query () . parse :: < u32 > () . default (source . page) . build ()"
        ));
    }
}
