use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{FnArg, GenericParam, ItemFn, Pat, Type, visit::Visit};

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
    let mut ctx_field = None;

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

        let is_ctx = attrs.iter().any(|attr| attr.path().is_ident("ctx"));
        if is_ctx {
            if ctx_field.is_some() {
                return Err(syn::Error::new_spanned(
                    fn_arg,
                    "component functions must declare exactly one `#[ctx]` parameter",
                ));
            }
            if param_name == "owner" || param_name == "error_handler" {
                return Err(syn::Error::new_spanned(
                    fn_arg,
                    "`owner` and `error_handler` are reserved aliases; use another ctx parameter name",
                ));
            }
            if attrs.iter().any(|attr| attr.path().is_ident("chain")) {
                return Err(syn::Error::new_spanned(
                    fn_arg,
                    "`#[ctx]` cannot be combined with `#[chain]`",
                ));
            }
            ctx_field = Some(param_name.clone());
        } else if param_name == "owner" || param_name == "error_handler" {
            return Err(syn::Error::new_spanned(
                fn_arg,
                "explicit owner and error-handler parameters are not component props; declare one `#[ctx]` parameter",
            ));
        } else if is_special_ctx_type(ty) {
            return Err(syn::Error::new_spanned(
                ty,
                "explicit owner and error-handler parameters are not component props; declare one `#[ctx]` parameter",
            ));
        }

        if attrs.iter().any(|attr| attr.path().is_ident("inject")) {
            return Err(syn::Error::new_spanned(
                fn_arg,
                "component parameters no longer support #[inject(...)]; declare a `#[ctx]` parameter",
            ));
        }

        field_defs.push(quote! {
            #(#attrs)*
            pub #param_name: #ty
        });
        prop_arg_names.push(param_name);
    }

    let Some(ctx_field) = ctx_field else {
        return Err(syn::Error::new_spanned(
            &input_fn.sig.ident,
            "component functions must declare exactly one `#[ctx]` parameter",
        ));
    };
    let ctx_ty = input_fn
        .sig
        .inputs
        .iter()
        .find_map(|arg| match arg {
            FnArg::Typed(arg)
                if matches!(arg.pat.as_ref(), Pat::Ident(ident) if ident.ident == ctx_field) =>
            {
                Some(arg.ty.clone())
            }
            _ => None,
        })
        .expect("ctx field must be present in component arguments");

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
                if generic_usage
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

    if !generic_marker_types.is_empty() {
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
    if let Some(owner_lifetime) = context_lifetime(&generics, &ctx_ty) {
        hidden_fn
            .sig
            .generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(
                #ctx_ty: #__silex::core::SilexContextProvider<#owner_lifetime>
            ));
    }

    let fallible_render = is_fallible_render(&input_fn.sig.output, &input_fn.block);
    if fallible_render
        && !is_result_output(&input_fn.sig.output)
        && let syn::ReturnType::Type(_, output) = &input_fn.sig.output
    {
        hidden_fn.sig.output = syn::parse_quote!(
            -> #__silex::core::SilexResult<#output>
        );
    }

    let mut hidden_stmts: Vec<syn::Stmt> = Vec::new();
    let destructure: syn::Stmt = syn::parse2(quote! {
        let #props_name { #(#prop_arg_names,)* .. } = props;
    })?;
    hidden_stmts.push(destructure);
    hidden_stmts.push(syn::parse_quote! {
        let owner = #__silex::core::SilexContextProvider::owner(&#ctx_field);
    });
    hidden_stmts.push(syn::parse_quote! {
        let error_handler = #__silex::core::SilexContextProvider::error_reporter(&#ctx_field);
    });
    hidden_stmts.extend(hidden_fn.block.stmts);
    hidden_fn.block.stmts = hidden_stmts;

    Ok(quote! {
        #[derive(Clone, #__silex::macros::PropsBuilder)]
        #[silex_component(
            builder = #builder_name,
            product = #product_name,
            render = #hidden_name,
            constructor = #fn_name
        )]
        #vis struct #props_name #impl_generics #where_clause {
            #(#field_defs,)*
        }

        #hidden_fn
    })
}

fn is_fallible_render(output: &syn::ReturnType, block: &syn::Block) -> bool {
    is_result_output(output) || block_returns_result(block)
}

fn is_result_output(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else {
        return false;
    };
    let Type::Path(type_path) = ty.as_ref() else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result" || segment.ident == "SilexResult")
}

fn block_returns_result(block: &syn::Block) -> bool {
    let Some(syn::Stmt::Expr(expression, _)) = block.stmts.last() else {
        return false;
    };
    expression_returns_result(expression)
}

fn expression_returns_result(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Call(call) => match call.func.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Ok" || segment.ident == "Err"),
            _ => false,
        },
        syn::Expr::Block(block) => block_returns_result(&block.block),
        syn::Expr::If(if_expression) => {
            block_returns_result(&if_expression.then_branch)
                && if_expression
                    .else_branch
                    .as_ref()
                    .is_some_and(|(_, expression)| expression_returns_result(expression))
        }
        syn::Expr::Match(match_expression) => match_expression
            .arms
            .iter()
            .all(|arm| expression_returns_result(&arm.body)),
        syn::Expr::Paren(paren) => expression_returns_result(&paren.expr),
        _ => false,
    }
}

fn is_special_ctx_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    matches!(
        segment.ident.to_string().as_str(),
        "OwnerAccess" | "OwnerHandle" | "ErrorReporter" | "ErrorHandler" | "ErrorHandlerToken"
    )
}

fn context_lifetime(generics: &syn::Generics, ctx_ty: &Type) -> Option<syn::Lifetime> {
    struct LifetimeVisitor {
        names: std::collections::HashSet<String>,
    }

    impl<'ast> Visit<'ast> for LifetimeVisitor {
        fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
            self.names.insert(lifetime.ident.to_string());
        }
    }

    let mut visitor = LifetimeVisitor {
        names: std::collections::HashSet::new(),
    };
    visitor.visit_type(ctx_ty);

    generics
        .params
        .iter()
        .find_map(|param| match param {
            GenericParam::Lifetime(def)
                if visitor.names.contains(&def.lifetime.ident.to_string()) =>
            {
                Some(def.lifetime.clone())
            }
            _ => None,
        })
        .or_else(|| {
            generics.params.iter().find_map(|param| match param {
                GenericParam::Lifetime(def) => Some(def.lifetime.clone()),
                _ => None,
            })
        })
}
