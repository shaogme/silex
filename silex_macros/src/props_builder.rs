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

    let builder_setters = fields.iter().map(generate_setter);

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

    let builder_clone = quote! {
        #[derive(Clone)]
        #vis struct #builder_name #impl_generics #where_clause {
            #(#builder_fields,)*
            _pending_attrs: ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>,
        }
    };

    let builder_impl = quote! {
        impl #impl_generics #builder_name #ty_generics #where_clause {
            pub fn new(#(#builder_new_params),*) -> Self {
                Self {
                    #(#builder_field_inits,)*
                    _pending_attrs: ::std::vec::Vec::new(),
                }
            }

            pub fn into_parts(self) -> (#props_name #ty_generics, ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>) {
                let Self {
                    #(#fields_destructure,)*
                    _pending_attrs,
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

    let mut view_where_clause: syn::WhereClause = match where_clause {
        Some(clause) => clause.clone(),
        None => syn::parse_quote!(where),
    };
    view_where_clause.predicates.push(syn::parse_quote!(
        #builder_name #ty_generics: ::core::clone::Clone
    ));

    let view_impl = quote! {
        impl #impl_generics ::silex::dom::view::View for #builder_name #ty_generics #view_where_clause {
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
        impl #impl_generics ::silex::dom::attribute::AttributeBuilder for #builder_name #ty_generics #where_clause {
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

        impl #impl_generics ::silex::dom::view::ApplyAttributes for #builder_name #ty_generics #where_clause {}
    };

    let constructor_fn = quote! {
        #[allow(non_snake_case)]
        #vis fn #component_name #impl_generics(#(#constructor_params),*) -> #builder_name #ty_generics #where_clause {
            #builder_name::new(#(#constructor_args),*)
        }
    };

    let type_alias = quote! {
        #[allow(non_camel_case_types)]
        #[allow(type_alias_bounds)]
        #vis type #component_component_alias #impl_generics = #builder_name #ty_generics;
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

fn generate_setter(field: &FieldSpec) -> TokenStream2 {
    let ident = &field.ident;
    let ty = &field.ty;
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
        setter_value
    } else {
        quote! { ::core::option::Option::Some(#setter_value) }
    };

    quote! {
        pub fn #ident(mut self, val: #setter_param) -> Self {
            self.#ident = #final_value;
            self
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
