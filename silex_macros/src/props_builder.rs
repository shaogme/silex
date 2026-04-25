use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type, Visibility};

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

    fn generate_builder_struct(&self) -> TokenStream2 {
        let vis = &self.vis;
        let builder_name = &self.builder_name;
        let (_, _, where_clause) = self.generics.split_for_impl();
        let (builder_generics_decl, _) = self.get_builder_generics();

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
                _pending_attrs: ::std::vec::Vec<::silex::dom::attribute::PendingAttribute>,
                _markers: ::core::marker::PhantomData<(#(#marker_types),*)>,
            }
        }
    }

    fn generate_builder_impl(&self) -> TokenStream2 {
        let (builder_generics_decl, builder_generics_type) = self.get_builder_generics();
        let builder_name = &self.builder_name;
        let props_name = &self.props_name;
        let (_, ty_generics, where_clause) = self.generics.split_for_impl();

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { ::silex::dom::view::PropMissing })
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

        let builder_setters = self.fields.iter().map(|f| self.generate_setter(f));

        quote! {
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
        }
    }

    fn generate_setter(&self, field: &FieldSpec) -> TokenStream2 {
        let builder_name = &self.builder_name;
        let ident = &field.ident;
        let ty = &field.ty;

        let fields_destructure: Vec<_> = self.fields.iter().map(|f| &f.ident).collect();

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
            let req_index = self
                .required_fields
                .iter()
                .position(|f| f.ident == field.ident)
                .unwrap();

            let mut return_states = Vec::new();
            for (i, p) in self.prop_generic_idents.iter().enumerate() {
                if i == req_index {
                    return_states.push(quote! { ::silex::dom::view::PropFixed });
                } else {
                    return_states.push(quote! { #p });
                }
            }
            let return_ty = self.get_builder_ty(&return_states);

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

    fn generate_view_impl(&self) -> TokenStream2 {
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let render_fn_name = &self.render_fn_name;

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { ::silex::dom::view::PropFixed })
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
        }
    }

    fn generate_attribute_impl(&self) -> TokenStream2 {
        let (builder_generics_decl, _) = self.get_builder_generics();
        let (_, _, where_clause) = self.generics.split_for_impl();

        let current_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|ident| quote! { #ident })
            .collect();
        let builder_ty_current = self.get_builder_ty(&current_states);

        quote! {
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
        }
    }

    fn generate_constructor(&self) -> TokenStream2 {
        let vis = &self.vis;
        let component_name = &self.component_name;
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        let component_component_alias = &self.component_component_alias;

        let initial_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { ::silex::dom::view::PropMissing })
            .collect();
        let builder_ty_initial = self.get_builder_ty(&initial_states);

        let fixed_states: Vec<_> = self
            .prop_generic_idents
            .iter()
            .map(|_| quote! { ::silex::dom::view::PropFixed })
            .collect();
        let builder_ty_fixed = self.get_builder_ty(&fixed_states);

        let standalone_fields: Vec<_> = self.fields.iter().filter(|f| !f.attrs.chained).collect();

        let constructor_params = standalone_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if is_any_view_type(ty) {
                quote! { #ident: impl ::silex::dom::view::View + 'static }
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
            #[allow(non_camel_case_types)]
            #[allow(type_alias_bounds)]
            #vis type #component_component_alias #impl_generics = #builder_ty_fixed;

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
