use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Field, Ident, Result, Token, Visibility, parse2};

pub struct ThemeDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub fields: Vec<Field>,
}

impl Parse for ThemeDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
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
                    let value = meta.value()?;
                    let s: syn::LitStr = value.parse()?;
                    prefix = s.value();
                }
                Ok(())
            });
        }
    }

    let mut struct_fields = Vec::new();
    let mut trait_decl_items: Vec<TokenStream> = Vec::new();
    let mut trait_impl_items: Vec<TokenStream> = Vec::new();
    let mut to_css_items: Vec<TokenStream> = Vec::new();
    let mut field_idents = Vec::new();
    let mut css_vars = Vec::new();

    for field in &def.fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;

        let mut custom_var = None;
        let mut filtered_field_attrs = Vec::new();
        for attr in &field.attrs {
            if attr.path().is_ident("theme") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("var") {
                        let value = meta.value()?;
                        let s: syn::LitStr = value.parse()?;
                        custom_var = Some(s.value());
                    }
                    Ok(())
                });
            } else {
                filtered_field_attrs.push(attr);
            }
        }

        let css_var = custom_var.unwrap_or_else(|| {
            format!("--{}-{}", prefix, field_name.to_string().replace("_", "-"))
        });
        css_vars.push(css_var.clone());
        field_idents.push(field_name.clone());

        struct_fields.push(quote! {
            #(#filtered_field_attrs)*
            pub #field_name: #field_ty
        });

        // Declaration in trait
        trait_decl_items.push(quote! {
            type #field_name;
        });

        // Definition in impl
        trait_impl_items.push(quote! {
            type #field_name = #field_ty;
        });

        to_css_items.push(quote! {
            format!("{}: {};", #css_var, self.#field_name)
        });
    }

    let trait_name = quote::format_ident!("{}Fields", name);

    let filtered_attrs: Vec<_> = def
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("theme"))
        .collect();

    let expanded = quote! {
        #[derive(Clone, Debug, Default)]
        #(#filtered_attrs)*
        #vis struct #name {
            #(#struct_fields),*
        }

        #[allow(non_camel_case_types)]
        pub trait #trait_name {
            #(#trait_decl_items)*
        }

        #[allow(non_camel_case_types)]
        impl #trait_name for #name {
            #(#trait_impl_items)*
        }

        impl ::silex::css::theme::ThemeType for #name {}

        impl ::silex::css::theme::ThemeToCss for #name {
            fn to_css_variables(&self) -> String {
                let mut s = String::new();
                #( s.push_str(&#to_css_items); )*
                s
            }

            fn get_variable_values(&self) -> Vec<String> {
                vec![
                    #( self.#field_idents.to_string() ),*
                ]
            }

            fn get_variable_names() -> &'static [&'static str] {
                &[
                    #( #css_vars ),*
                ]
            }
        }

        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", ::silex::css::theme::ThemeToCss::to_css_variables(self))
            }
        }
    };

    Ok(expanded)
}
