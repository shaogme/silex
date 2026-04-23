use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type};

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

fn get_builder_ty(
    builder_name: &Ident,
    prop_states: &[TokenStream2],
    generics: &syn::Generics,
) -> TokenStream2 {
    let mut params = Vec::new();
    // 1. Lifetimes must come first
    for param in &generics.params {
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
    for param in &generics.params {
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
    if params.is_empty() {
        quote! { #builder_name }
    } else {
        quote! { #builder_name <#(#params),*> }
    }
}

pub fn derive_props_builder_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
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

    let fields = match data {
        Data::Struct(ref data) => match &data.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .map(|field| {
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
                })
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

    let standalone_fields: Vec<_> = fields.iter().filter(|f| !f.attrs.chained).collect();

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let builder_fields = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.required {
            quote! { #ident: ::core::option::Option<#ty> }
        } else {
            quote! { #ident: #ty }
        }
    });

    let builder_field_inits = fields.iter().map(|field| {
        let ident = &field.ident;
        if !field.attrs.chained {
            quote! { #ident }
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

    let required_fields: Vec<_> = fields.iter().filter(|f| f.required).collect();
    let prop_generic_idents: Vec<_> = required_fields
        .iter()
        .map(|f| {
            let name = to_upper_camel_case(&f.ident.to_string());
            format_ident!("P{}", name)
        })
        .collect();

    let builder_setters = fields.iter().map(|f| {
        generate_setter(
            f,
            &builder_name,
            &prop_generic_idents,
            &required_fields,
            &generics,
            &fields,
        )
    });

    let builder_new_params: Vec<_> = standalone_fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! { #ident: #ty }
        })
        .collect();

    let constructor_params: Vec<_> = standalone_fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #ident: impl ::silex::dom::view::View + 'static }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { #ident: impl ::core::convert::Into<#ty> }
            } else {
                quote! { #ident: #ty }
            }
        })
        .collect();

    let constructor_args: Vec<_> = standalone_fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #ident.into_any() }
            } else if field.attrs.into_trait || is_auto_into_type(ty) {
                quote! { #ident.into() }
            } else {
                quote! { #ident }
            }
        })
        .collect();

    let fields_destructure: Vec<_> = fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            quote! { #ident }
        })
        .collect();

    let props_field_inits: Vec<_> = fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            if field.required {
                let name_str = ident.to_string();
                quote! {
                    #ident: #ident.expect(concat!("Component '", stringify!(#props_name), "' missing required prop: '", #name_str, "'"))
                }
            } else {
                quote! { #ident }
            }
        })
        .collect();

    let mut builder_decl_params = Vec::new();
    let mut builder_type_params = Vec::new();

    // 1. Lifetimes MUST come first in generic parameter lists
    for param in &generics.params {
        if let syn::GenericParam::Lifetime(_) = param {
            builder_decl_params.push(quote! { #param });
            if let syn::GenericParam::Lifetime(l) = param {
                let lifetime = &l.lifetime;
                builder_type_params.push(quote! { #lifetime });
            }
        }
    }

    // 2. Then our prop generic parameters (which are types)
    for ident in &prop_generic_idents {
        builder_decl_params.push(quote! { #ident });
        builder_type_params.push(quote! { #ident });
    }

    // 3. Then original type and const parameters
    for param in &generics.params {
        match param {
            syn::GenericParam::Type(t) => {
                builder_decl_params.push(quote! { #param });
                let ident = &t.ident;
                builder_type_params.push(quote! { #ident });
            }
            syn::GenericParam::Const(c) => {
                builder_decl_params.push(quote! { #param });
                let ident = &c.ident;
                builder_type_params.push(quote! { #ident });
            }
            _ => {} // Lifetimes already handled
        }
    }

    let builder_generics_decl = if builder_decl_params.is_empty() {
        quote! {}
    } else {
        quote! { <#(#builder_decl_params),*> }
    };
    let builder_generics_type = if builder_type_params.is_empty() {
        quote! {}
    } else {
        quote! { <#(#builder_type_params),*> }
    };

    let mut initial_states = Vec::new();
    for _ in &prop_generic_idents {
        initial_states.push(quote! { ::silex::dom::view::PropMissing });
    }
    let builder_ty_initial = get_builder_ty(&builder_name, &initial_states, &generics);

    let mut current_states = Vec::new();
    for ident in &prop_generic_idents {
        current_states.push(quote! { #ident });
    }
    let builder_ty_current = get_builder_ty(&builder_name, &current_states, &generics);

    let mut marker_types = Vec::new();
    // 1. Lifetimes (wrapped in &() to be types)
    for param in &generics.params {
        if let syn::GenericParam::Lifetime(l) = param {
            let lifetime = &l.lifetime;
            marker_types.push(quote! { &#lifetime () });
        }
    }
    // 2. Prop generic types
    for ident in &prop_generic_idents {
        marker_types.push(quote! { #ident });
    }
    // 3. Original type and const parameters
    for param in &generics.params {
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

    let builder_clone = quote! {
        #[derive(Clone)]
        #[allow(non_camel_case_types)]
        #vis struct #builder_name #builder_generics_decl #where_clause {
            #(#builder_fields,)*
            _pending_attrs: ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>,
            _markers: ::core::marker::PhantomData<(#(#marker_types),*)>,
        }
    };

    let builder_impl = quote! {
        impl #builder_generics_decl #builder_name #builder_generics_type #where_clause {
            pub fn new(#(#builder_new_params),*) -> #builder_ty_initial {
                #builder_name {
                    #(#builder_field_inits,)*
                    _pending_attrs: ::std::vec::Vec::new(),
                    _markers: ::core::marker::PhantomData,
                }
            }

            pub fn into_parts(self) -> (#props_name #ty_generics, ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>) {
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
    };

    let mut fixed_states = Vec::new();
    for _ in &prop_generic_idents {
        fixed_states.push(quote! { ::silex::dom::view::PropFixed });
    }
    let builder_ty_fixed = get_builder_ty(&builder_name, &fixed_states, &generics);

    let mut view_where_clause: syn::WhereClause = match where_clause {
        Some(clause) => clause.clone(),
        None => syn::parse_quote!(where),
    };
    view_where_clause.predicates.push(syn::parse_quote!(
        #builder_ty_fixed: ::core::clone::Clone
    ));

    let view_impl = quote! {
        impl #impl_generics ::silex::dom::view::View for #builder_ty_fixed #view_where_clause {
            fn mount(&self, parent: &::silex::reexports::web_sys::Node, attrs: ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>) {
                self.clone().mount_owned(parent, attrs);
            }

            fn mount_owned(self, parent: &::silex::reexports::web_sys::Node, attrs: ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>)
            where
                Self: Sized,
            {
                let (props, mut pending_attrs) = self.into_parts();
                pending_attrs.extend(attrs);
                let view_instance = #render_fn_name(props);
                ::silex::dom::view::View::mount_owned(view_instance, parent, pending_attrs);
            }
        }
    };

    let attribute_impl = quote! {
        impl #builder_generics_decl ::silex::dom::attribute::AttributeBuilder for #builder_ty_current #where_clause {
            fn build_attribute<__SilexValue>(mut self, target: ::silex::dom::attribute::ApplyTarget, value: __SilexValue) -> Self
            where
                __SilexValue: ::silex::dom::attribute::IntoStorable,
            {
                let owned_target = ::silex::dom::attribute::OwnedApplyTarget::from(target);
                self._pending_attrs.push(
                    ::silex::dom::attribute::PendingAttribute::build(
                        value.into_storable(),
                        owned_target,
                    )
                );
                self
            }

            fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
            where
                E: ::silex::dom::event::EventDescriptor + 'static,
                F: ::silex::dom::event::EventHandler<E::EventType, M> + Clone + 'static,
            {
                let event = event.clone();
                self._pending_attrs.push(
                    ::silex::dom::attribute::PendingAttribute::new_listener(move |el| {
                        ::silex::dom::element::bind_event(el, event, callback.clone());
                    })
                );
                self
            }
        }

        impl #builder_generics_decl ::silex::dom::view::ApplyAttributes for #builder_ty_current #where_clause {}
    };

    let constructor_fn = quote! {
        #[allow(non_snake_case, unused_variables, unused_mut)]
        #vis fn #component_name #impl_generics(#(#constructor_params),*) -> #builder_ty_initial #where_clause {
            <#builder_ty_initial>::new(#(#constructor_args),*)
        }
    };

    let type_alias = quote! {
        #[allow(non_camel_case_types)]
        #[allow(type_alias_bounds)]
        #vis type #component_component_alias #impl_generics = #builder_ty_fixed;
    };

    Ok(quote! {
        #builder_clone
        #builder_impl
        #attribute_impl
        #view_impl
        #type_alias
        #constructor_fn
    })
}

fn generate_setter(
    field: &FieldSpec,
    builder_name: &Ident,
    prop_generic_idents: &[Ident],
    required_fields: &[&FieldSpec],
    original_generics: &syn::Generics,
    all_fields: &[FieldSpec],
) -> TokenStream2 {
    let ident = &field.ident;
    let ty = &field.ty;

    let fields_destructure: Vec<_> = all_fields
        .iter()
        .map(|f| {
            let fid = &f.ident;
            quote! { #fid }
        })
        .collect();

    let setter_param = if is_any_view_type(ty) {
        quote! { impl ::silex::dom::view::View + 'static }
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

    let final_value = if !field.attrs.chained || !field.required {
        setter_value.clone()
    } else {
        quote! { ::core::option::Option::Some(#setter_value) }
    };

    if field.required {
        let req_index = required_fields
            .iter()
            .position(|f| f.ident == field.ident)
            .unwrap();

        let mut return_states = Vec::new();
        for (i, p) in prop_generic_idents.iter().enumerate() {
            if i == req_index {
                return_states.push(quote! { ::silex::dom::view::PropFixed });
            } else {
                return_states.push(quote! { #p });
            }
        }
        let return_ty = get_builder_ty(builder_name, &return_states, original_generics);

        quote! {
            #[allow(non_camel_case_types, unused_variables)]
            pub fn #ident(self, val: #setter_param) -> #return_ty {
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
        quote! {
            pub fn #ident(mut self, val: #setter_param) -> Self {
                self.#ident = #final_value;
                self
            }
        }
    }
}

fn field_value_transform(field: &FieldSpec, input: TokenStream2) -> TokenStream2 {
    let ty = &field.ty;
    if field.attrs.render && is_any_view_type(ty) {
        quote! { ::silex::dom::view::View::into_any(#input) }
    } else if field.attrs.into_trait || (is_auto_into_type(ty) && !is_any_view_type(ty)) {
        quote! { ::core::convert::Into::into(#input) }
    } else {
        input
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
