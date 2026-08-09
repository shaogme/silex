use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type, Visibility,
};

#[derive(Clone, Default)]
struct FieldAttrs {
    default: bool,
    default_value: Option<TokenStream2>,
    into_trait: bool,
    render: bool,
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

struct BuilderContext {
    vis: Visibility,
    props_name: Ident,
    builder_name: Ident,
    component_name: Ident,
    component_component_alias: Ident,
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
            data,
            ..
        } = input;

        let builder_name = format_ident!("{}Builder", props_name);
        let component_name = strip_props_suffix(&props_name);
        let component_component_alias = format_ident!("{}Component", component_name);
        let render_fn_name = format_ident!("__silex_render_{}", component_name);
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
            component_component_alias,
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
            if field.required {
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

    fn generate_builder_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (builder_generics_decl, builder_generics_type) = self.get_builder_generics();
        let builder_name = &self.builder_name;
        let props_name = &self.props_name;
        let (_, ty_generics, where_clause) = self.generics.split_for_impl();
        let pending_attribute_ty = self.pending_attribute_ty();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);
        let fallible_defaults = self.has_fallible_reactive_defaults();

        let standalone_fields: Vec<_> = self.fields.iter().filter(|f| !f.attrs.chained).collect();
        let builder_new_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        });

        let scope_field = self.scope_field.as_ref();
        let builder_field_inits = self.fields.iter().map(|field| {
            let ident = &field.ident;
            if !field.attrs.chained {
                quote! { #ident }
            } else if is_reactive_default_field(field) {
                let scope_field =
                    scope_field.expect("reactive defaults were validated to have a scope field");
                let init_expr = reactive_default_transform(
                    field,
                    &self.scope,
                    scope_field,
                    field.attrs.default_value.as_ref(),
                );
                let init_expr = if is_fallible_reactive_default_field(field) {
                    quote! { #init_expr? }
                } else {
                    init_expr
                };
                quote! { #ident: #init_expr }
            } else if let Some(default_expr) = &field.attrs.default_value {
                let init_expr = field_value_transform(field, quote! { #default_expr });
                quote! { #ident: #init_expr }
            } else if field.attrs.default {
                quote! { #ident: ::core::default::Default::default() }
            } else if field.required {
                quote! { #ident: ::core::option::Option::None }
            } else {
                quote! { #ident: ::core::default::Default::default() }
            }
        });

        let builder_value = quote! {
            #builder_name {
                #(#builder_field_inits,)*
                _pending_attrs: ::std::vec::Vec::new(),
                _markers: ::core::marker::PhantomData,
            }
        };
        let builder_new_return = if fallible_defaults {
            quote! { #__silex::core::SilexResult<#builder_ty_initial> }
        } else {
            quote! { #builder_ty_initial }
        };
        let builder_new_value = if fallible_defaults {
            quote! { ::core::result::Result::Ok(#builder_value) }
        } else {
            builder_value
        };

        let fields_destructure = self.fields.iter().map(|f| &f.ident);
        let props_field_inits = self.fields.iter().map(|field| {
            let ident = &field.ident;
            if field.required {
                let name_str = ident.to_string();
                quote! {
                    #ident: #ident.expect(concat!("Component '", stringify!(#props_name), "' missing required prop: '", #name_str, "'"))
                }
            } else {
                quote! { #ident }
            }
        });

        let builder_setters = self
            .fields
            .iter()
            .filter(|field| {
                !is_scope_marker_field(field)
                    && self
                        .scope_field
                        .as_ref()
                        .map(|scope_field| scope_field != &field.ident)
                        .unwrap_or(true)
            })
            .map(|f| self.generate_setter(f));

        quote! {
            impl #builder_generics_decl #builder_name #builder_generics_type #where_clause {
                pub fn new(#(#builder_new_params),*) -> #builder_new_return {
                    #builder_new_value
                }

                pub fn into_parts(self) -> (#props_name #ty_generics, ::std::vec::Vec<#pending_attribute_ty>) {
                    let Self {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        ..
                    } = self;

                    (
                        #props_name {
                            #(#props_field_inits,)*
                        },
                        _pending_attrs,
                    )
                }

                pub fn build(self) -> #props_name #ty_generics {
                    self.into_parts().0
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

        let setter_param = if reactive_input {
            quote! { #setter_generic }
        } else if is_any_view_type(ty) {
            quote! { impl #__silex::dom::view::View<#scope> + #scope }
        } else if field.attrs.render {
            quote! { #ty }
        } else if field.attrs.into_trait || is_auto_into_type(ty) {
            quote! { impl ::core::convert::Into<#ty> }
        } else {
            quote! { #ty }
        };

        let setter_value = if reactive_input {
            quote! {
                <#setter_generic as #__silex::core::ReactiveInput<#scope, #ty>>::into_reactive_input(
                    val,
                    #scope_expr,
                )
            }
        } else if is_any_view_type(ty) {
            quote! { val.into_any() }
        } else if field.attrs.into_trait || is_auto_into_type(ty) {
            quote! { val.into() }
        } else {
            quote! { val }
        };

        let final_value = if !field.attrs.chained || !field.required {
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

            let generic = if reactive_input {
                quote! { <#setter_generic> }
            } else {
                quote! {}
            };
            let where_clause = if reactive_input {
                quote! {
                    where
                        #setter_generic: #__silex::core::ReactiveInput<#scope, #ty>
                }
            } else {
                quote! {}
            };

            quote! {
                #[allow(non_camel_case_types, unused_variables)]
                pub fn #ident #generic(self, val: #setter_param) -> #return_ty #where_clause {
                    let Self {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        ..
                    } = self;

                    let #ident = #final_value;

                    #builder_name {
                        #(#fields_destructure,)*
                        _pending_attrs,
                        _markers: ::core::marker::PhantomData,
                    }
                }
            }
        } else {
            let generic = if reactive_input {
                quote! { <#setter_generic> }
            } else {
                quote! {}
            };
            let where_clause = if reactive_input {
                quote! {
                    where
                        #setter_generic: #__silex::core::ReactiveInput<#scope, #ty>
                }
            } else {
                quote! {}
            };

            quote! {
                pub fn #ident #generic(mut self, val: #setter_param) -> Self #where_clause {
                    self.#ident = #final_value;
                    self
                }
            }
        }
    }

    fn generate_view_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let render_fn_name = &self.render_fn_name;
        let scope = self.scope_lifetime();
        let pending_attribute_ty = self.pending_attribute_ty();

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropFixed })
            .collect();
        let builder_ty_fixed = self.get_builder_ty(&fixed_states);

        let mut view_where_clause: syn::WhereClause = match where_clause {
            Some(clause) => clause.clone(),
            None => syn::parse_quote!(where),
        };
        view_where_clause
            .predicates
            .push(syn::parse_quote!(#builder_ty_fixed: ::core::clone::Clone));

        quote! {
            impl #impl_generics #__silex::dom::view::View<#scope> for #builder_ty_fixed #view_where_clause {
                fn mount(
                    &self,
                    owner: &dyn #__silex::dom::view::ViewOwner<#scope>,
                    parent: &#__silex::reexports::web_sys::Node,
                    attrs: ::std::vec::Vec<#pending_attribute_ty>,
                ) -> #__silex::core::SilexResult<()> {
                    self.clone().mount_owned(owner, parent, attrs)
                }

                fn mount_owned(
                    self,
                    owner: &dyn #__silex::dom::view::ViewOwner<#scope>,
                    parent: &#__silex::reexports::web_sys::Node,
                    attrs: ::std::vec::Vec<#pending_attribute_ty>,
                ) -> #__silex::core::SilexResult<()>
                where
                    Self: Sized,
                {
                    let (props, mut pending_attrs) = self.into_parts();
                    pending_attrs.extend(attrs);
                    let view_instance = #render_fn_name(props);
                    #__silex::dom::view::View::mount_owned(
                        view_instance,
                        owner,
                        parent,
                        pending_attrs,
                    )
                }
            }
        }
    }

    fn generate_attribute_impl(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let (builder_generics_decl, _) = self.get_builder_generics();
        let (_, _, where_clause) = self.generics.split_for_impl();
        let scope = self.scope_lifetime();

        let current_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|ident| quote! { #ident })
            .collect();
        let builder_ty_current = self.get_builder_ty(&current_states);

        quote! {
            impl #builder_generics_decl #__silex::dom::attribute::AttributeBuilder<#scope> for #builder_ty_current #where_clause {
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
                        #__silex::dom::attribute::AttrOp::<#scope>::new_scoped(move |el, owner| {
                            #__silex::dom::element::bind_event(el, event, callback.clone(), owner)
                        })
                    );
                    self
                }
            }

            impl #builder_generics_decl #__silex::dom::view::ApplyAttributes<#scope> for #builder_ty_current #where_clause {}
        }
    }

    fn generate_constructor(&self) -> TokenStream2 {
        let __silex = crate::crate_path::silex();
        let vis = &self.vis;
        let component_name = &self.component_name;
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let component_component_alias = &self.component_component_alias;
        let scope = self.scope_lifetime();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);
        let fallible_defaults = self.has_fallible_reactive_defaults();

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { #__silex::dom::view::PropFixed })
            .collect();
        let builder_ty_fixed = self.get_builder_ty(&fixed_states);

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

        let constructor_return = if fallible_defaults {
            quote! { #__silex::core::SilexResult<#builder_ty_initial> }
        } else {
            quote! { #builder_ty_initial }
        };

        quote! {
            #[allow(non_camel_case_types)]
            #[allow(type_alias_bounds)]
            #vis type #component_component_alias #impl_generics = #builder_ty_fixed;

            #[allow(non_snake_case, unused_variables, unused_mut)]
            #vis fn #component_name #impl_generics(#(#constructor_params),*) -> #constructor_return #where_clause {
                <#builder_ty_initial>::new(#(#constructor_args),*)
            }
        }
    }
}

pub fn derive_props_builder_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ctx = BuilderContext::new(input)?;

    let builder_struct = ctx.generate_builder_struct();
    let builder_impl = ctx.generate_builder_impl();
    let attribute_impl = ctx.generate_attribute_impl();
    let view_impl = ctx.generate_view_impl();
    let constructor = ctx.generate_constructor();

    Ok(quote! {
        #builder_struct
        #builder_impl
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

    if is_fallible_reactive_default_field(field) {
        return match default_value {
            Some(value) => quote! {
                <#ty as #__silex::core::TryRxFrom<#scope>>::try_rx_from(
                    #scope_field,
                    #value,
                )
            },
            None => quote! {
                <#ty as #__silex::core::TryRxDefault<#scope>>::try_rx_default(#scope_field)
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
                } else if meta.path.is_ident("default") {
                    Err(meta.error("`default` is no longer supported in `#[prop]`, please use `#[chain(default)]` or `#[chain(default = ...)]` instead"))
                } else {
                    Err(meta.error("expected `into` or `render`"))
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
        && type_last_segment_name(&field.ty).is_some_and(|ident| ident == "Callback")
}

fn is_scope_marker_field(field: &FieldSpec) -> bool {
    field.ident == "__silex_scope_marker"
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
