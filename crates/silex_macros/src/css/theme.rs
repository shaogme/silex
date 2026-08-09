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

/// `#RGB` / `#RGBA` / `#RRGGBB` / `#RRGGBBAA`，`#` 可省略。
///
/// 与 `silex_css::types::Hex::try_new` 的判据保持一致；宏侧用不上运行时那份，
/// 只能照抄一遍判据。
fn looks_like_hex(value: &str) -> bool {
    let digits = value.trim().trim_start_matches('#');
    matches!(digits.len(), 3 | 4 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
}

/// 没有在 `[theme.field_types]` 里指定类型时，按取值猜一个。
fn infer_field_type(value: &str) -> &'static str {
    if looks_like_hex(value) {
        "Hex"
    } else {
        "String"
    }
}

/// 把配置里写的类型名解析成一个 Rust 类型。
///
/// 裸标识符按 CSS 值类型解析（`Hex` → `silex::css::types::Hex`），带 `::`
/// 的路径原样使用，`String` 特判成 `std::string::String`。
fn resolve_field_type(
    name: &str,
    silex: &TokenStream,
    key: &str,
    span_src: &Ident,
) -> Result<syn::Type> {
    let name = name.trim();
    if name == "String" {
        return Ok(syn::parse_quote!(::std::string::String));
    }
    if name.contains("::") || name.contains('<') {
        return syn::parse_str::<syn::Type>(name).map_err(|e| {
            syn::Error::new_spanned(
                span_src,
                format!("`[theme.field_types].{key}` 不是一个合法的 Rust 类型：{e}"),
            )
        });
    }
    let ident = syn::parse_str::<Ident>(name).map_err(|_| {
        syn::Error::new_spanned(
            span_src,
            format!("`[theme.field_types].{key}` 不是一个合法的类型名：`{name}`"),
        )
    })?;
    Ok(syn::parse_quote!(#silex::css::types::#ident))
}

/// 配置驱动的主题里，字段的初值。
///
/// 补出了字段却拿不到配好的颜色，等于把 `silex.toml` 里那张配色表白写了：
/// `Default` 会给出 `Hex::default()`（黑）而不是配置里的值。
fn default_expr_for(
    ty_name: &str,
    raw_value: &str,
    silex: &TokenStream,
    key: &str,
    span_src: &Ident,
) -> Result<TokenStream> {
    match ty_name.trim() {
        "Hex" => {
            if !looks_like_hex(raw_value) {
                return Err(syn::Error::new_spanned(
                    span_src,
                    format!(
                        "`[theme.colors].{key}` 的值 `{raw_value}` 不是合法的十六进制颜色，\
                         但字段类型被指定为 `Hex`；请改用 `#RRGGBB` 形式，\
                         或在 `[theme.field_types]` 里换一个类型"
                    ),
                ));
            }
            Ok(quote! { #silex::css::types::hex(#raw_value) })
        }
        "String" => Ok(quote! { #raw_value.to_string() }),
        // 量纲类型：`radius = "8px"` + `field_types.radius = "Px"` 必须真的给出
        // `8px`。走 `Default::default()` 的话是 `0px`——配置里写的值被静默丢掉，
        // 和当初 `def.fields` 那个空 Patch 是同一类问题。
        ty if unit_ctor(ty).is_some() => {
            let ctor = unit_ctor(ty).unwrap();
            let Some(num) = parse_unit_value(raw_value, ty) else {
                return Err(syn::Error::new_spanned(
                    span_src,
                    format!(
                        "`[theme.colors].{key}` 的值 `{raw_value}` 不是一个 `{ty}` \
                         取值（期望形如 `8{}`）；请改写取值，或在 \
                         `[theme.field_types]` 里换一个类型",
                        unit_suffix(ty).unwrap_or("")
                    ),
                ));
            };
            let ctor = syn::Ident::new(ctor, proc_macro2::Span::call_site());
            Ok(quote! { #silex::css::types::#ctor(#num) })
        }
        // 其余类型没有统一的「从字符串构造」入口，退回该类型自己的默认值
        _ => Ok(quote! { ::core::default::Default::default() }),
    }
}

/// 量纲类型名 → (工厂函数, CSS 后缀)。
///
/// 与 `silex_css::types::units` 里 `define_dimension!` 的清单对应。
fn unit_table(ty: &str) -> Option<(&'static str, &'static str)> {
    Some(match ty {
        "Px" => ("px", "px"),
        "Rem" => ("rem", "rem"),
        "Em" => ("em_unit", "em"),
        "Ch" => ("ch", "ch"),
        "Ex" => ("ex", "ex"),
        "Vw" => ("vw", "vw"),
        "Vh" => ("vh", "vh"),
        "Vmin" => ("vmin", "vmin"),
        "Vmax" => ("vmax", "vmax"),
        "Dvw" => ("dvw", "dvw"),
        "Dvh" => ("dvh", "dvh"),
        "Svw" => ("svw", "svw"),
        "Svh" => ("svh", "svh"),
        "Lvw" => ("lvw", "lvw"),
        "Lvh" => ("lvh", "lvh"),
        "Pt" => ("pt", "pt"),
        "Pc" => ("pc", "pc"),
        "Cm" => ("cm", "cm"),
        "Mm" => ("mm", "mm"),
        "In" => ("inch", "in"),
        "Qmm" => ("qmm", "Q"),
        "Percent" => ("pct", "%"),
        "Fr" => ("fr", "fr"),
        "Deg" => ("deg", "deg"),
        "Rad" => ("rad", "rad"),
        "Turn" => ("turn", "turn"),
        "Sec" => ("sec", "s"),
        "Ms" => ("ms", "ms"),
        _ => return None,
    })
}

fn unit_ctor(ty: &str) -> Option<&'static str> {
    unit_table(ty).map(|(ctor, _)| ctor)
}

fn unit_suffix(ty: &str) -> Option<&'static str> {
    unit_table(ty).map(|(_, suffix)| suffix)
}

/// 从 `"8px"` 里取出 `8.0`。后缀可以省略（`"8"` 也接受），但不能写错：
/// `"8rem"` 配 `Px` 会返回 `None`，进而变成一条编译错误，而不是静默取 `8px`。
fn parse_unit_value(raw: &str, ty: &str) -> Option<f64> {
    let suffix = unit_suffix(ty)?;
    let raw = raw.trim();
    let num = match raw.strip_suffix(suffix) {
        Some(n) => n,
        // 没有后缀时只接受纯数值
        None if raw
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')) =>
        {
            raw
        }
        None => return None,
    };
    let v: f64 = num.trim().parse().ok()?;
    v.is_finite().then_some(v)
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

    // 没有显式声明字段时，从 `silex.toml` 的配色表补出来。
    //
    // 字段类型此前被硬编码成 `String`，于是生成 `CssVar<String>`；而
    // `CssVar<T>` 的校验靠 `T: ValidFor<props::X>` 转发，`String` 不是
    // `ValidFor<props::Color>`——配置驱动的主题色恰恰不能用在 `color()` 上，
    // 与文档承诺的正好相反。现在类型由 `[theme.field_types]` 指定，
    // 不写就按取值猜（像十六进制颜色的用 `Hex`）。
    let mut fields = def.fields.clone();
    // 配置驱动时，各字段的初值表达式（与 `fields` 一一对应）
    let mut config_defaults: Vec<TokenStream> = Vec::new();
    if fields.is_empty()
        && let Some(cfg) = &config
    {
        let mut keys: Vec<&String> = cfg.theme.colors.keys().collect();
        keys.sort();
        for k in keys {
            let raw_value = cfg.theme.colors.get(k).map(String::as_str).unwrap_or("");
            let ty_name = match cfg.theme.field_types.get(k) {
                Some(t) => t.clone(),
                None => infer_field_type(raw_value).to_string(),
            };
            let field_ty = resolve_field_type(&ty_name, &__silex, k, &def.name)?;
            config_defaults.push(default_expr_for(
                &ty_name, raw_value, &__silex, k, &def.name,
            )?);

            let rust_name = k.replace('-', "_");
            let field_ident = syn::Ident::new(&rust_name, proc_macro2::Span::call_site());
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
    let mut patch_to_css_items = Vec::new();

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
        patch_to_css_items.push(quote! {
            if let Some(value) = &self.#f_name {
                ::std::write!(f, "{}: {};", #css_var_name, value)?;
            }
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

    // 配置驱动时用配置里的取值当默认值，而不是 `#[derive(Default)]` 的全零值
    let (derives, default_impl) = if config_defaults.is_empty() {
        (quote! { #[derive(Clone, Debug, Default)] }, quote! {})
    } else {
        let inits = field_idents
            .iter()
            .zip(config_defaults.iter())
            .map(|(f, v)| quote! { #f: #v });
        (
            quote! { #[derive(Clone, Debug)] },
            quote! {
                impl ::core::default::Default for #name {
                    fn default() -> Self {
                        Self { #(#inits),* }
                    }
                }
            },
        )
    };

    Ok(quote! {
        #derives
        #(#filtered_attrs)*
        #vis struct #name { #(#struct_fields),* }

        #default_impl

        impl #name {
            #(#const_impl_items)*
        }

        #[allow(non_camel_case_types)]
        pub trait #trait_name { #(#trait_decl_items)* }

        #[allow(non_camel_case_types)]
        impl #trait_name for #name { #(#trait_impl_items)* }

        impl #__silex::css::theme::ThemeType for #name {}

        impl #__silex::css::theme::ThemeToCss for #name {
            fn get_variable_values(&self) -> Vec<String> { vec![ #( self.#field_idents.to_string() ),* ] }
            fn get_variable_names() -> &'static [&'static str] { &[ #( #css_vars ),* ] }
        }

        // `Display` 就是「这个主题的 CSS 变量声明」。此前中间还隔了一个
        // `ThemeToCss::to_css_variables()`，而那个方法的唯一调用方就是这里
        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #( ::std::write!(f, "{}", #to_css_items)?; )*
                ::std::result::Result::Ok(())
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

        impl ::std::fmt::Display for #patch_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #( #patch_to_css_items )*
                ::std::result::Result::Ok(())
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

    /// 配置驱动的字段类型此前被硬编码成 `String`，于是生成 `CssVar<String>`；
    /// 而 `String` 不是 `ValidFor<props::Color>`——配置里的主题色恰恰不能用在
    /// `color()` 上。现在像颜色的取值默认给 `Hex`。
    #[test]
    fn config_driven_color_fields_are_typed_as_hex() {
        if !config_colors_available() {
            return;
        }
        let out = bridge_theme_impl(quote! { pub struct T {} })
            .unwrap()
            .to_string();
        assert!(out.contains("types :: Hex"), "{out}");
        assert!(
            !out.contains("brand_primary : :: std :: string :: String"),
            "{out}"
        );
    }

    /// `[theme.field_types]` 指定量纲类型时，配置里的取值必须真的落进初值。
    /// 走 `Default::default()` 的话 `radius = "8px"` 会变成 `0px`——配置被
    /// 静默丢掉。
    #[test]
    fn a_dimension_field_keeps_the_configured_value() {
        assert_eq!(parse_unit_value("8px", "Px"), Some(8.0));
        assert_eq!(parse_unit_value(" 1.5rem ", "Rem"), Some(1.5));
        assert_eq!(parse_unit_value("50%", "Percent"), Some(50.0));
        assert_eq!(parse_unit_value("300ms", "Ms"), Some(300.0));
        assert_eq!(parse_unit_value("8", "Px"), Some(8.0), "后缀可以省略");
    }

    /// 后缀写错了不能当没看见——`"8rem"` 配 `Px` 静默取 8px 是最坏的结果
    #[test]
    fn a_mismatched_unit_suffix_is_rejected() {
        assert_eq!(parse_unit_value("8rem", "Px"), None);
        assert_eq!(parse_unit_value("#fff", "Px"), None);
        assert_eq!(parse_unit_value("8px 16px", "Px"), None);
    }

    /// 补出了字段却拿不到配好的颜色，等于 `silex.toml` 里那张配色表白写
    #[test]
    fn config_driven_theme_defaults_to_the_configured_colors() {
        if !config_colors_available() {
            return;
        }
        let out = bridge_theme_impl(quote! { pub struct T {} })
            .unwrap()
            .to_string();
        assert!(out.contains("\"#6366f1\""), "{out}");
        assert!(
            out.contains("impl :: core :: default :: Default for T"),
            "{out}"
        );
    }

    #[test]
    fn hex_detection_only_accepts_real_hex_colors() {
        assert!(looks_like_hex("#6366f1"));
        assert!(looks_like_hex("fff"));
        assert!(!looks_like_hex("rgb(1,2,3)"));
        assert!(!looks_like_hex("#12345"));
        assert_eq!(infer_field_type("#6366f1"), "Hex");
        assert_eq!(infer_field_type("var(--x)"), "String");
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
