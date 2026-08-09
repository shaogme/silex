use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{FnArg, GenericParam, ItemFn, Pat, visit::Visit};

struct GenericUsage {
    type_and_const_names: HashSet<String>,
    lifetime_names: HashSet<String>,
    used_type_and_const_names: HashSet<String>,
    used_lifetime_names: HashSet<String>,
}

impl GenericUsage {
    fn new(generics: &syn::Generics) -> Self {
        let mut type_and_const_names = HashSet::new();
        let mut lifetime_names = HashSet::new();
        for param in &generics.params {
            match param {
                GenericParam::Lifetime(def) => {
                    lifetime_names.insert(def.lifetime.ident.to_string());
                }
                GenericParam::Type(def) => {
                    type_and_const_names.insert(def.ident.to_string());
                }
                GenericParam::Const(def) => {
                    type_and_const_names.insert(def.ident.to_string());
                }
            }
        }

        Self {
            type_and_const_names,
            lifetime_names,
            used_type_and_const_names: HashSet::new(),
            used_lifetime_names: HashSet::new(),
        }
    }
}

impl<'ast> Visit<'ast> for GenericUsage {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        for segment in &path.segments {
            let name = segment.ident.to_string();
            if self.type_and_const_names.contains(&name) {
                self.used_type_and_const_names.insert(name);
            }
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        let name = lifetime.ident.to_string();
        if self.lifetime_names.contains(&name) {
            self.used_lifetime_names.insert(name);
        }
    }
}

pub fn generate_component(input_fn: ItemFn) -> syn::Result<TokenStream2> {
    let __silex = crate::crate_path::silex();
    let fn_name = input_fn.sig.ident.clone();
    let props_name = format_ident!("{}Props", fn_name);
    let builder_name = format_ident!("{}Builder", fn_name);
    let product_name = format_ident!("{}Component", fn_name);
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

    // Keep generic parameters that only occur in the render return type or bounds
    // represented in Props without adding runtime storage.
    let mut generic_usage = GenericUsage::new(&generics);
    for arg in input_fn.sig.inputs.iter() {
        if let FnArg::Typed(arg) = arg {
            generic_usage.visit_type(&arg.ty);
        }
    }

    let generic_marker_types: Vec<_> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(def) => {
                if def.lifetime.ident == "scope"
                    || generic_usage
                        .used_lifetime_names
                        .contains(&def.lifetime.ident.to_string())
                {
                    return None;
                }
                let lifetime = &def.lifetime;
                Some(quote! { &#lifetime () })
            }
            GenericParam::Type(def) => {
                if generic_usage
                    .used_type_and_const_names
                    .contains(&def.ident.to_string())
                {
                    return None;
                }
                let ident = &def.ident;
                Some(quote! { *const #ident })
            }
            GenericParam::Const(def) => {
                if generic_usage
                    .used_type_and_const_names
                    .contains(&def.ident.to_string())
                {
                    return None;
                }
                if matches!(&def.ty, syn::Type::Path(path) if path.path.is_ident("usize")) {
                    let ident = &def.ident;
                    Some(quote! { [(); #ident] })
                } else {
                    None
                }
            }
        })
        .collect();

    if let Some(scope) = input_fn
        .sig
        .generics
        .params
        .iter()
        .find_map(|param| match param {
            GenericParam::Lifetime(def) if def.lifetime.ident == "scope" => {
                Some(def.lifetime.clone())
            }
            _ => None,
        })
    {
        let marker_type = if generic_marker_types.is_empty() {
            quote! { &#scope () }
        } else {
            quote! { (&#scope (), #(#generic_marker_types),*) }
        };
        field_defs.push(quote! {
            #[doc(hidden)]
            #[allow(dead_code)]
            #[chain(default)]
            pub __silex_scope_marker: ::core::marker::PhantomData<#marker_type>
        });
    } else if !generic_marker_types.is_empty() {
        field_defs.push(quote! {
            #[doc(hidden)]
            #[allow(dead_code)]
            #[chain(default)]
            pub __silex_generic_marker: ::core::marker::PhantomData<
                fn() -> (#(#generic_marker_types),*)
            >
        });
    }

    let mut hidden_fn = input_fn.clone();
    hidden_fn.sig.ident = hidden_name.clone();
    hidden_fn.vis = syn::Visibility::Inherited;
    hidden_fn.sig.inputs = syn::parse_quote!(props: #props_name #ty_generics);
    hidden_fn
        .attrs
        .retain(|attr| !attr.path().is_ident("component"));
    hidden_fn
        .attrs
        .push(syn::parse_quote!(#[allow(non_snake_case, unused_variables, unused_mut)]));

    let mut hidden_stmts: Vec<syn::Stmt> = Vec::new();
    let destructure: syn::Stmt = syn::parse2(quote! {
        let #props_name { #(#prop_arg_names,)* .. } = props;
    })?;
    hidden_stmts.push(destructure);
    hidden_stmts.extend(hidden_fn.block.stmts);
    hidden_fn.block.stmts = hidden_stmts;

    Ok(quote! {
        #[derive(Clone, #__silex::macros::PropsBuilder)]
        #[silex_component(
            builder = #builder_name,
            product = #product_name,
            render = #hidden_name,
            constructor = #fn_name,
        )]
        #vis struct #props_name #impl_generics #where_clause {
            #(#field_defs,)*
        }

        #hidden_fn
    })
}
