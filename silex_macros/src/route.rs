use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Error, Fields, Member};

struct RouteDef {
    variant_ident: syn::Ident,
    fields: Fields,
    path_segments: Vec<Segment>,
    is_wildcard: bool,
    // 如果存在嵌套路由字段，存储其成员标识符 (字段名或索引)
    nested_field: Option<Member>,
}

enum Segment {
    Static(String),
    Param(String), // name without ':'
}

pub fn derive_route_impl(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;

    let variants = match input.data {
        Data::Enum(ref data) => &data.variants,
        _ => {
            return Err(Error::new_spanned(
                input,
                "Route derive only supports Enums",
            ));
        }
    };

    let mut route_defs = Vec::new();

    for variant in variants {
        let route_attr = variant
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("route"));

        let route_path = if let Some(attr) = route_attr {
            parse_route_attr(attr)?
        } else {
            return Err(Error::new_spanned(
                variant,
                "Missing #[route(\"...\")] attribute",
            ));
        };

        let (segments, is_wildcard) = parse_path_segments(&route_path);

        // 检测嵌套字段
        let nested_field = detect_nested_field(&variant.fields, &segments)?;

        route_defs.push(RouteDef {
            variant_ident: variant.ident.clone(),
            fields: variant.fields.clone(),
            path_segments: segments,
            is_wildcard,
            nested_field,
        });
    }

    let match_arms = generate_match_arms(name, &route_defs)?;
    let to_path_arms = generate_to_path_arms(name, &route_defs)?;

    let expanded = quote! {
        impl ::silex::router::Routable for #name {
            fn match_path(path: &str) -> Option<Self> {
                // 预处理路径：去除两端斜杠，分割
                let clean_path = path.trim_matches('/');
                let segments: Vec<&str> = if clean_path.is_empty() {
                    Vec::new()
                } else {
                    clean_path.split('/').filter(|s| !s.is_empty()).collect()
                };

                #match_arms

                None
            }

            fn to_path(&self) -> String {
                match self {
                    #to_path_arms,
                    // 如果是不可达的（理论上 to_path_arms 覆盖所有变体），返回 /
                    _ => "/".to_string()
                }
            }
        }
    };

    Ok(expanded)
}

fn parse_route_attr(attr: &Attribute) -> syn::Result<String> {
    attr.parse_nested_meta(|meta| Err(meta.error("We use parse_args manually")))
        .unwrap_or(());

    let lit: syn::LitStr = attr
        .parse_args()
        .map_err(|_| Error::new_spanned(attr, "Expected string literal in #[route(...)]"))?;
    Ok(lit.value())
}

fn parse_path_segments(path: &str) -> (Vec<Segment>, bool) {
    let clean = path.trim_matches('/');
    if clean == "*" {
        return (Vec::new(), true);
    }

    let mut segments = Vec::new();
    let mut wildcard = false;

    for s in clean.split('/') {
        if s.is_empty() {
            continue;
        }
        if s == "*" {
            wildcard = true;
            break;
        }
        if s.starts_with(':') {
            segments.push(Segment::Param(s[1..].to_string()));
        } else {
            segments.push(Segment::Static(s.to_string()));
        }
    }

    (segments, wildcard)
}

fn detect_nested_field(fields: &Fields, segments: &[Segment]) -> syn::Result<Option<Member>> {
    let param_names: Vec<&str> = segments
        .iter()
        .filter_map(|s| match s {
            Segment::Param(n) => Some(n.as_str()),
            _ => None,
        })
        .collect();

    let mut nested = None;

    match fields {
        Fields::Named(named) => {
            for field in &named.named {
                let name = field.ident.as_ref().unwrap().to_string();
                let is_param = param_names.contains(&name.as_str());

                // Check for #[nested] attribute
                let is_marked_nested = field
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("nested"));

                if is_param {
                    if is_marked_nested {
                        return Err(Error::new_spanned(
                            field,
                            "Field cannot be both a route parameter and a nested route.",
                        ));
                    }
                    // It's a param, normal processing
                } else {
                    // It is NOT a param.
                    // According to "pure" route enum rules, it MUST be a nested route.
                    // To solve inference issues, we REQUIRE #[nested] for named fields.

                    if is_marked_nested {
                        if nested.is_some() {
                            return Err(Error::new_spanned(
                                field,
                                "Multiple nested route fields defined. Only one allowed.",
                            ));
                        }
                        nested = Some(Member::Named(field.ident.clone().unwrap()));
                    } else {
                        // Not a param, not marked nested -> Error
                        // This prevents typos in param names being inferred as nested routes.
                        return Err(Error::new_spanned(
                            field,
                            format!(
                                "Field '{}' is not a route parameter (missing in path) and not marked as #[nested]. Route Enums must be pure.",
                                name
                            ),
                        ));
                    }
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            // Tuple variants don't support named params in this macro implementation usually.
            if !param_names.is_empty() {
                return Err(Error::new_spanned(
                    fields,
                    "Route params only supported with Named Fields",
                ));
            }

            for (i, _field) in unnamed.unnamed.iter().enumerate() {
                // For tuple variants, any field implies nested because no params are allowed.
                // We accept it as nested regardless of attribute.
                // But we still enforce existing rule: only 1 field allowed.

                if nested.is_some() {
                    return Err(Error::new_spanned(
                        fields,
                        "Multiple fields in tuple variant. Only one nested route field allowed.",
                    ));
                }

                // We accept it as nested regardless of attribute, but if they used attribute, that's fine too.
                // If they have multiple fields, the check above catches it.
                nested = Some(Member::Unnamed(syn::Index {
                    index: i as u32,
                    span: proc_macro2::Span::call_site(),
                }));
            }
        }
        Fields::Unit => {}
    }

    Ok(nested)
}

fn generate_match_arms(enum_name: &syn::Ident, defs: &[RouteDef]) -> syn::Result<TokenStream> {
    let mut arms = Vec::new();

    for def in defs {
        let variant_ident = &def.variant_ident;
        let expected_len = def.path_segments.len();
        let is_wildcard = def.is_wildcard;
        let is_nested = def.nested_field.is_some();

        let mut checks = Vec::new();
        let mut param_parsing = Vec::new();

        // 1. 基本长度检查
        if is_wildcard {
            // 通配符：只要长度够就行
            checks.push(quote! {
                if segments.len() < #expected_len { return None; }
            });
        } else if is_nested {
            // 嵌套路由：这是前缀匹配，segments 长度至少要覆盖当前层的定义的段
            checks.push(quote! {
                if segments.len() < #expected_len { return None; }
            });
        } else {
            // 叶子路由：必须精确匹配
            checks.push(quote! {
                if segments.len() != #expected_len { return None; }
            });
        }

        // 2. 段匹配逻辑
        for (idx, seg) in def.path_segments.iter().enumerate() {
            match seg {
                Segment::Static(s) => {
                    checks.push(quote! {
                        if segments[#idx] != #s { return None; }
                    });
                }
                Segment::Param(name) => {
                    let ident = format_ident!("{}", name);
                    // 查找字段类型
                    let field_ty = find_field_type(&def.fields, name).ok_or_else(|| {
                        Error::new_spanned(
                            variant_ident,
                            format!("Route param '{}' not found in variant fields", name),
                        )
                    })?;

                    param_parsing.push(quote! {
                        let #ident = segments[#idx].parse::<#field_ty>().ok()?;
                    });
                }
            }
        }

        // 3. 构造变体
        let construct_variant = match &def.fields {
            Fields::Named(_) => {
                let mut inits = Vec::new();
                // 添加参数字段
                for s in &def.path_segments {
                    if let Segment::Param(name) = s {
                        let ident = format_ident!("{}", name);
                        inits.push(quote! { #ident: #ident });
                    }
                }

                // 处理嵌套字段
                if let Some(Member::Named(nested_name)) = &def.nested_field {
                    inits.push(quote! { #nested_name: sub_route });
                }

                quote! {
                    Some(#enum_name::#variant_ident { #(#inits),* })
                }
            }
            Fields::Unnamed(_) => {
                // 如果是 Unnamed，我们之前禁止了 Param，所以只能是 Nested
                if def.nested_field.is_some() {
                    quote! {
                        Some(#enum_name::#variant_ident(sub_route))
                    }
                } else {
                    // 既无 Param 也无 Nested，可能是空元组
                    quote! {
                        Some(#enum_name::#variant_ident)
                    }
                }
            }
            Fields::Unit => quote! {
                Some(#enum_name::#variant_ident)
            },
        };

        // 4. 嵌套路由递归逻辑
        let final_logic = if let Some(nested_member) = &def.nested_field {
            // 获取嵌套字段的类型
            let nested_ty = match &def.fields {
                Fields::Named(f) => {
                    let target_ident = match nested_member {
                        Member::Named(n) => n,
                        _ => {
                            return Err(Error::new_spanned(
                                variant_ident,
                                "Internal error: Mismatched field type",
                            ));
                        }
                    };

                    let field = f
                        .named
                        .iter()
                        .find(|field| field.ident.as_ref() == Some(target_ident))
                        .ok_or_else(|| {
                            Error::new_spanned(variant_ident, "Nested field not found")
                        })?;

                    field.ty.clone()
                }
                Fields::Unnamed(f) => {
                    // Unnamed 仅允许一个字段作为 nested
                    f.unnamed
                        .first()
                        .ok_or_else(|| {
                            Error::new_spanned(variant_ident, "Tuple variant missing field")
                        })?
                        .ty
                        .clone()
                }
                Fields::Unit => {
                    return Err(Error::new_spanned(
                        variant_ident,
                        "Unit struct cannot have nested field",
                    ));
                }
            };

            quote! {
                // 提取剩余路径
                // 如果当前层消耗了 expected_len 个段，剩余的从 expected_len 开始
                let remaining_segments = &segments[#expected_len..];
                let remaining_path = remaining_segments.join("/");

                // 递归匹配
                if let Some(sub_route) = <#nested_ty as ::silex::router::Routable>::match_path(&remaining_path) {
                    #construct_variant
                } else {
                    None
                }
            }
        } else {
            // 非嵌套，直接返回
            construct_variant
        };

        arms.push(quote! {
            if let Some(res) = (|| {
                #(#checks)*
                #(#param_parsing)*
                #final_logic
            })() {
                return Some(res);
            }
        });
    }

    Ok(quote! {
        #(#arms)*
    })
}

fn generate_to_path_arms(enum_name: &syn::Ident, defs: &[RouteDef]) -> syn::Result<TokenStream> {
    let mut arms = Vec::new();

    for def in defs {
        let variant_ident = &def.variant_ident;
        let mut format_string = String::new();
        let mut format_args = Vec::new(); // 参数变量名
        let mut field_bindings = Vec::new(); // match 结构的绑定

        // 构建当前层的路径格式
        for seg in &def.path_segments {
            format_string.push('/');
            match seg {
                Segment::Static(s) => format_string.push_str(s),
                Segment::Param(name) => {
                    format_string.push_str("{}");
                    let ident = format_ident!("{}", name);
                    format_args.push(quote! { #ident });
                    field_bindings.push(ident); // 绑定参数字段
                }
            }
        }

        // 处理嵌套字段的 to_path
        if let Some(mapped_member) = &def.nested_field {
            // 需要在 format_string 后追加 "{}"
            // 并在 format_args 中追加 sub.to_path()
            match mapped_member {
                Member::Named(_) => {
                    // 绑定名为 sub_route_val
                    // 既然我们后面硬编码了 sub_route_val，这里不需要 format_ident

                    // 在 Named field match 中: Ident: sub_route_val
                    format_string.push_str("{}"); // 嵌套路径不加前导 / (子路由 to_path 会返回 absolute /... ?) 

                    // 在 args 里处理：

                    format_args.push(quote! {
                        {
                            // 处理路径拼接
                            // 如果是根式父路径 "/"，直接忽略，除非子路径为空
                            // 这里比较 trick。
                            // 简单点：生成两个字符串，在运行时拼接
                            sub_route_val.to_path()
                        }
                    });
                }
                Member::Unnamed(_) => {
                    // 绑定名为 sub_route_val
                    // Unnamed 只有一个字段
                    format_args.push(quote! { sub_route_val.to_path() });
                }
            }
        } else {
            if format_string.is_empty() {
                format_string.push('/');
            }
        }

        // 构造匹配模式
        let destruct = match &def.fields {
            Fields::Named(_) => {
                let mut binds = Vec::new();
                for b in &field_bindings {
                    binds.push(quote! { #b });
                }
                if let Some(Member::Named(nested_name)) = &def.nested_field {
                    binds.push(quote! { #nested_name: sub_route_val });
                }

                if binds.is_empty() {
                    quote! { { .. } }
                } else {
                    quote! { { #(#binds),* } }
                }
            }
            Fields::Unnamed(_) => {
                // 只有嵌套字段
                if def.nested_field.is_some() {
                    quote! { (sub_route_val) }
                } else {
                    quote! { (..) }
                }
            }
            Fields::Unit => quote! {},
        };

        // 生成最终的 format 调用
        // 对于嵌套情况，我们需要特殊的拼接逻辑以避免 //
        if def.nested_field.is_some() {
            // 分离 args
            let child_arg = format_args.pop().unwrap(); // last one is child path
            let base_args = format_args;

            // 移除 format_string 最后的 {} (这是之前专门为 nested 字段添加的占位符)
            // 因为我们将手动拼接 child path，所以这里的 format 只需要负责 base path
            let base_len = format_string.len();
            let base_fmt = if base_len >= 2 && &format_string[base_len - 2..] == "{}" {
                &format_string[..base_len - 2]
            } else {
                &format_string
            };

            arms.push(quote! {
                #enum_name::#variant_ident #destruct => {
                    let base = format!(#base_fmt, #(#base_args),*);
                    let child = #child_arg;
                    if base == "/" {
                        child
                    } else if child.starts_with('/') {
                         format!("{}{}", base, child)
                    } else {
                         format!("{}{}", base, child)
                    }
                }
            });
        } else {
            // 普通情况
            arms.push(quote! {
                #enum_name::#variant_ident #destruct => format!(#format_string, #(#format_args),*)
            });
        }
    }

    Ok(quote! {
        #(#arms),*
    })
}

fn find_field_type<'a>(fields: &'a Fields, name: &str) -> Option<&'a syn::Type> {
    match fields {
        Fields::Named(named) => {
            for f in &named.named {
                if let Some(ident) = &f.ident {
                    if ident == name {
                        return Some(&f.ty);
                    }
                }
            }
            None
        }
        _ => None,
    }
}
