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
    let __silex = crate::crate_path::silex();
    let def: ThemeDefinition = parse2(input)?;
    let name = &def.name;
    let vis = &def.vis;

    let config = crate::css::config::get_config();

    // 前缀的来源，从低到高：内置默认 → `silex.toml` 的 `[theme].prefix`
    // → `#[theme(prefix = "…")]`。配置那一层此前只在「字段为空、从配置补全」
    // 的分支里读，显式声明了字段的主题拿不到 `[theme].prefix`。
    let mut prefix = "slx-theme".to_string();
    if let Some(cfg) = &config
        && let Some(p) = &cfg.theme.prefix
    {
        prefix = p.clone();
    }
    let mut is_main = false;
    for attr in &def.attrs {
        if attr.path().is_ident("theme") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("prefix") {
                    prefix = meta.value()?.parse::<syn::LitStr>()?.value();
                } else if meta.path.is_ident("main") {
                    is_main = true;
                }
                Ok(())
            });
        }
    }

    let is_main_tokens = if is_main {
        quote! { #vis type Theme = #name; }
    } else {
        quote! {}
    };

    let mut struct_fields = Vec::new();
    let mut trait_decl_items = Vec::new();
    let mut trait_impl_items = Vec::new();
    let mut to_css_items = Vec::new();
    let mut field_idents = Vec::new();
    let mut css_vars = Vec::new();
    let mut const_impl_items = Vec::new();

    // 没有显式声明字段时，从 `silex.toml` 的配色表补出来
    let mut fields = def.fields.clone();
    if fields.is_empty()
        && let Some(cfg) = &config
    {
        let mut keys: Vec<&String> = cfg.theme.colors.keys().collect();
        keys.sort();
        for k in keys {
            let rust_name = k.replace('-', "_");
            let field_ident = syn::Ident::new(&rust_name, proc_macro2::Span::call_site());
            let field_ty: syn::Type = syn::parse_quote!(String);
            let field_ast: Field = syn::Field {
                attrs: Vec::new(),
                vis: syn::Visibility::Inherited,
                mutability: syn::FieldMutability::None,
                ident: Some(field_ident),
                colon_token: Some(Default::default()),
                ty: field_ty,
            };
            fields.push(field_ast);
        }
    }

    let dep_tokens = crate::css::config::generate_config_dependency_tokens();

    for field in &fields {
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
            pub const #const_name: #__silex::css::types::CssVar<#f_ty> =
                #__silex::css::types::CssVar(
                    #__silex::css::types::CssVarValue::Static(#var_expr),
                    ::std::marker::PhantomData
                );
        });
    }

    let trait_name = quote::format_ident!("{}Fields", name);
    let patch_name = quote::format_ident!("{}Patch", name);
    let mut patch_fields = Vec::new();
    let mut patch_entries = Vec::new();
    let mut patch_setters = Vec::new();

    // 用补全后的 `fields`，不是 `def.fields`：配置驱动的主题
    // （`theme!{ struct T {} }` + `silex.toml` 配色）在 `def.fields` 里是空的，
    // 于是 `TPatch` 一个字段都没有、`get_patch_entries()` 返回空 vec，
    // `theme_patch()` 静默无效
    for (field_idx, field) in fields.iter().enumerate() {
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

        impl #__silex::css::theme::ThemeType for #name {}

        impl #__silex::css::theme::ThemeToCss for #name {
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
                write!(f, "{}", #__silex::css::theme::ThemeToCss::to_css_variables(self))
            }
        }

        #[derive(Clone, Debug, Default)]
        #vis struct #patch_name { #(#patch_fields),* }

        impl #patch_name {
            #(#patch_setters)*
        }

        #is_main_tokens

        impl #__silex::css::theme::ThemePatchToCss for #patch_name {
            fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)> {
                vec![ #(#patch_entries),* ]
            }
        }

        #dep_tokens
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 工作区根目录的 `silex.toml` 里有 `brand-primary` / `brand-accent`，
    /// `get_config` 会一路向上找到它，所以这里能覆盖「配置驱动」这条路。
    fn config_colors_available() -> bool {
        crate::css::config::get_config()
            .map(|c| c.theme.colors.contains_key("brand-primary"))
            .unwrap_or(false)
    }

    /// 配置驱动的主题此前 Patch 结构体是空的：补全字段用的是 `fields`，
    /// 而 Patch 那一段读的是 `def.fields`（空）。于是 `get_patch_entries()`
    /// 返回空 vec，`theme_patch()` 静默无效。
    #[test]
    fn config_driven_theme_gets_patch_fields() {
        if !config_colors_available() {
            return;
        }
        let out = bridge_theme_impl(quote! { pub struct T {} })
            .unwrap()
            .to_string();
        assert!(out.contains("struct TPatch"), "{out}");
        assert!(out.contains("brand_primary"), "{out}");
        assert!(out.contains("brand_accent"), "{out}");
    }

    /// 显式声明字段的主题也要读到 `[theme].prefix`
    #[test]
    fn explicit_fields_still_read_the_configured_prefix() {
        if !config_colors_available() {
            return;
        }
        let out = bridge_theme_impl(quote! { pub struct T { primary: Hex } })
            .unwrap()
            .to_string();
        assert!(out.contains("--slx-theme-primary"), "{out}");
    }

    /// `#[theme(prefix = …)]` 优先于配置
    #[test]
    fn attribute_prefix_wins_over_config() {
        let out = bridge_theme_impl(quote! {
            #[theme(prefix = "custom")]
            pub struct T { primary: Hex }
        })
        .unwrap()
        .to_string();
        assert!(out.contains("--custom-primary"), "{out}");
    }
}
