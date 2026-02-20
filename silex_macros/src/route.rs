use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Error, Fields, Member, Token};

struct RouteDef {
    variant_ident: syn::Ident,
    route_attr_span: proc_macro2::Span,
    fields: Fields,
    path_segments: Vec<Segment>,
    is_wildcard: bool,
    // 如果存在嵌套路由字段，存储其成员标识符 (字段名或索引)
    nested_field: Option<Member>,
    view: Option<syn::Path>,
    guards: Vec<syn::Path>,
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
                &input.ident,
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

        let (route_path, view_component, guards, route_attr_span) = if let Some(attr) = route_attr {
            let (p, v, g) = parse_route_attr(attr)?;
            (p, v, g, attr.span())
        } else {
            return Err(Error::new_spanned(
                &variant.ident,
                "Missing #[route(\"...\")] attribute",
            ));
        };

        let (segments, is_wildcard) = parse_path_segments(&route_path);

        // 检测嵌套字段
        let nested_field = detect_nested_field(&variant.fields, &segments, route_attr_span)?;

        route_defs.push(RouteDef {
            variant_ident: variant.ident.clone(),
            route_attr_span,
            fields: variant.fields.clone(),
            path_segments: segments,
            is_wildcard,
            nested_field,
            view: view_component,
            guards,
        });
    }

    let match_arms = generate_match_arms(name, &route_defs)?;
    let to_path_arms = generate_to_path_arms(name, &route_defs)?;
    let render_arms = generate_render_arms(name, &route_defs)?;

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

        impl ::silex::router::RouteView for #name {
            fn render(&self) -> ::silex::dom::view::AnyView {
                use ::silex::dom::view::View;
                match self {
                    #render_arms
                }
            }
        }
    };

    Ok(expanded)
}

fn parse_route_attr(attr: &Attribute) -> syn::Result<(String, Option<syn::Path>, Vec<syn::Path>)> {
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        let lit: syn::LitStr = input.parse()?;
        let path = lit.value();

        let mut view = None;
        let mut guards = Vec::new();

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }

            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "view" {
                view = Some(input.parse()?);
            } else if key == "guard" {
                if input.peek(syn::token::Bracket) {
                    let content;
                    syn::bracketed!(content in input);
                    let list = content.parse_terminated(syn::Path::parse, Token![,])?;
                    guards.extend(list);
                } else {
                    guards.push(input.parse()?);
                }
            } else {
                return Err(Error::new_spanned(
                    &key,
                    "Expected 'view' or 'guard' parameter",
                ));
            }
        }

        Ok((path, view, guards))
    })
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
        if let Some(stripped) = s.strip_prefix(':') {
            segments.push(Segment::Param(stripped.to_string()));
        } else {
            segments.push(Segment::Static(s.to_string()));
        }
    }

    (segments, wildcard)
}

fn detect_nested_field(
    fields: &Fields,
    segments: &[Segment],
    route_attr_span: proc_macro2::Span,
) -> syn::Result<Option<Member>> {
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
            if !param_names.is_empty() {
                return Err(Error::new(
                    route_attr_span,
                    "Route params only supported with Named Fields",
                ));
            }

            for (i, field) in unnamed.unnamed.iter().enumerate() {
                // For tuple variants, any field implies nested because no params are allowed.
                // We accept it as nested regardless of attribute.
                // But we still enforce existing rule: only 1 field allowed.

                if nested.is_some() {
                    return Err(Error::new_spanned(
                        field,
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

/// Trie Node for route matching
#[derive(Default)]
struct Node {
    static_children: BTreeMap<String, Node>,
    param_child: Option<Box<Node>>,

    // Indices in `route_defs` that match at this node
    exact_matches: Vec<usize>,
    wildcard_matches: Vec<usize>,
    nested_matches: Vec<usize>,
}

impl Node {
    fn insert(
        &mut self,
        segments: &[Segment],
        route_idx: usize,
        is_wildcard: bool,
        is_nested: bool,
    ) {
        if segments.is_empty() {
            if is_wildcard {
                self.wildcard_matches.push(route_idx);
            } else if is_nested {
                self.nested_matches.push(route_idx);
            } else {
                self.exact_matches.push(route_idx);
            }
            return;
        }

        match &segments[0] {
            Segment::Static(s) => {
                self.static_children.entry(s.clone()).or_default().insert(
                    &segments[1..],
                    route_idx,
                    is_wildcard,
                    is_nested,
                );
            }
            Segment::Param(_) => {
                if self.param_child.is_none() {
                    self.param_child = Some(Box::new(Node::default()));
                }
                self.param_child.as_mut().unwrap().insert(
                    &segments[1..],
                    route_idx,
                    is_wildcard,
                    is_nested,
                );
            }
        }
    }
}

fn generate_match_arms(enum_name: &syn::Ident, defs: &[RouteDef]) -> syn::Result<TokenStream> {
    let mut root = Node::default();

    for (i, def) in defs.iter().enumerate() {
        root.insert(
            &def.path_segments,
            i,
            def.is_wildcard,
            def.nested_field.is_some(),
        );
    }

    let match_logic = generate_node_logic(&root, 0, defs, enum_name)?;

    Ok(match_logic)
}

fn generate_node_logic(
    node: &Node,
    depth: usize,
    defs: &[RouteDef],
    enum_name: &syn::Ident,
) -> syn::Result<TokenStream> {
    // 1. 处理路径结束的情况 (segments.len() == depth)
    let check_end_logic = {
        let mut attempts = Vec::new();
        // Exact matches
        for &idx in &node.exact_matches {
            attempts.push(generate_route_handler(&defs[idx], enum_name)?);
        }
        // Wildcard / Nested can also match empty remainder
        for &idx in &node.wildcard_matches {
            attempts.push(generate_route_handler(&defs[idx], enum_name)?);
        }
        for &idx in &node.nested_matches {
            attempts.push(generate_route_handler(&defs[idx], enum_name)?);
        }

        quote! {
            if segments.len() == #depth {
                #(#attempts)*
                return None;
            }
        }
    };

    // 2. Static Children Matching
    let match_static = if !node.static_children.is_empty() {
        let mut static_arms = Vec::new();
        for (key, child) in &node.static_children {
            let child_logic = generate_node_logic(child, depth + 1, defs, enum_name)?;
            static_arms.push(quote! {
                #key => {
                    #child_logic
                }
            });
        }
        quote! {
            match segments[#depth] {
                #(#static_arms),*
                _ => {}
            }
        }
    } else {
        quote! {}
    };

    // 3. Param Child Matching
    let match_param = if let Some(child) = &node.param_child {
        let child_logic = generate_node_logic(child, depth + 1, defs, enum_name)?;
        quote! {
            {
                #child_logic
            }
        }
    } else {
        quote! {}
    };

    // 4. Wildcard / Nested (consuming remaining segments)
    // Runs if we have segments left but failed to match static/param structure fully down the tree,
    // OR if we are at this node and static/param didn't match current segment.
    // Note: If `match_static` or `match_param` matched, they would have executed `check_end_logic` deeper in logical tree
    // or returned Some. If checks failed (e.g. param parsing types), they return None and fall through.

    let mut fallback_attempts = Vec::new();
    for &idx in &node.wildcard_matches {
        fallback_attempts.push(generate_route_handler(&defs[idx], enum_name)?);
    }
    for &idx in &node.nested_matches {
        fallback_attempts.push(generate_route_handler(&defs[idx], enum_name)?);
    }

    Ok(quote! {
        // Check if we ran out of segments physically at this node
        #check_end_logic

        // We have segment at `depth`. Try specific children.
        #match_static
        #match_param

        // Fallback to wildcard/nested at this level
        #(#fallback_attempts)*
    })
}

fn generate_route_handler(def: &RouteDef, enum_name: &syn::Ident) -> syn::Result<TokenStream> {
    let variant_ident = &def.variant_ident;
    let expected_len = def.path_segments.len();

    let mut param_parsing = Vec::new();

    // Param Parsing
    // We trust the tree structure, so we just grab params by their known indices.
    for (idx, seg) in def.path_segments.iter().enumerate() {
        if let Segment::Param(name) = seg {
            let ident = format_ident!("{}", name);
            let field_ty = find_field_type(&def.fields, name).ok_or_else(|| {
                Error::new(
                    def.route_attr_span,
                    format!("Route param '{}' not found in variant fields", name),
                )
            })?;

            param_parsing.push(quote! {
                let #ident = segments[#idx].parse::<#field_ty>().ok()?;
            });
        }
    }

    // Construct Variant
    let construct_variant = match &def.fields {
        Fields::Named(_) => {
            let mut inits = Vec::new();
            for s in &def.path_segments {
                if let Segment::Param(name) = s {
                    let ident = format_ident!("{}", name);
                    inits.push(quote! { #ident: #ident });
                }
            }
            if let Some(Member::Named(nested_name)) = &def.nested_field {
                inits.push(quote! { #nested_name: sub_route });
            }
            quote! { Some(#enum_name::#variant_ident { #(#inits),* }) }
        }
        Fields::Unnamed(_) => {
            if def.nested_field.is_some() {
                quote! { Some(#enum_name::#variant_ident(sub_route)) }
            } else {
                quote! { Some(#enum_name::#variant_ident) }
            }
        }
        Fields::Unit => quote! { Some(#enum_name::#variant_ident) },
    };

    // Final Logic (Nested vs Regular)
    let final_logic = if let Some(nested_member) = &def.nested_field {
        let nested_ty = match &def.fields {
            Fields::Named(f) => {
                let target_ident = match nested_member {
                    Member::Named(n) => n,
                    _ => return Err(Error::new_spanned(variant_ident, "Internal error")),
                };
                f.named
                    .iter()
                    .find(|field| field.ident.as_ref() == Some(target_ident))
                    .unwrap()
                    .ty
                    .clone()
            }
            Fields::Unnamed(f) => f.unnamed.first().unwrap().ty.clone(),
            _ => {
                return Err(Error::new(def.route_attr_span, "Unit struct nested error"));
            }
        };

        quote! {
            let remaining_segments = &segments[#expected_len..];
            let remaining_path = remaining_segments.join("/");
            if let Some(sub_route) = <#nested_ty as ::silex::router::Routable>::match_path(&remaining_path) {
                #construct_variant
            } else {
                None
            }
        }
    } else {
        construct_variant
    };

    Ok(quote! {
        if let Some(res) = (|| {
            #(#param_parsing)*
            #final_logic
        })() {
            return Some(res);
        }
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
        } else if format_string.is_empty() {
            format_string.push('/');
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

                    // 智能路径拼接，避免双重斜杠或从缺斜杠
                    let base_clean = base.trim_end_matches('/');
                    // 子路径通常由 Routable::to_path 生成，以 / 开头
                    // 但我们也处理不以 / 开头的情况
                    let child_clean = child.strip_prefix('/').unwrap_or(&child);

                    if base_clean.is_empty() {
                         format!("/{}", child_clean)
                    } else {
                         format!("{}/{}", base_clean, child_clean)
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
                if let Some(ident) = &f.ident
                    && ident == name
                {
                    return Some(&f.ty);
                }
            }
            None
        }
        _ => None,
    }
}

fn generate_render_arms(enum_name: &syn::Ident, defs: &[RouteDef]) -> syn::Result<TokenStream> {
    let mut arms = Vec::new();

    for def in defs {
        let variant_ident = &def.variant_ident;

        if let Some(view_component) = &def.view {
            // 如果指定了 view，我们需要构建组件
            // 策略：所有字段必须是 Named Field (除了可能的 unique nested field in tuple?)
            // 我们通过字段名将 variant 的字段传给 Component::new().field(val)

            match &def.fields {
                Fields::Named(named) => {
                    let mut props_setters = Vec::new();
                    let mut field_bindings = Vec::new();

                    for field in &named.named {
                        let fname = field.ident.as_ref().unwrap();
                        field_bindings.push(fname.clone());
                        // Component::new().prop(prop)
                        props_setters.push(quote! { .#fname(#fname.clone()) });
                    }

                    let mut view_expr = quote! {
                        #view_component()
                            #(#props_setters)*
                            .into_any()
                    };

                    // 应用 Guard (从内向外包裹)
                    // Guard(children) -> Guard().children(move || view)
                    for guard in def.guards.iter().rev() {
                        view_expr = quote! {
                            #guard()
                                .children(move || #view_expr)
                                .into_any()
                        };
                    }

                    arms.push(quote! {
                        #enum_name::#variant_ident { #(#field_bindings),* } => {
                            #view_expr
                        }
                    });
                }
                Fields::Unit => {
                    let mut view_expr = quote! {
                        #view_component().into_any()
                    };

                    for guard in def.guards.iter().rev() {
                        view_expr = quote! {
                            #guard()
                                .children(move || #view_expr)
                                .into_any()
                        };
                    }

                    arms.push(quote! {
                        #enum_name::#variant_ident => #view_expr
                    });
                }
                Fields::Unnamed(unnamed) => {
                    // 对于 Tuple Variant，我们只允许一种情况：
                    // 只有一个字段，且它是 nested route。
                    // 并且我们需要猜测 prop 名字？
                    // 为了安全起见，我们暂不支持 Tuple Variant 的自动绑定，要求用户改用 Named Variant
                    // 除非... 没有任何字段（那匹配 Unit）
                    if unnamed.unnamed.is_empty() {
                        let mut view_expr = quote! {
                            #view_component().into_any()
                        };

                        for guard in def.guards.iter().rev() {
                            view_expr = quote! {
                                #guard()
                                    .children(move || #view_expr)
                                    .into_any()
                            };
                        }

                        arms.push(quote! {
                            #enum_name::#variant_ident() => #view_expr
                        });
                    } else {
                        return Err(Error::new_spanned(
                            unnamed,
                            "Route view binding currently only supports Named Fields (e.g., Variant { id: String }) to map parameters to component props. Please convert your Tuple Variant to a Struct Variant.",
                        ));
                    }
                }
            }
        } else {
            // 如果没有指定 view，返回 Empty
            // 根据字段类型生成正确的匹配模式
            let pattern = match &def.fields {
                Fields::Named(_) => quote! { #enum_name::#variant_ident { .. } },
                Fields::Unnamed(_) => quote! { #enum_name::#variant_ident(..) },
                Fields::Unit => quote! { #enum_name::#variant_ident },
            };

            arms.push(quote! {
                #pattern => ::silex::dom::view::AnyView::new(())
            });
        }
    }

    Ok(quote! {
        #(#arms),*
    })
}
