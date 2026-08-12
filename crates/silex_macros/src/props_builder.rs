use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type, Visibility,
    parse::Parse,
};

#[derive(Clone, Default)]
struct FieldAttrs {
    default: bool,
    default_value: Option<TokenStream2>,
    into_trait: bool,
    render: bool,
    render_fn_args: Option<Vec<Type>>,
    chained: bool,
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
    scope: syn::Lifetime,
    scope_field: Option<Ident>,
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
        let (builder_name, product_name, render_fn_name, component_name) =
            if let Some(metadata) = metadata {
                (
                    metadata.builder_name,
                    metadata.product_name,
                    metadata.render_fn_name,
                    metadata
                        .constructor_name
                        .unwrap_or_else(|| inferred_component_name.clone()),
                )
            } else {
                (
                    format_ident!("{}Builder", props_name),
                    format_ident!("{}Component", inferred_component_name),
                    format_ident!("__silex_render_{}", inferred_component_name),
                    inferred_component_name,
                )
            };
        let scope = generics
            .params
            .iter()
            .find_map(|param| match param {
                syn::GenericParam::Lifetime(lifetime) if lifetime.lifetime.ident == "scope" => {
                    Some(lifetime.lifetime.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| syn::Lifetime::new("'static", proc_macro2::Span::call_site()));

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

        let scope_field = fields
            .iter()
            .find(|field| {
                !field.attrs.chained && field.ident == "scope" && is_scope_type(&field.ty, &scope)
            })
            .map(|field| field.ident.clone());

        if let Some(field) = fields.iter().find(|field| is_reactive_default_field(field))
            && scope_field.is_none()
        {
            return Err(syn::Error::new_spanned(
                &field.ident,
                "RxDefault requires an explicit `scope: Scope<'scope>` parameter; scoped reactive defaults cannot create an implicit runtime",
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
            scope,
            scope_field,
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

    fn scope_lifetime(&self) -> syn::Lifetime {
        self.scope.clone()
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
        let scope = self.scope_lifetime();
        quote! { #__silex::dom::attribute::PendingAttribute<#scope> }
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
        let __silex = crate::crate_path::silex();
        let vis = &self.vis;
        let product_name = &self.product_name;
        let props_name = &self.props_name;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let pending_attribute_ty = self.pending_attribute_ty();

        quote! {
            #[derive(Clone)]
            #[allow(non_camel_case_types)]
            #vis struct #product_name #impl_generics #where_clause {
                props: #props_name #ty_generics,
                _pending_attrs: ::std::vec::Vec<#pending_attribute_ty>,
            }
        }
    }

    fn generate_builder_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (builder_generics_decl, builder_generics_type) = self.get_builder_generics();
        let builder_name = &self.builder_name;
        let (_, _, where_clause) = self.generics.split_for_impl();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);

        let standalone_fields: Vec<_> = self.fields.iter().filter(|f| !f.attrs.chained).collect();
        let builder_new_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        });

        let builder_field_inits = self.fields.iter().map(|field| {
            let ident = &field.ident;
            if !field.attrs.chained {
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
                    && self
                        .scope_field
                        .as_ref()
                        .map(|scope_field| scope_field != &field.ident)
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
        let builder_name = &self.builder_name;
        let ident = &field.ident;
        let ty = &field.ty;
        let scope = self.scope_lifetime();
        let reactive_input = self.scope_field.is_some() && is_reactive_input_type(ty, &scope);
        let scope_field = self.scope_field.as_ref();
        let setter_generic = self.reactive_input_generic_ident();
        let scope_expr = if reactive_input {
            let scope_field =
                scope_field.expect("reactive input setters were validated to have a scope field");
            if field.required {
                quote! { #scope_field }
            } else {
                quote! { self.#scope_field }
            }
        } else {
            quote! {}
        };

        let fields_destructure: Vec<_> = self.fields.iter().map(|f| &f.ident).collect();

        let (setter_param, setter_value, generic, where_clause) = if let Some(render_fn_args) =
            field.attrs.render_fn_args.as_deref()
        {
            let render_fn = self.fresh_generic_ident("__SilexRenderFn", &[]);
            let render_view = self.fresh_generic_ident("__SilexRenderView", &[&render_fn]);
            (
                quote! { #render_fn },
                quote! { <#ty>::from_fn(val) },
                quote! { <#render_fn, #render_view> },
                quote! {
                    where
                        #render_fn: Fn(#(#render_fn_args),*) -> #render_view + #scope,
                        #render_view: #__silex::dom::view::View<#scope> + #scope,
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
                quote! { impl #__silex::dom::view::View<#scope> + #scope }
            } else if field.attrs.render {
                quote! { #ty }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { impl ::core::convert::Into<#ty> }
            } else {
                quote! { #ty }
            };
            let setter_value = if is_any_view_type(ty) {
                quote! { val.into_any() }
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
                    return_states.push(quote! { #__silex::dom::view::PropFixed });
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
            let return_value = if reactive_input {
                quote! { ::core::result::Result::Ok(#builder_value) }
            } else {
                builder_value
            };

            quote! {
                #[allow(non_camel_case_types, unused_variables)]
                pub fn #ident #generic(self, val: #setter_param) -> #setter_return_ty #where_clause {
                    let Self {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        ..
                    } = self;

                    let #ident = #final_value;

                    #return_value
                }
            }
        } else {
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
                pub fn #ident #generic(mut self, val: #setter_param) -> #setter_return_ty #where_clause {
                    self.#ident = #final_value;
                    #return_value
                }
            }
        }
    }

    fn generate_build_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let product_name = &self.product_name;
        let props_name = &self.props_name;
        let product_ty = quote! { #product_name #ty_generics };
        let fallible_defaults = self.has_fallible_reactive_defaults();

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropFixed })
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
                let scope_field = self
                    .scope_field
                    .as_ref()
                    .expect("reactive defaults were validated to have a scope field");
                let init_expr = reactive_default_transform(
                    field,
                    &self.scope,
                    scope_field,
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
            quote! { #ident }
        });
        let product_value = quote! {
            #product_name {
                props: #props_name {
                    #(#props_field_inits,)*
                },
                _pending_attrs,
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
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let product_name = &self.product_name;
        let (_, product_ty_generics, _) = self.generics.split_for_impl();
        let product_ty = quote! { #product_name #product_ty_generics };
        let render_fn_name = &self.render_fn_name;
        let scope = self.scope_lifetime();
        let pending_attribute_ty = self.pending_attribute_ty();

        let mut view_where_clause: syn::WhereClause = match where_clause {
            Some(clause) => clause.clone(),
            None => syn::parse_quote!(where),
        };
        view_where_clause
            .predicates
            .push(syn::parse_quote!(#product_ty: ::core::clone::Clone));

        let mount_body = quote! {
            let view_instance = #render_fn_name(self.props);
            #__silex::dom::view::View::mount_owned(
                view_instance,
                owner,
                parent,
                pending_attrs,
                error_handler,
            )
        };

        quote! {
            impl #impl_generics #__silex::dom::view::View<#scope> for #product_ty #view_where_clause {
                fn mount(
                    &self,
                    owner: &dyn #__silex::dom::view::ViewOwner<#scope>,
                    parent: &#__silex::reexports::web_sys::Node,
                    attrs: ::std::vec::Vec<#pending_attribute_ty>,
                    error_handler: #__silex::dom::view::ViewErrorHandler<#scope>,
                ) -> #__silex::core::SilexResult<()> {
                    self.clone().mount_owned(owner, parent, attrs, error_handler)
                }

                fn mount_owned(
                    mut self,
                    owner: &dyn #__silex::dom::view::ViewOwner<#scope>,
                    parent: &#__silex::reexports::web_sys::Node,
                    attrs: ::std::vec::Vec<#pending_attribute_ty>,
                    error_handler: #__silex::dom::view::ViewErrorHandler<#scope>,
                ) -> #__silex::core::SilexResult<()>
                where
                    Self: Sized,
                {
                    self._pending_attrs.extend(attrs);
                    let pending_attrs = self._pending_attrs;
                    #mount_body
                }
            }
        }
    }

    fn generate_attribute_builder_methods(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let scope = self.scope_lifetime();

        quote! {
            fn build_attribute<__SilexValue>(mut self, target: #__silex::dom::attribute::ApplyTarget, value: __SilexValue) -> Self
            where
                __SilexValue: #__silex::dom::attribute::IntoStorable<#scope>,
            {
                self._pending_attrs.push(
                    #__silex::dom::attribute::AttrOp::<#scope>::build(
                        value.into_storable(),
                        target,
                    )
                );
                self
            }

            fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
            where
                E: #__silex::dom::event::EventDescriptor + 'static,
                F: #__silex::dom::event::EventHandler<#scope, E::EventType, M> + Clone + #scope,
            {
                let event = event.clone();
                self._pending_attrs.push(
                    #__silex::dom::attribute::AttrOp::<#scope>::new_scoped(move |el, owner, error_handler| {
                        #__silex::dom::element::bind_event(el, event, callback.clone(), owner, error_handler)
                    })
                );
                self
            }
        }
    }

    fn generate_attribute_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (builder_generics_decl, _) = self.get_builder_generics();
        let (product_impl_generics, product_ty_generics, product_where_clause) =
            self.generics.split_for_impl();
        let (_, _, builder_where_clause) = self.generics.split_for_impl();
        let product_name = &self.product_name;
        let scope = self.scope_lifetime();

        let current_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|ident| quote! { #ident })
            .collect();
        let builder_ty_current = self.get_builder_ty(&current_states);
        let product_ty = quote! { #product_name #product_ty_generics };
        let methods = self.generate_attribute_builder_methods();

        quote! {
            impl #builder_generics_decl #__silex::dom::attribute::AttributeBuilder<#scope> for #builder_ty_current #builder_where_clause {
                #methods
            }

            impl #product_impl_generics #__silex::dom::attribute::AttributeBuilder<#scope> for #product_ty #product_where_clause {
                #methods
            }

            impl #product_impl_generics #__silex::dom::view::ApplyAttributes<#scope> for #product_ty #product_where_clause {
                fn apply_attributes(&mut self, attrs: ::std::vec::Vec<#__silex::dom::attribute::PendingAttribute<#scope>>) {
                    self._pending_attrs.extend(attrs);
                }
            }
        }
    }

    fn generate_constructor(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let vis = &self.vis;
        let component_name = &self.component_name;
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let scope = self.scope_lifetime();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);

        let standalone_fields: Vec<_> = self.fields.iter().filter(|f| !f.attrs.chained).collect();

        let constructor_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #ident: impl #__silex::dom::view::View<#scope> + #scope }
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
                quote! { #ident.into_any() }
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
    let ty = &field.ty;
    if field.attrs.render && is_any_view_type(ty) {
        quote! { #__silex::dom::view::View::into_any(#input) }
    } else if field.attrs.into_trait || (is_auto_into_type(ty) && !is_any_view_type(ty)) {
        quote! { ::core::convert::Into::into(#input) }
    } else {
        input
    }
}

fn reactive_default_transform(
    field: &FieldSpec,
    scope: &syn::Lifetime,
    scope_field: &Ident,
    default_value: Option<&TokenStream2>,
) -> TokenStream2 {
    let __silex = crate::crate_path::silex();
    let ty = &field.ty;

    if let Some(value_ty) = reactive_input_value_type(ty, scope) {
        return match default_value {
            Some(value) => quote! {
                {
                    use #__silex::core::ReactiveInput as _;
                    (#value).into_reactive_input(#scope_field)
                }
            },
            None => quote! {
                {
                    use #__silex::core::ReactiveInput as _;
                    <#value_ty as ::core::default::Default>::default()
                        .into_reactive_input(#scope_field)
                }
            },
        };
    }

    match default_value {
        Some(value) => quote! {
            <#ty as #__silex::core::RxFrom<#scope>>::rx_from(#scope_field, #value)
        },
        None => quote! {
            <#ty as #__silex::core::RxDefault<#scope>>::rx_default(#scope_field)
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
        } else {
            return Err(meta.error("expected `builder`, `product`, `render`, or `constructor`"));
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
                    } else {
                        Err(meta.error("expected `default`"))
                    }
                })?;
            }
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
        Some("Signal")
            | Some("ReadSignal")
            | Some("RwSignal")
            | Some("Memo")
            | Some("StoredValue")
            | Some("Rx")
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
        "Signal" | "ReadSignal" | "RwSignal" | "Memo" | "StoredValue" | "Rx"
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

fn is_scope_marker_field(field: &FieldSpec) -> bool {
    field.ident == "__silex_scope_marker"
}

fn is_internal_marker_field(field: &FieldSpec) -> bool {
    is_scope_marker_field(field) || field.ident == "__silex_generic_marker"
}

fn is_scope_type(ty: &Type, scope: &syn::Lifetime) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Scope" {
        return false;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    if arguments.args.len() != 1 {
        return false;
    }

    matches!(
        arguments.args.first(),
        Some(GenericArgument::Lifetime(lifetime)) if lifetime.ident == scope.ident
    )
}

fn is_auto_into_type(ty: &Type) -> bool {
    matches!(
        type_last_segment_name(ty).as_deref(),
        Some("AnyView")
            | Some("String")
            | Some("PathBuf")
            | Some("Callback")
            | Some("Signal")
            | Some("ReadSignal")
            | Some("RwSignal")
            | Some("Memo")
    )
}
