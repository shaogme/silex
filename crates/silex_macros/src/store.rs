use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Field, Fields, GenericParam, Generics, Ident, ItemStruct, Result, Type, Visibility, parse_quote,
};

struct StoreFieldInfo {
    ident: Ident,
    ty: Type,
    visibility: Visibility,
    handle_ident: Ident,
    input_ident: Ident,
}

pub fn store_impl(mut model: ItemStruct) -> Result<TokenStream> {
    let core = crate::crate_path::silex_core();
    let model_name = model.ident.clone();
    let model_visibility = model.vis.clone();
    let model_generics = model.generics.clone();

    reject_reserved_lifetime(&model_generics)?;

    let fields = match &model.fields {
        Fields::Named(fields) => fields.named.iter().collect::<Vec<_>>(),
        _ => {
            return Err(syn::Error::new_spanned(
                &model,
                "#[store] only supports structs with named fields",
            ));
        }
    };

    reject_persistence_attributes(&model, &fields)?;
    model
        .attrs
        .retain(|attribute| !attribute.path().is_ident("store"));

    let field_infos = fields
        .iter()
        .enumerate()
        .map(|(index, field)| StoreFieldInfo {
            ident: field.ident.clone().expect("named Store field"),
            ty: field.ty.clone(),
            visibility: field.vis.clone(),
            handle_ident: format_ident!("__SilexStoreField{index}"),
            input_ident: format_ident!("__SilexStoreInput{index}"),
        })
        .collect::<Vec<_>>();

    let store_name = format_ident!("{model_name}Store");
    let fields_name = format_ident!("{model_name}StoreFields");
    let model_type = model_type(&model_name, &model_generics);

    let mut alias_generics = alias_generics(&model_generics, &field_infos, &core);
    add_owner_bounds_to_parameters(&mut alias_generics);
    let impl_generics = impl_generics(&model_generics);
    let mut fields_generics = fields_generics(&model_generics, &field_infos);
    add_owner_bounds_to_where(&mut fields_generics, &model_generics);
    add_store_field_bounds(&mut fields_generics, &field_infos, &core);

    let (fields_impl_generics, fields_ty_generics, fields_where) = fields_generics.split_for_impl();

    let default_fields_args = type_arguments(&model_generics, &field_infos, |field| {
        let ty = &field.ty;
        quote!(#core::RwSignal<'owner, #ty>)
    });
    let alias_fields_args = type_arguments(&model_generics, &field_infos, |field| {
        let handle = &field.handle_ident;
        quote!(#handle)
    });
    let model_fields = field_infos.iter().map(|field| {
        let ident = &field.ident;
        let handle = &field.handle_ident;
        let visibility = &field.visibility;
        quote!(#visibility #ident: #handle)
    });

    let new_fields = field_infos.iter().map(|field| {
        let ident = &field.ident;
        quote!(#ident: owner.rw_signal(source.#ident)?)
    });

    let handle_arguments = field_infos
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let handle = &field.handle_ident;
            quote!(#ident: #handle)
        })
        .collect::<Vec<_>>();
    let handle_names = field_infos
        .iter()
        .map(|field| {
            let ident = &field.ident;
            quote!(#ident)
        })
        .collect::<Vec<_>>();
    let input_handle_arguments = field_infos
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let input = &field.input_ident;
            quote!(#ident: #input)
        })
        .collect::<Vec<_>>();
    let input_generics = field_infos
        .iter()
        .map(|field| {
            let input = &field.input_ident;
            quote!(#input)
        })
        .collect::<Vec<_>>();
    let input_constraints = field_infos
        .iter()
        .map(|field| {
            let input = &field.input_ident;
            let ty = &field.ty;
            quote!(
                #input: #core::StoreField<'owner, #ty>
                    + Into<#core::RwSignal<'owner, #ty>>
            )
        })
        .collect::<Vec<_>>();
    let input_conversion_fields = field_infos
        .iter()
        .map(|field| {
            let ident = &field.ident;
            quote!(#ident: #ident.into())
        })
        .collect::<Vec<_>>();
    let input_method_generics = if input_generics.is_empty() {
        quote!()
    } else {
        quote!(<#(#input_generics),*>)
    };
    let input_method_where = if input_constraints.is_empty() {
        quote!()
    } else {
        quote!(where #(#input_constraints),*)
    };
    let snapshot_fields = field_infos.iter().map(|field| {
        let ident = &field.ident;
        quote!(#ident: #core::RxGet::get(&self.#ident))
    });
    let snapshot_untracked_fields = field_infos.iter().map(|field| {
        let ident = &field.ident;
        quote!(#ident: #core::RxGet::get_untracked(&self.#ident))
    });
    let model_expression = model_expression(&model_name, &model_generics);

    let mut default_impl_generics = impl_generics.clone();
    add_owner_bounds_to_where(&mut default_impl_generics, &model_generics);
    let (default_impl_generics_tokens, _, default_impl_where) =
        default_impl_generics.split_for_impl();

    Ok(quote! {
        #model

        #[doc(hidden)]
        #[derive(Clone, Copy)]
        #model_visibility struct #fields_name #fields_generics #fields_where {
            owner: #core::OwnerAccess<'owner>,
            _marker: ::std::marker::PhantomData<fn() -> #model_type>,
            #(#model_fields),*
        }

        #model_visibility type #store_name #alias_generics = #fields_name #alias_fields_args;

        impl #default_impl_generics_tokens #fields_name #default_fields_args #default_impl_where
        {
            pub fn new(
                owner: #core::OwnerAccess<'owner>,
                source: #model_type,
            ) -> #core::SilexResult<Self> {
                Ok(Self {
                    owner,
                    _marker: ::std::marker::PhantomData,
                    #(#new_fields),*
                })
            }

            /// Builds the default Store type from compatible scoped handles.
            ///
            pub fn from_handles #input_method_generics (
                owner: #core::OwnerAccess<'owner>,
                #(#input_handle_arguments),*
            ) -> #core::SilexResult<Self>
            #input_method_where
            {
                Ok(Self {
                    owner,
                    _marker: ::std::marker::PhantomData,
                    #(#input_conversion_fields),*
                })
            }

        }

        impl #fields_impl_generics #fields_name #fields_ty_generics #fields_where {
            pub fn owner(&self) -> #core::OwnerAccess<'owner> {
                self.owner
            }

            pub fn from_typed_handles(
                owner: #core::OwnerAccess<'owner>,
                #(#handle_arguments),*
            ) -> #core::SilexResult<Self> {
                Ok(Self {
                    owner,
                    _marker: ::std::marker::PhantomData,
                    #(#handle_names),*
                })
            }

            pub fn snapshot(&self) -> #core::SilexResult<#model_type> {
                Ok(#model_expression {
                    #(#snapshot_fields?),*
                })
            }

            pub fn snapshot_untracked(&self) -> #core::SilexResult<#model_type> {
                Ok(#model_expression {
                    #(#snapshot_untracked_fields?),*
                })
            }
        }
    })
}

fn reject_persistence_attributes(model: &ItemStruct, fields: &[&Field]) -> Result<()> {
    for attribute in &model.attrs {
        if attribute.path().is_ident("persist") {
            return Err(syn::Error::new_spanned(
                attribute,
                "#[persist(...)] is not supported by #[store]; build Persistent explicitly and use from_handles",
            ));
        }
    }

    for field in fields {
        for attribute in &field.attrs {
            if attribute.path().is_ident("persist") {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "#[persist(...)] is not supported by #[store]; build Persistent explicitly and use from_handles",
                ));
            }
        }
    }

    Ok(())
}

fn reject_reserved_lifetime(generics: &Generics) -> Result<()> {
    for parameter in &generics.params {
        if let GenericParam::Lifetime(parameter) = parameter
            && parameter.lifetime.ident == "owner"
        {
            return Err(syn::Error::new_spanned(
                parameter,
                "the model lifetime 'owner is reserved by #[store]",
            ));
        }
    }

    Ok(())
}

fn model_type(model_name: &Ident, generics: &Generics) -> TokenStream {
    let (_, ty_generics, _) = generics.split_for_impl();
    quote!(#model_name #ty_generics)
}

fn model_expression(model_name: &Ident, generics: &Generics) -> TokenStream {
    let (_, ty_generics, _) = generics.split_for_impl();

    if generics.params.is_empty() {
        quote!(#model_name)
    } else {
        quote!(#model_name :: #ty_generics)
    }
}

fn alias_generics(
    model_generics: &Generics,
    fields: &[StoreFieldInfo],
    core: &TokenStream,
) -> Generics {
    let mut params = syn::punctuated::Punctuated::new();
    params.push(parse_quote!('owner));
    params.extend(model_generics.params.iter().cloned());

    for field in fields {
        let ty = &field.ty;
        params.push(GenericParam::Type(syn::TypeParam {
            attrs: Vec::new(),
            ident: field.handle_ident.clone(),
            colon_token: None,
            bounds: syn::punctuated::Punctuated::new(),
            eq_token: Some(Default::default()),
            default: Some(parse_quote!(#core::RwSignal<'owner, #ty>)),
        }));
    }

    Generics {
        lt_token: Some(Default::default()),
        params,
        gt_token: Some(Default::default()),
        where_clause: None,
    }
}

fn impl_generics(model_generics: &Generics) -> Generics {
    let mut generics = model_generics.clone();
    generics.params.insert(0, parse_quote!('owner));
    clear_generic_defaults(&mut generics);
    generics
}

fn fields_generics(model_generics: &Generics, fields: &[StoreFieldInfo]) -> Generics {
    let mut params = syn::punctuated::Punctuated::new();
    params.push(parse_quote!('owner));

    for parameter in &model_generics.params {
        match parameter {
            GenericParam::Lifetime(parameter) => {
                params.push(GenericParam::Lifetime(parameter.clone()));
            }
            GenericParam::Type(parameter) => {
                let mut parameter = parameter.clone();
                parameter.eq_token = None;
                parameter.default = None;
                params.push(GenericParam::Type(parameter));
            }
            GenericParam::Const(parameter) => {
                let mut parameter = parameter.clone();
                parameter.eq_token = None;
                parameter.default = None;
                params.push(GenericParam::Const(parameter));
            }
        }
    }

    for field in fields {
        params.push(GenericParam::Type(syn::TypeParam {
            attrs: Vec::new(),
            ident: field.handle_ident.clone(),
            colon_token: None,
            bounds: syn::punctuated::Punctuated::new(),
            eq_token: None,
            default: None,
        }));
    }

    Generics {
        lt_token: Some(Default::default()),
        params,
        gt_token: Some(Default::default()),
        where_clause: model_generics.where_clause.clone(),
    }
}

fn clear_generic_defaults(generics: &mut Generics) {
    for parameter in &mut generics.params {
        match parameter {
            GenericParam::Type(parameter) => {
                parameter.eq_token = None;
                parameter.default = None;
            }
            GenericParam::Const(parameter) => {
                parameter.eq_token = None;
                parameter.default = None;
            }
            GenericParam::Lifetime(_) => {}
        }
    }
}

fn add_owner_bounds_to_parameters(generics: &mut Generics) {
    for parameter in &mut generics.params {
        match parameter {
            GenericParam::Lifetime(parameter) => {
                if parameter.lifetime.ident != "owner" {
                    parameter.bounds.push(parse_quote!('owner));
                }
            }
            GenericParam::Type(parameter) => {
                parameter.bounds.push(parse_quote!('owner));
            }
            GenericParam::Const(_) => {}
        }
    }
}

fn add_owner_bounds_to_where(generics: &mut Generics, model_generics: &Generics) {
    let where_clause = generics.make_where_clause();

    for parameter in &model_generics.params {
        match parameter {
            GenericParam::Lifetime(parameter) => {
                let lifetime = &parameter.lifetime;
                where_clause
                    .predicates
                    .push(parse_quote!(#lifetime: 'owner));
            }
            GenericParam::Type(parameter) => {
                let ident = &parameter.ident;
                where_clause.predicates.push(parse_quote!(#ident: 'owner));
            }
            GenericParam::Const(_) => {}
        }
    }
}

fn add_store_field_bounds(generics: &mut Generics, fields: &[StoreFieldInfo], core: &TokenStream) {
    let where_clause = generics.make_where_clause();

    for field in fields {
        let ty = &field.ty;
        let handle = &field.handle_ident;
        where_clause.predicates.push(parse_quote!(
            #handle: #core::StoreField<'owner, #ty>
        ));
    }
}

fn type_arguments<F>(
    model_generics: &Generics,
    fields: &[StoreFieldInfo],
    field_argument: F,
) -> TokenStream
where
    F: Fn(&StoreFieldInfo) -> TokenStream,
{
    let mut arguments = vec![quote!('owner)];
    for parameter in &model_generics.params {
        match parameter {
            GenericParam::Lifetime(parameter) => {
                let lifetime = &parameter.lifetime;
                arguments.push(quote!(#lifetime));
            }
            GenericParam::Type(parameter) => {
                let ident = &parameter.ident;
                arguments.push(quote!(#ident));
            }
            GenericParam::Const(parameter) => {
                let ident = &parameter.ident;
                arguments.push(quote!(#ident));
            }
        }
    }

    arguments.extend(fields.iter().map(field_argument));

    quote!(<#(#arguments),*>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn generated_store_expansion_is_parseable() {
        let input: ItemStruct = parse_quote! {
            #[derive(Clone)]
            struct Generic<'model, T>
            where
                T: Clone,
            {
                value: T,
                label: &'model str,
            }
        };

        let tokens = store_impl(input).unwrap();
        syn::parse2::<syn::File>(tokens).unwrap();
    }

    #[test]
    fn persistence_attributes_are_rejected() {
        let input: ItemStruct = parse_quote! {
            struct Settings {
                #[persist(local, codec = "string")]
                theme: String,
            }
        };

        let error = store_impl(input).unwrap_err();
        assert!(error.to_string().contains("not supported by #[store]"));
    }
}
