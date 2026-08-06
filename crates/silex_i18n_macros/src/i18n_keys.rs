use proc_macro2::{Span, TokenStream};
use quote::quote;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env, fs,
    path::PathBuf,
};
use syn::{Attribute, Data, DeriveInput, Fields, LitStr, Path, Type, ext::IdentExt, parse::Result};

#[derive(Default)]
struct ContainerAttrs {
    path: Option<LitStr>,
    runtime_crate: Option<LitStr>,
}

#[derive(Default)]
struct VariantAttrs {
    key: Option<LitStr>,
    count: Option<LitStr>,
}

#[derive(Clone)]
struct MessageSchema {
    placeholders: BTreeSet<String>,
    plural: bool,
}

pub fn derive_i18n_keys(input: DeriveInput) -> Result<TokenStream> {
    let enum_data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "I18nKeys can only be derived for an enum",
            ));
        }
    };
    let container_attrs = parse_container_attrs(&input.attrs)?;
    let path = container_attrs.path.as_ref().ok_or_else(|| {
        syn::Error::new(
            input.ident.span(),
            "I18nKeys requires #[i18n(path = \"locales/en-US.json\")]",
        )
    })?;
    let schemas = load_catalog(path, input.ident.span())?;
    let runtime = resolve_runtime_path(container_attrs.runtime_crate.as_ref(), input.ident.span())?;
    let catalog_source = resolve_catalog_path(path)?.to_string_lossy().into_owned();

    let mut key_arms = Vec::new();
    let mut argument_arms = Vec::new();
    let mut count_arms = Vec::new();

    for variant in &enum_data.variants {
        let variant_attrs = parse_variant_attrs(&variant.attrs)?;
        let key = variant_attrs.key.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(
                &variant.ident,
                "each I18nKeys variant requires #[i18n(key = \"message.key\")]",
            )
        })?;
        let key_value = key.value();
        if key_value.is_empty() {
            return Err(syn::Error::new_spanned(key, "i18n key must not be empty"));
        }

        let schema = schemas.get(&key_value).ok_or_else(|| {
            syn::Error::new_spanned(
                key,
                format!("i18n key '{key_value}' does not exist in the canonical catalog"),
            )
        })?;
        let named_fields = match &variant.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|field| {
                    field.ident.clone().ok_or_else(|| {
                        syn::Error::new_spanned(field, "I18nKeys fields must have names")
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            Fields::Unit => Vec::new(),
            Fields::Unnamed(fields) => {
                return Err(syn::Error::new_spanned(
                    fields,
                    "I18nKeys variants must use named fields, not tuple fields",
                ));
            }
        };
        let field_names = named_fields
            .iter()
            .map(|field| field.unraw().to_string())
            .collect::<BTreeSet<_>>();
        if field_names != schema.placeholders {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                format!(
                    "i18n key '{key_value}' placeholders do not match fields: catalog {:?}, variant {:?}",
                    schema.placeholders, field_names
                ),
            ));
        }

        let count_name = if schema.plural {
            let count_name = variant_attrs
                .count
                .as_ref()
                .map(LitStr::value)
                .unwrap_or_else(|| "count".to_string());
            let count_field = named_fields
                .iter()
                .find(|field| field.unraw() == count_name)
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        &variant.ident,
                        format!("plural i18n key '{key_value}' requires a '{count_name}' field"),
                    )
                })?;
            let count_type = match &variant.fields {
                Fields::Named(fields) => fields
                    .named
                    .iter()
                    .find(|field| field.ident.as_ref() == Some(count_field))
                    .map(|field| &field.ty)
                    .expect("count field was collected from the same named fields"),
                _ => unreachable!("non-named variants were rejected above"),
            };
            if !is_numeric_type(count_type) {
                return Err(syn::Error::new_spanned(
                    count_type,
                    format!("plural count field '{count_name}' must use a numeric Rust type"),
                ));
            }
            Some(count_name)
        } else {
            if let Some(count) = &variant_attrs.count {
                return Err(syn::Error::new_spanned(
                    count,
                    "the count attribute is only valid for plural messages",
                ));
            }
            None
        };

        let variant_ident = &variant.ident;
        let key_arm = match &variant.fields {
            Fields::Unit => quote! { Self::#variant_ident => #key_value },
            Fields::Named(_) => quote! { Self::#variant_ident { .. } => #key_value },
            Fields::Unnamed(_) => unreachable!("tuple variants were rejected above"),
        };
        key_arms.push(key_arm);

        let argument_arm = match &variant.fields {
            Fields::Unit => quote! {
                Self::#variant_ident => ::std::vec::Vec::new()
            },
            Fields::Named(_) => {
                let arguments = named_fields.iter().map(|field| {
                    let name = field.unraw().to_string();
                    quote! {
                        #runtime::Argument::new(#name, #field)
                    }
                });
                quote! {
                    Self::#variant_ident { #(#named_fields),* } =>
                        ::std::vec![#(#arguments),*]
                }
            }
            Fields::Unnamed(_) => unreachable!("tuple variants were rejected above"),
        };
        argument_arms.push(argument_arm);

        let count_arm = match (&variant.fields, count_name) {
            (Fields::Unit, Some(count_name)) => {
                quote! { Self::#variant_ident => ::std::option::Option::Some(#count_name) }
            }
            (Fields::Named(_), Some(count_name)) => {
                quote! { Self::#variant_ident { .. } => ::std::option::Option::Some(#count_name) }
            }
            (Fields::Unit, None) => {
                quote! { Self::#variant_ident => ::std::option::Option::None }
            }
            (Fields::Named(_), None) => {
                quote! { Self::#variant_ident { .. } => ::std::option::Option::None }
            }
            (Fields::Unnamed(_), _) => unreachable!("tuple variants were rejected above"),
        };
        count_arms.push(count_arm);
    }

    let enum_ident = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        const _: &str = include_str!(#catalog_source);

        impl #impl_generics #runtime::I18nVariant for #enum_ident #ty_generics #where_clause {
            fn key(&self) -> &'static str {
                match self {
                    #(#key_arms),*
                }
            }

            fn arguments(&self) -> ::std::vec::Vec<#runtime::Argument> {
                match self {
                    #(#argument_arms),*
                }
            }

            fn count_name(&self) -> ::std::option::Option<&'static str> {
                match self {
                    #(#count_arms),*
                }
            }
        }
    })
}

fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs> {
    let mut parsed = ContainerAttrs::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("i18n")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("path") {
                if parsed.path.is_some() {
                    return Err(meta.error("duplicate i18n path attribute"));
                }
                parsed.path = Some(meta.value()?.parse()?);
                return Ok(());
            }
            if meta.path.is_ident("crate") {
                if parsed.runtime_crate.is_some() {
                    return Err(meta.error("duplicate i18n crate attribute"));
                }
                parsed.runtime_crate = Some(meta.value()?.parse()?);
                return Ok(());
            }
            Err(meta.error("expected `path = \"...\"` or `crate = \"...\"`"))
        })?;
    }
    Ok(parsed)
}

fn parse_variant_attrs(attrs: &[Attribute]) -> Result<VariantAttrs> {
    let mut parsed = VariantAttrs::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("i18n")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("key") {
                if parsed.key.is_some() {
                    return Err(meta.error("duplicate i18n key attribute"));
                }
                parsed.key = Some(meta.value()?.parse()?);
                return Ok(());
            }
            if meta.path.is_ident("count") {
                if parsed.count.is_some() {
                    return Err(meta.error("duplicate i18n count attribute"));
                }
                parsed.count = Some(meta.value()?.parse()?);
                return Ok(());
            }
            Err(meta.error("expected `key = \"...\"` or `count = \"...\"`"))
        })?;
    }
    Ok(parsed)
}

fn resolve_runtime_path(custom: Option<&LitStr>, span: Span) -> Result<Path> {
    if let Some(custom) = custom {
        return syn::parse_str::<Path>(&custom.value()).map_err(|error| {
            syn::Error::new(custom.span(), format!("invalid i18n crate path: {error}"))
        });
    }

    if let Ok(found) = proc_macro_crate::crate_name("silex_i18n") {
        return match found {
            proc_macro_crate::FoundCrate::Itself => syn::parse_str("crate"),
            proc_macro_crate::FoundCrate::Name(name) => syn::parse_str(&format!("::{name}")),
        };
    }
    if let Ok(found) = proc_macro_crate::crate_name("silex") {
        return match found {
            proc_macro_crate::FoundCrate::Itself => syn::parse_str("crate::i18n"),
            proc_macro_crate::FoundCrate::Name(name) => syn::parse_str(&format!("::{name}::i18n")),
        };
    }

    Err(syn::Error::new(
        span,
        "could not find `silex_i18n` or `silex`; add one as a dependency or set #[i18n(crate = \"...\")]",
    ))
}

fn load_catalog(path_lit: &LitStr, span: Span) -> Result<BTreeMap<String, MessageSchema>> {
    let requested_path = path_lit.value();
    let path = resolve_catalog_path(path_lit)?;
    if !path.is_file() {
        return Err(syn::Error::new(
            span,
            format!(
                "could not read i18n catalog '{}': file does not exist",
                requested_path
            ),
        ));
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        syn::Error::new(
            span,
            format!("could not read i18n catalog '{}': {error}", requested_path),
        )
    })?;
    let value: Value = serde_json::from_str(&source).map_err(|error| {
        syn::Error::new(
            span,
            format!("could not parse i18n catalog '{}': {error}", requested_path),
        )
    })?;
    let mut leaves = BTreeMap::new();
    let mut objects = HashSet::new();
    visit_json_value("", &value, &mut leaves, &mut objects, span)?;
    Ok(leaves)
}

fn resolve_catalog_path(path: &LitStr) -> Result<PathBuf> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        syn::Error::new(
            path.span(),
            "CARGO_MANIFEST_DIR is unavailable while expanding I18nKeys",
        )
    })?;
    Ok(PathBuf::from(manifest_dir).join(path.value()))
}

fn visit_json_value(
    path: &str,
    value: &Value,
    leaves: &mut BTreeMap<String, MessageSchema>,
    objects: &mut HashSet<String>,
    span: Span,
) -> Result<()> {
    if let Some(object) = value.as_object() {
        let is_plural = object.keys().any(|key| plural_category(key).is_some());
        if is_plural {
            if !object.contains_key("other") {
                return Err(syn::Error::new(
                    span,
                    format!("plural message '{path}' is missing the other form"),
                ));
            }
            let mut forms = BTreeMap::new();
            for (category, form) in object {
                if plural_category(category).is_none() {
                    return Err(syn::Error::new(
                        span,
                        format!("invalid plural category '{category}' for '{path}'"),
                    ));
                }
                let template = form.as_str().ok_or_else(|| {
                    syn::Error::new(
                        span,
                        format!("plural form '{path}.{category}' must be a string"),
                    )
                })?;
                forms.insert(category, parse_placeholders(path, template, span)?);
            }
            let placeholders = forms
                .values()
                .next()
                .cloned()
                .expect("plural messages always contain other");
            if forms.values().any(|form| form != &placeholders) {
                return Err(syn::Error::new(
                    span,
                    format!("plural message '{path}' must use the same placeholders in every form"),
                ));
            }
            insert_leaf(
                path,
                MessageSchema {
                    placeholders,
                    plural: true,
                },
                leaves,
                objects,
                span,
            )?;
            return Ok(());
        }

        if !path.is_empty() {
            if leaves.contains_key(path) || has_leaf_ancestor(path, leaves) {
                return Err(syn::Error::new(
                    span,
                    format!("catalog path '{path}' is both a message and an object"),
                ));
            }
            objects.insert(path.to_string());
        }
        for (key, child) in object {
            if key.is_empty() {
                return Err(syn::Error::new(
                    span,
                    "catalog object keys must not be empty",
                ));
            }
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            visit_json_value(&child_path, child, leaves, objects, span)?;
        }
        return Ok(());
    }

    let template = value.as_str().ok_or_else(|| {
        syn::Error::new(
            span,
            format!("message '{path}' must be a string or plural object"),
        )
    })?;
    insert_leaf(
        path,
        MessageSchema {
            placeholders: parse_placeholders(path, template, span)?,
            plural: false,
        },
        leaves,
        objects,
        span,
    )
}

fn insert_leaf(
    path: &str,
    schema: MessageSchema,
    leaves: &mut BTreeMap<String, MessageSchema>,
    objects: &HashSet<String>,
    span: Span,
) -> Result<()> {
    if path.is_empty()
        || leaves.contains_key(path)
        || objects.contains(path)
        || has_leaf_ancestor(path, leaves)
    {
        return Err(syn::Error::new(
            span,
            format!("catalog path '{path}' is duplicated or collides with an object"),
        ));
    }
    leaves.insert(path.to_string(), schema);
    Ok(())
}

fn has_leaf_ancestor(path: &str, leaves: &BTreeMap<String, MessageSchema>) -> bool {
    let mut end = path.len();
    while let Some(separator) = path[..end].rfind('.') {
        if leaves.contains_key(&path[..separator]) {
            return true;
        }
        end = separator;
    }
    false
}

fn parse_placeholders(key: &str, template: &str, span: Span) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let bytes = template.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                let close = template[index + 1..]
                    .find('}')
                    .map(|offset| index + 1 + offset)
                    .ok_or_else(|| {
                        syn::Error::new(
                            span,
                            format!("placeholder in '{key}' is missing a closing brace"),
                        )
                    })?;
                let name = &template[index + 1..close];
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    || name
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_digit())
                {
                    return Err(syn::Error::new(
                        span,
                        format!("invalid placeholder '{name}' in '{key}'"),
                    ));
                }
                names.insert(name.to_string());
                index = close + 1;
            }
            b'}' => {
                return Err(syn::Error::new(
                    span,
                    format!("placeholder in '{key}' has an unexpected closing brace"),
                ));
            }
            _ => index += 1,
        }
    }
    Ok(names)
}

fn plural_category(value: &str) -> Option<&'static str> {
    match value {
        "zero" => Some("zero"),
        "one" => Some("one"),
        "two" => Some("two"),
        "few" => Some("few"),
        "many" => Some("many"),
        "other" => Some("other"),
        _ => None,
    }
}

fn is_numeric_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            matches!(
                path.path.segments[0].ident.to_string().as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
            )
        }
        Type::Reference(reference) => is_numeric_type(&reference.elem),
        Type::Paren(paren) => is_numeric_type(&paren.elem),
        _ => false,
    }
}
