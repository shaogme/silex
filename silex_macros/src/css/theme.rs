use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Field, Ident, Result, Token, Visibility, parse2};

pub struct ThemeDefinition {
    pub vis: Visibility,
    pub name: Ident,
    pub fields: Vec<Field>,
}

impl Parse for ThemeDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        let fields = content.parse_terminated(Field::parse_named, Token![,])?;

        Ok(ThemeDefinition {
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

    let mut struct_fields = Vec::new();
    let mut trait_decl_items: Vec<TokenStream> = Vec::new();
    let mut trait_impl_items: Vec<TokenStream> = Vec::new();
    let mut to_css_items: Vec<TokenStream> = Vec::new();

    for field in &def.fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;

        let css_var = format!("--slx-theme-{}", field_name);

        struct_fields.push(quote! {
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

    let expanded = quote! {
        #[derive(Clone, Debug, Default)]
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
        }

        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", ::silex::css::theme::ThemeToCss::to_css_variables(self))
            }
        }
    };

    Ok(expanded)
}
