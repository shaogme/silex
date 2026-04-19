use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Field, Ident, Result, Token, Visibility, parse2};

pub struct ThemeDefinition {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub fields: Vec<Field>,
}

impl Parse for ThemeDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let fields = content.parse_terminated(Field::parse_named, Token![,])?;

        Ok(ThemeDefinition {
            attrs,
            vis,
            name,
            fields: fields.into_iter().collect(),
        })
    }
}

pub fn bridge_theme_impl(input: TokenStream) -> Result<TokenStream> {
    let def: ThemeDefinition = parse2(input)?;
    let name = &def.name;
    let vis = &def.vis;

    let mut prefix = "slx-theme".to_string();
    for attr in &def.attrs {
        if attr.path().is_ident("theme") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("prefix") {
                    prefix = meta.value()?.parse::<syn::LitStr>()?.value();
                }
                Ok(())
            });
        }
    }

    let mut struct_fields = Vec::new();
    let mut trait_decl_items = Vec::new();
    let mut trait_impl_items = Vec::new();
    let mut to_css_items = Vec::new();
    let mut field_idents = Vec::new();
    let mut css_vars = Vec::new();
    let mut const_impl_items = Vec::new();

    for field in &def.fields {
        let f_name = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "Theme fields must be named"))?;
        let f_ty = &field.ty;

        let mut custom_var = None;
        let mut filtered_attrs = Vec::new();
        for attr in &field.attrs {
            if attr.path().is_ident("theme") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("var") {
                        custom_var = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                    }
                    Ok(())
                });
            } else {
                filtered_attrs.push(attr);
            }
        }

        let css_var_name = custom_var
            .unwrap_or_else(|| format!("--{}-{}", prefix, f_name.to_string().replace('_', "-")));
        css_vars.push(css_var_name.clone());
        field_idents.push(f_name.clone());

        struct_fields.push(quote! { #(#filtered_attrs)* pub #f_name: #f_ty });
        trait_decl_items.push(quote! { type #f_name; });
        trait_impl_items.push(quote! { type #f_name = #f_ty; });
        to_css_items.push(quote! { format!("{}: {};", #css_var_name, self.#f_name) });

        let const_name = quote::format_ident!("{}", f_name.to_string().to_uppercase());
        let var_expr = format!("var({})", css_var_name);

        const_impl_items.push(quote! {
            pub const #const_name: ::silex::css::types::CssVar<#f_ty> =
                ::silex::css::types::CssVar(
                    ::silex::css::types::CssVarValue::Static(#var_expr),
                    ::std::marker::PhantomData
                );
        });
    }

    let trait_name = quote::format_ident!("{}Fields", name);
    let patch_name = quote::format_ident!("{}Patch", name);
    let mut patch_fields = Vec::new();
    let mut patch_entries = Vec::new();
    let mut patch_setters = Vec::new();

    for (field_idx, field) in def.fields.iter().enumerate() {
        let f_name = field_idents[field_idx].clone();
        let f_ty = &field.ty;
        let css_var_name = &css_vars[field_idx];

        patch_fields.push(quote! { pub #f_name: Option<#f_ty> });
        patch_entries.push(quote! {
            (#css_var_name, self.#f_name.as_ref().map(|v| v.to_string()))
        });
        patch_setters.push(quote! {
            pub fn #f_name(mut self, val: impl Into<#f_ty>) -> Self {
                self.#f_name = Some(val.into());
                self
            }
        });
    }
    let filtered_attrs: Vec<_> = def
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("theme"))
        .collect();

    Ok(quote! {
        #[derive(Clone, Debug, Default)]
        #(#filtered_attrs)*
        #vis struct #name { #(#struct_fields),* }

        impl #name {
            #(#const_impl_items)*
        }

        #[allow(non_camel_case_types)]
        pub trait #trait_name { #(#trait_decl_items)* }

        #[allow(non_camel_case_types)]
        impl #trait_name for #name { #(#trait_impl_items)* }

        impl ::silex::css::theme::ThemeType for #name {}

        impl ::silex::css::theme::ThemeToCss for #name {
            fn to_css_variables(&self) -> String {
                let mut s = String::new();
                #( s.push_str(&#to_css_items); )*
                s
            }
            fn get_variable_values(&self) -> Vec<String> { vec![ #( self.#field_idents.to_string() ),* ] }
            fn get_variable_names() -> &'static [&'static str] { &[ #( #css_vars ),* ] }
        }

        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", ::silex::css::theme::ThemeToCss::to_css_variables(self))
            }
        }

        #[derive(Clone, Debug, Default)]
        #vis struct #patch_name { #(#patch_fields),* }

        impl #patch_name {
            #(#patch_setters)*
        }

        impl ::silex::css::theme::ThemePatchToCss for #patch_name {
            fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)> {
                vec![ #(#patch_entries),* ]
            }
        }
    })
}
