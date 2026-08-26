use crate::crate_path::silex_view;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{
    Attribute, Data, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type, Visibility,
    parse::Parse, visit::Visit,
};

#[derive(Clone, Default)]
struct FieldAttrs {
    default: bool,
    default_value: Option<TokenStream2>,
    into_trait: bool,
    render: bool,
    render_fn_args: Option<Vec<Type>>,
    chained: bool,
    chain_method: Option<Ident>,
    chain_each: bool,
    ctx: bool,
    attrs: bool,
}

#[derive(Clone)]
struct FieldSpec {
    ident: Ident,
    ty: Type,
    attrs: FieldAttrs,
    required: bool,
}

impl FieldSpec {
    fn from_syn_field(field: &syn::Field) -> syn::Result<Self> {
        let ident = field
            .ident
            .clone()
            .expect("named fields must have identifiers");
        let attrs = parse_field_attrs(&field.attrs)?;
        if attrs.attrs && (attrs.ctx || attrs.chained || attrs.chain_each) {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`#[attrs]` cannot be combined with `#[ctx]` or `#[chain]`",
            ));
        }
        if attrs.attrs && !is_attribute_group_type(&field.ty) {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`#[attrs]` requires an `AttributeGroup<'scope>` field",
            ));
        }
        if attrs.chain_each && vec_item_type(&field.ty).is_none() {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`#[chain(each)]` requires a `Vec<T>` field",
            ));
        }
        if attrs.chain_each && attrs.render_fn_args.is_some() {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`#[prop(render_fn(...))]` cannot be combined with `#[chain(each)]`",
            ));
        }
        let required = attrs.chained && !attrs.default && attrs.default_value.is_none();
        Ok(FieldSpec {
            ident,
            ty: field.ty.clone(),
            attrs,
            required,
        })
    }
}

#[derive(Clone)]
struct ComponentMetadata {
    builder_name: Ident,
    product_name: Ident,
    render_fn_name: Ident,
    constructor_name: Option<Ident>,
    html_tag: Option<Ident>,
}

struct BuilderContext {
    vis: Visibility,
    props_name: Ident,
    builder_name: Ident,
    component_name: Ident,
    product_name: Ident,
    render_fn_name: Ident,
    generics: syn::Generics,
    fields: Vec<FieldSpec>,
    prop_generic_idents: Vec<Ident>,
    required_fields: Vec<FieldSpec>,
    owner_lifetime: syn::Lifetime,
    ctx_field: Option<Ident>,
    html_tag: Option<Ident>,
}

impl BuilderContext {
    fn new(input: DeriveInput) -> syn::Result<Self> {
        let DeriveInput {
            ident: props_name,
            generics,
            vis,
            attrs,
            data,
            ..
        } = input;

        let inferred_component_name = strip_props_suffix(&props_name);
        let metadata = parse_component_metadata(&attrs, &props_name)?;
        // Component metadata is authoritative; only standalone derives use the legacy naming fallback.
        let (builder_name, product_name, render_fn_name, component_name, html_tag) =
            if let Some(metadata) = metadata {
                (
                    metadata.builder_name,
                    metadata.product_name,
                    metadata.render_fn_name,
                    metadata
                        .constructor_name
                        .unwrap_or_else(|| inferred_component_name.clone()),
                    metadata.html_tag,
                )
            } else {
                (
                    format_ident!("{}Builder", props_name),
                    format_ident!("{}Component", inferred_component_name),
                    format_ident!("__silex_render_{}", inferred_component_name),
                    inferred_component_name,
                    None,
                )
            };
        let fields = match data {
            Data::Struct(ref data) => match &data.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(FieldSpec::from_syn_field)
                    .collect::<syn::Result<Vec<_>>>()?,
                _ => {
                    return Err(syn::Error::new_spanned(
                        props_name,
                        "PropsBuilder only supports structs with named fields",
                    ));
                }
            },
            _ => {
                return Err(syn::Error::new_spanned(
                    props_name,
                    "PropsBuilder only supports structs",
                ));
            }
        };

        let mut chain_methods = HashSet::new();
        for field in fields.iter().filter(|field| field.attrs.chained) {
            let method = field.attrs.chain_method.as_ref().unwrap_or(&field.ident);
            if !chain_methods.insert(method.to_string()) {
                return Err(syn::Error::new_spanned(
                    method,
                    format!("duplicate chain method `{method}`"),
                ));
            }
            if method == "new" || method == "build" {
                return Err(syn::Error::new_spanned(
                    method,
                    format!("chain method `{method}` conflicts with a generated builder method"),
                ));
            }
        }

        let owner_lifetime = owner_lifetime(&generics, &fields);

        let ctx_fields: Vec<_> = fields.iter().filter(|field| field.attrs.ctx).collect();
        if ctx_fields.len() > 1 {
            return Err(syn::Error::new_spanned(
                &ctx_fields[1].ident,
                "PropsBuilder supports exactly one `#[ctx]` field",
            ));
        }
        let ctx_field = ctx_fields.first().map(|field| field.ident.clone());

        if fields.iter().filter(|field| field.attrs.attrs).count() > 1 {
            let duplicate = fields
                .iter()
                .filter(|field| field.attrs.attrs)
                .nth(1)
                .expect("second attrs field must exist");
            return Err(syn::Error::new_spanned(
                &duplicate.ident,
                "PropsBuilder supports at most one `#[attrs]` field",
            ));
        }

        if let Some(field) = fields.iter().find(|field| is_reactive_default_field(field))
            && ctx_field.is_none()
        {
            return Err(syn::Error::new_spanned(
                &field.ident,
                "RxDefault requires an explicit `#[ctx]` parameter; scoped reactive defaults cannot create an implicit runtime",
            ));
        }

        let required_fields: Vec<_> = fields.iter().filter(|f| f.required).cloned().collect();
        let prop_generic_idents: Vec<_> = required_fields
            .iter()
            .map(|f| {
                let name = to_upper_camel_case(&f.ident.to_string());
                format_ident!("P{}", name)
            })
            .collect();

        Ok(Self {
            vis,
            props_name,
            builder_name,
            component_name,
            product_name,
            render_fn_name,
            generics,
            fields,
            prop_generic_idents,
            required_fields,
            owner_lifetime,
            ctx_field,
            html_tag,
        })
    }

    fn get_builder_ty(&self, prop_states: &[TokenStream2]) -> TokenStream2 {
        let mut params = Vec::new();
        // 1. Lifetimes must come first
        for param in &self.generics.params {
            if let syn::GenericParam::Lifetime(l) = param {
                let lifetime = &l.lifetime;
                params.push(quote! { #lifetime });
            }
        }
        // 2. Then prop states (which are types)
        for state in prop_states {
            params.push(quote! { #state });
        }
        // 3. Then other generic parameters (types and consts)
        for param in &self.generics.params {
            match param {
                syn::GenericParam::Type(t) => {
                    let ident = &t.ident;
                    params.push(quote! { #ident });
                }
                syn::GenericParam::Const(c) => {
                    let ident = &c.ident;
                    params.push(quote! { #ident });
                }
                _ => {}
            }
        }
        let builder_name = &self.builder_name;
        if params.is_empty() {
            quote! { #builder_name }
        } else {
            quote! { #builder_name <#(#params),*> }
        }
    }

    fn get_builder_generics(&self) -> (TokenStream2, TokenStream2) {
        let mut decl_params = Vec::new();
        let mut ty_params = Vec::new();

        // 1. Lifetimes
        for param in &self.generics.params {
            if let syn::GenericParam::Lifetime(l) = param {
                decl_params.push(quote! { #param });
                let lifetime = &l.lifetime;
                ty_params.push(quote! { #lifetime });
            }
        }
        // 2. Prop generics
        for ident in &self.prop_generic_idents {
            decl_params.push(quote! { #ident });
            ty_params.push(quote! { #ident });
        }
        // 3. Original type/const params
        for param in &self.generics.params {
            match param {
                syn::GenericParam::Type(t) => {
                    decl_params.push(quote! { #param });
                    let ident = &t.ident;
                    ty_params.push(quote! { #ident });
                }
                syn::GenericParam::Const(c) => {
                    decl_params.push(quote! { #param });
                    let ident = &c.ident;
                    ty_params.push(quote! { #ident });
                }
                _ => {}
            }
        }

        let decl = if decl_params.is_empty() {
            quote! {}
        } else {
            quote! { <#(#decl_params),*> }
        };
        let ty = if ty_params.is_empty() {
            quote! {}
        } else {
            quote! { <#(#ty_params),*> }
        };
        (decl, ty)
    }

    fn owner_lifetime(&self) -> syn::Lifetime {
        self.owner_lifetime.clone()
    }

    fn ctx_where_clause(&self) -> syn::WhereClause {
        let __silex = crate::crate_path::silex();
        let scope = self.owner_lifetime();
        let mut where_clause = self
            .generics
            .where_clause
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(where));
        if let Some(ctx_field) = &self.ctx_field {
            let ctx_ty = self
                .fields
                .iter()
                .find(|field| &field.ident == ctx_field)
                .map(|field| &field.ty)
                .expect("ctx field must be present in Props");
            where_clause
                .predicates
                .push(syn::parse_quote!(#ctx_ty: #__silex::core::SilexContextProvider<#scope>));
        }
        where_clause
    }

    fn reactive_input_generic_ident(&self) -> Ident {
        let mut index = 0;
        loop {
            let suffix = if index == 0 {
                String::new()
            } else {
                index.to_string()
            };
            let candidate = format_ident!("__SilexReactiveInput{}", suffix);
            let used = self.generics.params.iter().any(|param| match param {
                syn::GenericParam::Lifetime(param) => param.lifetime.ident == candidate,
                syn::GenericParam::Type(param) => param.ident == candidate,
                syn::GenericParam::Const(param) => param.ident == candidate,
            }) || self
                .prop_generic_idents
                .iter()
                .any(|ident| ident == &candidate);
            if !used {
                return candidate;
            }
            index += 1;
        }
    }

    fn fresh_generic_ident(&self, prefix: &str, reserved: &[&Ident]) -> Ident {
        let mut index = 0usize;
        loop {
            let candidate = if index == 0 {
                format_ident!("{}", prefix)
            } else {
                format_ident!("{}{}", prefix, index)
            };
            let used = self.generics.params.iter().any(|param| match param {
                syn::GenericParam::Lifetime(param) => param.lifetime.ident == candidate,
                syn::GenericParam::Type(param) => param.ident == candidate,
                syn::GenericParam::Const(param) => param.ident == candidate,
            }) || self
                .prop_generic_idents
                .iter()
                .any(|ident| ident == &candidate)
                || reserved.contains(&&candidate);
            if !used {
                return candidate;
            }
            index += 1;
        }
    }

    fn pending_attribute_ty(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let scope = self.owner_lifetime();
        quote! { #__view::attributes::AttrOp<#scope> }
    }

    fn has_attrs(&self) -> bool {
        self.fields.iter().any(|field| field.attrs.attrs)
    }

    fn has_fallible_reactive_defaults(&self) -> bool {
        self.fields.iter().any(is_fallible_reactive_default_field)
    }

    fn generate_builder_struct(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let vis = &self.vis;
        let builder_name = &self.builder_name;
        let (_, _, where_clause) = self.generics.split_for_impl();
        let (builder_generics_decl, _) = self.get_builder_generics();
        let pending_attribute_ty = self.pending_attribute_ty();

        let builder_fields = self.fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if field.attrs.chained {
                quote! { #ident: ::core::option::Option<#ty> }
            } else {
                quote! { #ident: #ty }
            }
        });

        let mut marker_types = Vec::new();
        for param in &self.generics.params {
            if let syn::GenericParam::Lifetime(l) = param {
                let lifetime = &l.lifetime;
                marker_types.push(quote! { &#lifetime () });
            }
        }
        for ident in &self.prop_generic_idents {
            marker_types.push(quote! { #ident });
        }
        for param in &self.generics.params {
            match param {
                syn::GenericParam::Type(t) => {
                    let ident = &t.ident;
                    marker_types.push(quote! { #ident });
                }
                syn::GenericParam::Const(c) => {
                    let ident = &c.ident;
                    marker_types.push(quote! { #ident });
                }
                _ => {}
            }
        }

        quote! {
            #[derive(Clone)]
            #[allow(non_camel_case_types)]
            #vis struct #builder_name #builder_generics_decl #where_clause {
                #(#builder_fields,)*
                _pending_attrs: ::std::vec::Vec<#pending_attribute_ty>,
                _markers: ::core::marker::PhantomData<(#(#marker_types),*)>,
            }
        }
    }

    fn generate_product_struct(&self) -> TokenStream2 {
        let vis = &self.vis;
        let product_name = &self.product_name;
        let props_name = &self.props_name;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

        quote! {
            #[derive(Clone)]
            #[allow(non_camel_case_types)]
            #vis struct #product_name #impl_generics #where_clause {
                props: #props_name #ty_generics,
            }
        }
    }

    fn generate_builder_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let (builder_generics_decl, builder_generics_type) = self.get_builder_generics();
        let builder_name = &self.builder_name;
        let where_clause = self.ctx_where_clause();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__view::mount::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);

        let standalone_fields: Vec<_> = self
            .fields
            .iter()
            .filter(|f| !f.attrs.chained && !f.attrs.attrs)
            .collect();
        let builder_new_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        });

        let builder_field_inits = self.fields.iter().map(|field| {
            let ident = &field.ident;
            if field.attrs.attrs {
                quote! {
                    #ident: #__view::attributes::AttributeGroup::default()
                }
            } else if !field.attrs.chained {
                quote! { #ident }
            } else {
                quote! { #ident: ::core::option::Option::None }
            }
        });

        let builder_value = quote! {
            #builder_name {
                #(#builder_field_inits,)*
                _pending_attrs: ::std::vec::Vec::new(),
                _markers: ::core::marker::PhantomData,
            }
        };

        let builder_setters = self
            .fields
            .iter()
            .filter(|field| {
                !is_internal_marker_field(field)
                    && !field.attrs.attrs
                    && self
                        .ctx_field
                        .as_ref()
                        .map(|ctx_field| ctx_field != &field.ident)
                        .unwrap_or(true)
            })
            .map(|f| self.generate_setter(f));

        quote! {
            impl #builder_generics_decl #builder_name #builder_generics_type #where_clause {
                pub fn new(#(#builder_new_params),*) -> #builder_ty_initial {
                    #builder_value
                }

                #(#builder_setters)*
            }
        }
    }

    fn generate_setter(&self, field: &FieldSpec) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let builder_name = &self.builder_name;
        let ident = &field.ident;
        let ty = &field.ty;
        let method = field.attrs.chain_method.as_ref().unwrap_or(ident);
        let vec_item_ty = chained_vec_item_type(field);
        let scope = self.owner_lifetime();
        let reactive_input = self.ctx_field.is_some() && is_reactive_input_type(ty, &scope);
        let ctx_field = self.ctx_field.as_ref();
        let setter_generic = self.reactive_input_generic_ident();
        let scope_expr = if reactive_input {
            let ctx_field = ctx_field.expect("reactive input setters require a ctx field");
            if field.required {
                quote! {
                    #__silex::core::SilexContextProvider::owner(&#ctx_field)
                }
            } else {
                quote! {
                    #__silex::core::SilexContextProvider::owner(&self.#ctx_field)
                }
            }
        } else {
            quote! {}
        };

        let fields_destructure: Vec<_> = self.fields.iter().map(|f| &f.ident).collect();

        let (setter_param, setter_value, generic, where_clause) = if let Some(item_ty) =
            vec_item_ty.as_ref()
        {
            let setter_param = if field.attrs.render && is_any_view_type(item_ty) {
                quote! { impl #__view::mount::View<#scope> + #scope }
            } else if field.attrs.into_trait || is_auto_into_type(item_ty) {
                quote! { impl ::core::convert::Into<#item_ty> }
            } else {
                quote! { #item_ty }
            };
            let setter_value = if field.attrs.render && is_any_view_type(item_ty) {
                quote! { #__view::mount::View::into_any(val) }
            } else if field.attrs.into_trait || is_auto_into_type(item_ty) {
                quote! { val.into() }
            } else {
                quote! { val }
            };
            (setter_param, setter_value, quote! {}, quote! {})
        } else if let Some(render_fn_args) = field.attrs.render_fn_args.as_deref() {
            let render_fn = self.fresh_generic_ident("__SilexRenderFn", &[]);
            let render_view = self.fresh_generic_ident("__SilexRenderView", &[&render_fn]);
            (
                quote! { #render_fn },
                quote! { <#ty>::from_fn(val) },
                quote! { <#render_fn, #render_view> },
                quote! {
                    where
                        #render_fn: Fn(#(#render_fn_args),*) -> #render_view + #scope,
                        #render_view: #__view::mount::View<#scope> + #scope,
                },
            )
        } else if reactive_input {
            (
                quote! { #setter_generic },
                quote! {
                    <#setter_generic as #__silex::core::ReactiveInput<#scope, #ty>>::into_reactive_input(
                        val,
                        #scope_expr,
                    )
                },
                quote! { <#setter_generic> },
                quote! {
                    where
                        #setter_generic: #__silex::core::ReactiveInput<#scope, #ty>
                },
            )
        } else {
            let setter_param = if is_any_view_type(ty) {
                quote! { impl #__view::mount::View<#scope> + #scope }
            } else if field.attrs.render {
                quote! { #ty }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { impl ::core::convert::Into<#ty> }
            } else {
                quote! { #ty }
            };
            let setter_value = if is_any_view_type(ty) {
                quote! { #__view::mount::View::into_any(val) }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { val.into() }
            } else {
                quote! { val }
            };
            (setter_param, setter_value, quote! {}, quote! {})
        };

        let setter_value = if reactive_input {
            quote! { #setter_value? }
        } else {
            setter_value
        };

        let final_value = if !field.attrs.chained {
            setter_value.clone()
        } else {
            quote! { ::core::option::Option::Some(#setter_value) }
        };

        if field.required {
            let req_index = self
                .required_fields
                .iter()
                .position(|f| f.ident == field.ident)
                .unwrap();

            let mut return_states = Vec::new();
            for (i, p) in self.prop_generic_idents.iter().enumerate() {
                if i == req_index {
                    return_states.push(quote! { #__view::mount::PropFixed });
                } else {
                    return_states.push(quote! { #p });
                }
            }
            let return_ty = self.get_builder_ty(&return_states);
            let setter_return_ty = if reactive_input {
                quote! { #__silex::core::SilexResult<#return_ty> }
            } else {
                return_ty.clone()
            };
            let builder_value = quote! {
                #builder_name {
                    #(#fields_destructure,)*
                    _pending_attrs,
                    _markers: ::core::marker::PhantomData,
                }
            };
            let setter_value = if vec_item_ty.is_some() {
                quote! {
                    let mut #ident = #ident.unwrap_or_default();
                    #ident.push(#setter_value);
                    let #ident = ::core::option::Option::Some(#ident);
                }
            } else {
                quote! { let #ident = #final_value; }
            };
            let return_value = if reactive_input {
                quote! { ::core::result::Result::Ok(#builder_value) }
            } else {
                builder_value
            };

            quote! {
                #[allow(non_camel_case_types, unused_variables)]
                pub fn #method #generic(self, val: #setter_param) -> #setter_return_ty #where_clause {
                    let Self {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        ..
                    } = self;

                    #setter_value

                    #return_value
                }
            }
        } else {
            let setter_value = if vec_item_ty.is_some() {
                quote! {
                    self.#ident
                        .get_or_insert_with(::std::vec::Vec::new)
                        .push(#setter_value);
                }
            } else {
                quote! { self.#ident = #final_value; }
            };
            let return_value = if reactive_input {
                quote! { ::core::result::Result::Ok(self) }
            } else {
                quote! { self }
            };
            let setter_return_ty = if reactive_input {
                quote! { #__silex::core::SilexResult<Self> }
            } else {
                quote! { Self }
            };
            quote! {
                pub fn #method #generic(mut self, val: #setter_param) -> #setter_return_ty #where_clause {
                    #setter_value
                    #return_value
                }
            }
        }
    }

    fn generate_build_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let (impl_generics, ty_generics, _) = self.generics.split_for_impl();
        let where_clause = self.ctx_where_clause();
        let product_name = &self.product_name;
        let props_name = &self.props_name;
        let product_ty = quote! { #product_name #ty_generics };
        let fallible_defaults = self.has_fallible_reactive_defaults();

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__view::mount::PropFixed })
            .collect();
        let builder_ty_fixed = self.get_builder_ty(&fixed_states);
        let fields_destructure: Vec<_> = self.fields.iter().map(|field| &field.ident).collect();

        let field_initializers = self.fields.iter().filter_map(|field| {
            let ident = &field.ident;

            if field.required {
                let name_str = ident.to_string();
                return Some(quote! {
                    let #ident = #ident.expect(concat!(
                        "Component '",
                        stringify!(#props_name),
                        "' missing required prop: '",
                        #name_str,
                        "'",
                    ));
                });
            }

            if !field.attrs.chained {
                return None;
            }

            let default_value = if is_reactive_default_field(field) {
                let ctx_field = self
                    .ctx_field
                    .as_ref()
                    .expect("reactive defaults require a ctx field");
                let init_expr = reactive_default_transform(
                    field,
                    &self.owner_lifetime,
                    ctx_field,
                    field.attrs.default_value.as_ref(),
                );
                if is_fallible_reactive_default_field(field) {
                    quote! { #init_expr? }
                } else {
                    init_expr
                }
            } else if let Some(default_expr) = &field.attrs.default_value {
                field_value_transform(field, quote! { #default_expr })
            } else {
                quote! { ::core::default::Default::default() }
            };

            Some(quote! {
                let #ident = match #ident {
                    Some(value) => value,
                    None => #default_value,
                };
            })
        });
        let props_field_inits = self.fields.iter().map(|field| {
            let ident = &field.ident;
            if field.attrs.attrs {
                quote! {
                    #ident: {
                        let _ = #ident;
                        #__view::attributes::AttributeGroup::new(_pending_attrs)
                    }
                }
            } else {
                quote! { #ident }
            }
        });
        let product_value = quote! {
            #product_name {
                props: #props_name {
                    #(#props_field_inits,)*
                },
            }
        };
        let build_return = if fallible_defaults {
            quote! { #__silex::core::SilexResult<#product_ty> }
        } else {
            product_ty.clone()
        };
        let build_value = if fallible_defaults {
            quote! {
                ::core::result::Result::Ok(#product_value)
            }
        } else {
            product_value
        };

        quote! {
            impl #impl_generics #builder_ty_fixed #where_clause {
                pub fn build(self) -> #build_return {
                    let Self {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        ..
                    } = self;

                    #(#field_initializers)*
                    #build_value
                }
            }
        }
    }

    fn generate_view_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let (impl_generics, _, _) = self.generics.split_for_impl();
        let product_name = &self.product_name;
        let (_, product_ty_generics, _) = self.generics.split_for_impl();
        let product_ty = quote! { #product_name #product_ty_generics };
        let render_fn_name = &self.render_fn_name;
        let scope = self.owner_lifetime();
        let where_clause = self.ctx_where_clause();
        let ctx_field = self
            .ctx_field
            .as_ref()
            .expect("component Props must contain a ctx field");

        let mut view_where_clause = where_clause;
        view_where_clause
            .predicates
            .push(syn::parse_quote!(#product_ty: ::core::clone::Clone));

        let mount_body = quote! {
            product.props.#ctx_field = #__silex::core::SilexContextProvider::with_error_reporter(
                product.props.#ctx_field,
                context.error_handler(),
            );
            let view_instance = #render_fn_name(product.props);
            context.mount(&view_instance)
        };

        quote! {
            impl #impl_generics #__view::mount::View<#scope> for #product_ty #view_where_clause {
                fn mount(&self, context: &#__view::mount::MountContext<#scope>) -> #__silex::core::SilexResult<#__view::mount::MountInstance<#scope>> {
                    let mut product = self.clone();
                    #mount_body
                }
            }
        }
    }

    fn generate_attribute_builder_methods(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let scope = self.owner_lifetime();

        quote! {
            fn build_attribute<__SilexValue>(mut self, target: #__view::attributes::ApplyTarget, value: __SilexValue) -> Self
            where
                __SilexValue: #__view::attributes::IntoStorable<#scope>,
            {
                self._pending_attrs.push(
                    #__view::attributes::AttrOp::<#scope>::build(
                        value.into_storable(),
                        target,
                    )
                );
                self
            }

            fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
            where
                E: #__view::events::EventDescriptor + 'static,
                F: #__view::events::EventHandler<#scope, M> + Clone + #scope,
            {
                let event = event.clone();
                self._pending_attrs.push(
                    #__view::attributes::AttrOp::<#scope>::new_scoped(move |el, context| {
                        #__view::events::bind_event(
                            context,
                            el,
                            event,
                            callback.clone(),
                            context.error_handler(),
                        )
                    })
                );
                self
            }
        }
    }

    fn generate_attribute_impl(&self) -> TokenStream2 {
        if !self.has_attrs() {
            return quote! {};
        }
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let (builder_generics_decl, _) = self.get_builder_generics();
        let builder_where_clause = self.ctx_where_clause();
        let scope = self.owner_lifetime();

        let current_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|ident| quote! { #ident })
            .collect();
        let builder_ty_current = self.get_builder_ty(&current_states);
        let methods = self.generate_attribute_builder_methods();
        let carrier_impls = self.generate_carrier_impls();
        let attrs_method = quote! {
            pub fn attrs(mut self, group: #__view::attributes::AttributeGroup<#scope>) -> Self {
                self._pending_attrs.extend(group.into_ops());
                self
            }
        };

        quote! {
            impl #builder_generics_decl #builder_ty_current #builder_where_clause {
                #attrs_method
            }

            impl #builder_generics_decl #__view::attributes::AttributeBuilder<#scope> for #builder_ty_current #builder_where_clause {
                #methods
            }

            #carrier_impls
        }
    }

    fn generate_carrier_impls(&self) -> TokenStream2 {
        let Some(html_tag) = &self.html_tag else {
            return quote! {};
        };

        let __silex = crate::crate_path::silex();
        let (builder_generics_decl, _) = self.get_builder_generics();
        let (product_impl_generics, product_ty_generics, _) = self.generics.split_for_impl();
        let builder_where_clause = self.ctx_where_clause();
        let product_where_clause = self.ctx_where_clause();
        let current_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|ident| quote! { #ident })
            .collect();
        let builder_ty_current = self.get_builder_ty(&current_states);
        let product_name = &self.product_name;
        let product_ty = quote! { #product_name #product_ty_generics };

        quote! {
            impl #builder_generics_decl #__silex::html::HtmlTagCarrier
                for #builder_ty_current #builder_where_clause
            {
                type Tag = #__silex::html::#html_tag;
            }

            impl #product_impl_generics #__silex::html::HtmlTagCarrier
                for #product_ty #product_where_clause
            {
                type Tag = #__silex::html::#html_tag;
            }
        }
    }

    fn generate_constructor(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let __view = silex_view();
        let vis = &self.vis;
        let component_name = &self.component_name;
        let (impl_generics, _, _) = self.generics.split_for_impl();
        let where_clause = self.ctx_where_clause();
        let scope = self.owner_lifetime();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__view::mount::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);

        let standalone_fields: Vec<_> = self
            .fields
            .iter()
            .filter(|f| !f.attrs.chained && !f.attrs.attrs)
            .collect();

        let constructor_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #ident: impl #__view::mount::View<#scope> + #scope }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { #ident: impl ::core::convert::Into<#ty> }
            } else {
                quote! { #ident: #ty }
            }
        });

        let constructor_args = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #__view::mount::View::into_any(#ident) }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { #ident.into() }
            } else {
                quote! { #ident }
            }
        });

        quote! {
            #[allow(non_snake_case, unused_variables, unused_mut)]
            #vis fn #component_name #impl_generics(#(#constructor_params),*) -> #builder_ty_initial #where_clause {
                <#builder_ty_initial>::new(#(#constructor_args),*)
            }
        }
    }
}

pub fn derive_props_builder_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ctx = BuilderContext::new(input)?;

    let builder_struct = ctx.generate_builder_struct();
    let product_struct = ctx.generate_product_struct();
    let builder_impl = ctx.generate_builder_impl();
    let build_impl = ctx.generate_build_impl();
    let attribute_impl = ctx.generate_attribute_impl();
    let view_impl = ctx.generate_view_impl();
    let constructor = ctx.generate_constructor();

    Ok(quote! {
        #builder_struct
        #product_struct
        #builder_impl
        #build_impl
        #attribute_impl
        #view_impl
        #constructor
    })
}

fn field_value_transform(field: &FieldSpec, input: TokenStream2) -> TokenStream2 {
    let __silex = crate::crate_path::silex();
    let __view = silex_view();
    let ty = &field.ty;
    if field.attrs.render && is_any_view_type(ty) {
        quote! { #__view::mount::View::into_any(#input) }
    } else if field.attrs.into_trait || (is_auto_into_type(ty) && !is_any_view_type(ty)) {
        quote! { ::core::convert::Into::into(#input) }
    } else {
        input
    }
}

fn chained_vec_item_type(field: &FieldSpec) -> Option<Type> {
    if !field.attrs.chain_each {
        return None;
    }

    vec_item_type(&field.ty)
}

fn vec_item_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    match arguments.args.first()? {
        GenericArgument::Type(item_ty) => Some(item_ty.clone()),
        _ => None,
    }
}

fn reactive_default_transform(
    field: &FieldSpec,
    scope: &syn::Lifetime,
    ctx_field: &Ident,
    default_value: Option<&TokenStream2>,
) -> TokenStream2 {
    let __silex = crate::crate_path::silex();
    let ty = &field.ty;

    if let Some(value_ty) = reactive_input_value_type(ty, scope) {
        return match default_value {
            Some(value) => quote! {
                {
                    use #__silex::core::ReactiveInput as _;
                    (#value).into_reactive_input(
                        #__silex::core::SilexContextProvider::owner(&#ctx_field),
                    )
                }
            },
            None => quote! {
                {
                    use #__silex::core::ReactiveInput as _;
                    <#value_ty as ::core::default::Default>::default()
                        .into_reactive_input(
                            #__silex::core::SilexContextProvider::owner(&#ctx_field),
                        )
                }
            },
        };
    }

    match default_value {
        Some(value) => quote! {
            <#ty as #__silex::core::RxFrom<#scope>>::rx_from(
                #__silex::core::SilexContextProvider::owner(&#ctx_field),
                #value,
            )
        },
        None => quote! {
            <#ty as #__silex::core::RxDefault<#scope>>::rx_default(
                #__silex::core::SilexContextProvider::owner(&#ctx_field),
            )
        },
    }
}

fn parse_component_metadata(
    attrs: &[Attribute],
    props_name: &Ident,
) -> syn::Result<Option<ComponentMetadata>> {
    let mut metadata_attr = None;
    for attr in attrs {
        if attr.path().is_ident("silex_component") {
            if metadata_attr.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "duplicate `silex_component` metadata attribute",
                ));
            }
            metadata_attr = Some(attr);
        }
    }

    let Some(attr) = metadata_attr else {
        return Ok(None);
    };

    let mut builder_name = None;
    let mut product_name = None;
    let mut render_fn_name = None;
    let mut constructor_name = None;
    let mut html_tag = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("builder") {
            if builder_name.is_some() {
                return Err(meta.error("duplicate `builder` metadata entry"));
            }
            builder_name = Some(meta.value()?.parse::<Ident>()?);
        } else if meta.path.is_ident("product") {
            if product_name.is_some() {
                return Err(meta.error("duplicate `product` metadata entry"));
            }
            product_name = Some(meta.value()?.parse::<Ident>()?);
        } else if meta.path.is_ident("render") {
            if render_fn_name.is_some() {
                return Err(meta.error("duplicate `render` metadata entry"));
            }
            render_fn_name = Some(meta.value()?.parse::<Ident>()?);
        } else if meta.path.is_ident("constructor") {
            if constructor_name.is_some() {
                return Err(meta.error("duplicate `constructor` metadata entry"));
            }
            constructor_name = Some(meta.value()?.parse::<Ident>()?);
        } else if meta.path.is_ident("tag") {
            if html_tag.is_some() {
                return Err(meta.error("duplicate `tag` metadata entry"));
            }
            html_tag = Some(meta.value()?.parse::<Ident>()?);
        } else {
            return Err(
                meta.error("expected `builder`, `product`, `render`, `constructor`, or `tag`")
            );
        }
        Ok(())
    })?;

    let Some(builder_name) = builder_name else {
        return Err(syn::Error::new_spanned(
            attr,
            format!(
                "`silex_component` metadata for `{props_name}` requires `builder`; omit the attribute for the standalone fallback (`{props_name}Builder`, `<component>Component`, and `__silex_render_<component>`)",
            ),
        ));
    };
    let Some(product_name) = product_name else {
        return Err(syn::Error::new_spanned(
            attr,
            format!(
                "`silex_component` metadata for `{props_name}` requires `product`; omit the attribute for the standalone fallback (`{props_name}Builder`, `<component>Component`, and `__silex_render_<component>`)",
            ),
        ));
    };
    let Some(render_fn_name) = render_fn_name else {
        return Err(syn::Error::new_spanned(
            attr,
            format!(
                "`silex_component` metadata for `{props_name}` requires `render`; omit the attribute for the standalone fallback (`{props_name}Builder`, `<component>Component`, and `__silex_render_<component>`)",
            ),
        ));
    };

    Ok(Some(ComponentMetadata {
        builder_name,
        product_name,
        render_fn_name,
        constructor_name,
        html_tag,
    }))
}

fn parse_field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut result = FieldAttrs::default();

    for attr in attrs {
        if attr.path().is_ident("prop") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("into") {
                    result.into_trait = true;
                    Ok(())
                } else if meta.path.is_ident("render") {
                    result.render = true;
                    Ok(())
                } else if meta.path.is_ident("render_fn") {
                    if result.render_fn_args.is_some() {
                        return Err(meta.error("duplicate `render_fn`"));
                    }
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let args = content.parse_terminated(Type::parse, syn::Token![,])?;
                    if args.is_empty() {
                        return Err(meta.error("`render_fn` requires closure parameter types"));
                    }
                    result.render_fn_args = Some(args.into_iter().collect());
                    Ok(())
                } else if meta.path.is_ident("default") {
                    Err(meta.error("`default` is no longer supported in `#[prop]`, please use `#[chain(default)]` or `#[chain(default = ...)]` instead"))
                } else {
                    Err(meta.error("expected `into`, `render`, or `render_fn`"))
                }
            })?;
        } else if attr.path().is_ident("chain") {
            result.chained = true;
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("default") {
                        result.default = true;
                        if meta.input.peek(syn::Token![=]) {
                            meta.input.parse::<syn::Token![=]>()?;
                            let expr: syn::Expr = meta.input.parse()?;
                            result.default_value = Some(quote! { #expr });
                        }
                        Ok(())
                    } else if meta.path.is_ident("name") {
                        if result.chain_method.is_some() {
                            return Err(meta.error("duplicate chain method name"));
                        }
                        let value = meta.value()?.parse::<syn::Expr>()?;
                        let method = match value {
                            syn::Expr::Path(path)
                                if path.qself.is_none() && path.path.segments.len() == 1 =>
                            {
                                path.path.segments.first().unwrap().ident.clone()
                            }
                            syn::Expr::Lit(expr) => match expr.lit {
                                syn::Lit::Str(value) => syn::parse_str::<Ident>(&value.value())
                                    .map_err(|_| {
                                        meta.error("chain method name must be a valid identifier")
                                    })?,
                                _ => {
                                    return Err(meta.error(
                                        "chain method name must be an identifier or string literal",
                                    ));
                                }
                            },
                            _ => {
                                return Err(meta.error(
                                    "chain method name must be an identifier or string literal",
                                ));
                            }
                        };
                        result.chain_method = Some(method);
                        Ok(())
                    } else if meta.path.is_ident("each") {
                        if result.chain_each {
                            return Err(meta.error("duplicate `each` chain option"));
                        }
                        result.chain_each = true;
                        Ok(())
                    } else {
                        Err(meta.error("expected `default`, `name`, or `each`"))
                    }
                })?;
            }
        } else if attr.path().is_ident("ctx") {
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "`#[ctx]` does not accept arguments",
                ));
            }
            result.ctx = true;
        } else if attr.path().is_ident("attrs") {
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "`#[attrs]` does not accept arguments",
                ));
            }
            result.attrs = true;
        }
    }

    if result.render_fn_args.is_some() && (result.into_trait || result.render) {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`render_fn` cannot be combined with `into` or `render`",
        ));
    }

    Ok(result)
}

fn is_attribute_group_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "AttributeGroup")
}

fn strip_props_suffix(name: &Ident) -> Ident {
    let name_str = name.to_string();
    if let Some(stripped) = name_str.strip_suffix("Props") {
        format_ident!("{}", stripped)
    } else {
        name.clone()
    }
}

fn to_upper_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn type_last_segment_name(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        return type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
    }
    None
}

fn is_any_view_type(ty: &Type) -> bool {
    type_last_segment_name(ty).is_some_and(|ident| ident == "AnyView")
}

fn is_reactive_wrapper_type(ty: &Type) -> bool {
    matches!(
        type_last_segment_name(ty).as_deref(),
        Some("Rx")
            | Some("ReadSignal")
            | Some("Signal")
            | Some("Computed")
            | Some("StoredValue")
            | Some("Callback")
            | Some("NodeRef")
    )
}

fn is_reactive_input_type(ty: &Type, scope: &syn::Lifetime) -> bool {
    reactive_input_value_type(ty, scope).is_some()
}

fn reactive_input_value_type(ty: &Type, scope: &syn::Lifetime) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }

    let segment = type_path.path.segments.last()?;
    if !matches!(
        segment.ident.to_string().as_str(),
        "Rx" | "ReadSignal" | "Signal" | "Computed" | "StoredValue"
    ) {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 2 {
        return None;
    }

    let mut args = arguments.args.iter();
    let Some(GenericArgument::Lifetime(lifetime)) = args.next() else {
        return None;
    };
    if lifetime.ident != scope.ident {
        return None;
    }

    match args.next() {
        Some(GenericArgument::Type(value)) => Some(value.clone()),
        _ => None,
    }
}

fn is_reactive_default_field(field: &FieldSpec) -> bool {
    field.attrs.chained
        && (field.attrs.default || field.attrs.default_value.is_some())
        && is_reactive_wrapper_type(&field.ty)
}

fn is_fallible_reactive_default_field(field: &FieldSpec) -> bool {
    is_reactive_default_field(field)
}

fn owner_lifetime(generics: &syn::Generics, fields: &[FieldSpec]) -> syn::Lifetime {
    struct LifetimeVisitor {
        names: std::collections::HashSet<String>,
    }

    impl<'ast> Visit<'ast> for LifetimeVisitor {
        fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
            self.names.insert(lifetime.ident.to_string());
        }
    }

    let mut ctx_names = std::collections::HashSet::new();
    for field in fields.iter().filter(|field| field.attrs.ctx) {
        let mut visitor = LifetimeVisitor {
            names: std::collections::HashSet::new(),
        };
        visitor.visit_type(&field.ty);
        ctx_names.extend(visitor.names);
    }

    let mut field_names = std::collections::HashSet::new();
    for field in fields {
        let mut visitor = LifetimeVisitor {
            names: std::collections::HashSet::new(),
        };
        visitor.visit_type(&field.ty);
        field_names.extend(visitor.names);
    }

    let lifetime = |names: &std::collections::HashSet<String>| {
        generics.params.iter().find_map(|param| match param {
            syn::GenericParam::Lifetime(def) if names.contains(&def.lifetime.ident.to_string()) => {
                Some(def.lifetime.clone())
            }
            _ => None,
        })
    };

    lifetime(&ctx_names)
        .or_else(|| lifetime(&field_names))
        .or_else(|| {
            generics.params.iter().find_map(|param| match param {
                syn::GenericParam::Lifetime(def) if def.lifetime.ident == "owner" => {
                    Some(def.lifetime.clone())
                }
                _ => None,
            })
        })
        .or_else(|| {
            generics.params.iter().find_map(|param| match param {
                syn::GenericParam::Lifetime(def) => Some(def.lifetime.clone()),
                _ => None,
            })
        })
        .unwrap_or_else(|| syn::Lifetime::new("'static", proc_macro2::Span::call_site()))
}

fn is_owner_marker_field(field: &FieldSpec) -> bool {
    field.ident == "__silex_owner_marker"
}

fn is_internal_marker_field(field: &FieldSpec) -> bool {
    is_owner_marker_field(field) || field.ident == "__silex_generic_marker"
}

fn is_auto_into_type(ty: &Type) -> bool {
    matches!(
        type_last_segment_name(ty).as_deref(),
        Some("AnyView")
            | Some("String")
            | Some("PathBuf")
            | Some("Callback")
            | Some("Rx")
            | Some("ReadSignal")
            | Some("Signal")
            | Some("Computed")
    )
}
