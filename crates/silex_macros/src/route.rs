use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::parse::{Parse, ParseStream};
use syn::{Error, ExprClosure, Ident, LitStr, Token, Type, Visibility};

struct RouterInput {
    visibility: Visibility,
    name: Ident,
    nodes: syn::punctuated::Punctuated<RouteNodeInput, Token![,]>,
}

impl Parse for RouterInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let visibility = if input.peek(Token![pub]) {
            input.parse()?
        } else {
            Visibility::Inherited
        };
        input.parse::<Token![enum]>()?;
        let name = input.parse()?;
        let content;
        syn::braced!(content in input);
        let nodes = content.parse_terminated(RouteNodeInput::parse, Token![,])?;
        Ok(Self {
            visibility,
            name,
            nodes,
        })
    }
}

struct RouteNodeInput {
    name: Ident,
    kind: RouteNodeKindInput,
}

enum RouteNodeKindInput {
    Leaf {
        fields: Vec<RouteField>,
        path: LitStr,
    },
    Nested {
        prefix: LitStr,
        layout: ExprClosure,
        children: syn::punctuated::Punctuated<RouteNodeInput, Token![,]>,
    },
}

#[derive(Clone)]
struct RouteField {
    name: Ident,
    ty: Type,
}

impl Parse for RouteNodeInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse::<Ident>()?;
        if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            return Ok(Self {
                name,
                kind: RouteNodeKindInput::Leaf {
                    fields: Vec::new(),
                    path: input.parse()?,
                },
            });
        }

        let fields_or_nested = if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            if looks_like_nested(&content) {
                RouteNodeKindInput::Nested {
                    prefix: parse_nested_prefix(&content)?,
                    layout: parse_nested_layout(&content)?,
                    children: parse_nested_children(&content)?,
                }
            } else {
                RouteNodeKindInput::Leaf {
                    fields: parse_named_fields(&content)?,
                    path: {
                        input.parse::<Token![=>]>()?;
                        input.parse()?
                    },
                }
            }
        } else {
            return Err(Error::new_spanned(
                name,
                "route variant must use `=> \"/path\"` or a nested route body",
            ));
        };

        Ok(Self {
            name,
            kind: fields_or_nested,
        })
    }
}

fn looks_like_nested(content: ParseStream<'_>) -> bool {
    let fork = content.fork();
    let Ok(keyword) = fork.parse::<Ident>() else {
        return false;
    };
    keyword == "prefix"
        && fork.parse::<Token![:]>().is_ok()
        && fork.parse::<LitStr>().is_ok()
        && fork.parse::<Token![;]>().is_ok()
}

fn parse_nested_prefix(content: ParseStream<'_>) -> syn::Result<LitStr> {
    let keyword = content.parse::<Ident>()?;
    if keyword != "prefix" {
        return Err(Error::new_spanned(keyword, "expected `prefix: \"/path\";`"));
    }
    content.parse::<Token![:]>()?;
    let prefix = content.parse()?;
    content.parse::<Token![;]>()?;
    Ok(prefix)
}

fn parse_nested_layout(content: ParseStream<'_>) -> syn::Result<ExprClosure> {
    let keyword = content.parse::<Ident>()?;
    if keyword != "layout" {
        return Err(Error::new_spanned(
            keyword,
            "expected `layout: |ctx, outlet| ...;`",
        ));
    }
    content.parse::<Token![:]>()?;
    let layout: ExprClosure = content.parse()?;
    content.parse::<Token![;]>()?;
    if layout.inputs.len() != 2 {
        return Err(Error::new_spanned(
            layout,
            "nested route layout must accept RouterContext and outlet",
        ));
    }
    Ok(layout)
}

fn parse_nested_children(
    content: ParseStream<'_>,
) -> syn::Result<syn::punctuated::Punctuated<RouteNodeInput, Token![,]>> {
    let keyword = content.parse::<Ident>()?;
    if keyword != "children" {
        return Err(Error::new_spanned(keyword, "expected `children: { ... }`"));
    }
    content.parse::<Token![:]>()?;
    let children_content;
    syn::braced!(children_content in content);
    let children = children_content.parse_terminated(RouteNodeInput::parse, Token![,])?;
    if !content.is_empty() {
        return Err(content.error("unexpected tokens after nested route children"));
    }
    Ok(children)
}

fn parse_named_fields(content: ParseStream<'_>) -> syn::Result<Vec<RouteField>> {
    let mut fields = Vec::new();
    while !content.is_empty() {
        let name = content.parse::<Ident>()?;
        content.parse::<Token![:]>()?;
        let ty = content.parse::<Type>()?;
        if fields.iter().any(|field: &RouteField| field.name == name) {
            return Err(Error::new_spanned(
                name,
                "route variant fields must be unique",
            ));
        }
        fields.push(RouteField { name, ty });
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(fields)
}

#[derive(Clone)]
enum RouteSegment {
    Static { raw: String },
    Param { name: String },
    Wildcard { name: Option<String> },
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum PatternKeySegment {
    Static(String),
    Param,
    Wildcard,
}

struct RouteLeaf {
    name: Ident,
    fields: Vec<RouteField>,
    pattern: String,
    full_pattern: String,
    segments: Vec<RouteSegment>,
}

struct NestedRoute {
    name: Ident,
    prefix: String,
    layout: ExprClosure,
    enum_name: Ident,
    children: RouteTree,
}

enum RouteNode {
    Leaf(RouteLeaf),
    Nested(NestedRoute),
}

struct RouteTree {
    name: Ident,
    visibility: Visibility,
    nodes: Vec<RouteNode>,
}

struct CompileState {
    full_patterns: HashSet<Vec<PatternKeySegment>>,
    enum_names: HashSet<String>,
}

pub fn router_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let input: RouterInput = syn::parse2(input)?;
    let mut state = CompileState {
        full_patterns: HashSet::new(),
        enum_names: HashSet::new(),
    };
    state.enum_names.insert(input.name.to_string());
    let tree = compile_tree(
        input.name,
        input.visibility,
        input.nodes.into_iter().collect(),
        "/",
        &mut state,
    )?;
    let silex = crate::crate_path::silex();
    let definitions = generate_enum_definitions(&tree);
    let implementations = generate_enum_implementations(&silex, &tree, true, &tree);
    Ok(quote! {
        #definitions
        #implementations
    })
}

fn compile_tree(
    name: Ident,
    visibility: Visibility,
    definitions: Vec<RouteNodeInput>,
    parent_prefix: &str,
    state: &mut CompileState,
) -> syn::Result<RouteTree> {
    let mut names = HashSet::new();
    let mut local_patterns = HashSet::new();
    let mut nodes = Vec::with_capacity(definitions.len());

    for definition in definitions {
        if !names.insert(definition.name.to_string()) {
            return Err(Error::new_spanned(
                definition.name,
                "route variants in one enum must be unique",
            ));
        }

        match definition.kind {
            RouteNodeKindInput::Leaf { fields, path } => {
                let (pattern, segments, key) = parse_pattern(&path)?;
                validate_fields(&definition.name, &fields, &segments)?;
                let full_pattern = join_patterns(parent_prefix, &pattern);
                let full_literal = LitStr::new(&full_pattern, path.span());
                let (_, _, full_key) = parse_pattern(&full_literal)?;
                if !local_patterns.insert(key) {
                    return Err(Error::new_spanned(
                        path,
                        "route patterns in one enum must be unique by shape",
                    ));
                }
                if !state.full_patterns.insert(full_key) {
                    return Err(Error::new_spanned(
                        path,
                        "complete route patterns must be unique by shape",
                    ));
                }
                nodes.push(RouteNode::Leaf(RouteLeaf {
                    name: definition.name,
                    fields,
                    pattern,
                    full_pattern,
                    segments,
                }));
            }
            RouteNodeKindInput::Nested {
                prefix,
                layout,
                children,
            } => {
                let prefix_span = prefix.span();
                let (prefix, prefix_segments, _prefix_key) = parse_pattern(&prefix)?;
                if prefix_segments
                    .iter()
                    .any(|segment| !matches!(segment, RouteSegment::Static { .. }))
                {
                    return Err(Error::new_spanned(
                        prefix,
                        "nested route prefixes must contain only static segments",
                    ));
                }
                let synthetic_pattern = if prefix == "/" {
                    String::from("/*")
                } else {
                    format!("{prefix}/*")
                };
                let synthetic_literal = LitStr::new(&synthetic_pattern, prefix_span);
                let (_, _, synthetic_key) = parse_pattern(&synthetic_literal)?;
                if !local_patterns.insert(synthetic_key.clone()) {
                    return Err(Error::new_spanned(
                        prefix,
                        "nested route prefix conflicts with another route pattern",
                    ));
                }
                let full_prefix = join_patterns(parent_prefix, &prefix);
                let full_synthetic = if full_prefix == "/" {
                    String::from("/*")
                } else {
                    format!("{full_prefix}/*")
                };
                let full_synthetic_literal = LitStr::new(&full_synthetic, prefix_span);
                let (_, _, full_synthetic_key) = parse_pattern(&full_synthetic_literal)?;
                if !state.full_patterns.insert(full_synthetic_key) {
                    return Err(Error::new_spanned(
                        prefix,
                        "nested route prefix conflicts with another complete route pattern",
                    ));
                }
                let enum_name = format_ident!("{}Route", definition.name);
                if !state.enum_names.insert(enum_name.to_string()) {
                    return Err(Error::new_spanned(
                        definition.name,
                        "nested route enum names must be unique",
                    ));
                }
                let children = compile_tree(
                    enum_name.clone(),
                    visibility.clone(),
                    children.into_iter().collect(),
                    &full_prefix,
                    state,
                )?;
                nodes.push(RouteNode::Nested(NestedRoute {
                    name: definition.name,
                    prefix,
                    layout,
                    enum_name,
                    children,
                }));
            }
        }
    }

    Ok(RouteTree {
        name,
        visibility,
        nodes,
    })
}

fn validate_fields(
    route_name: &Ident,
    fields: &[RouteField],
    segments: &[RouteSegment],
) -> syn::Result<()> {
    let expected: Vec<(&str, bool)> = segments
        .iter()
        .filter_map(|segment| match segment {
            RouteSegment::Param { name } => Some((name.as_str(), false)),
            RouteSegment::Wildcard { name: Some(name) } => Some((name.as_str(), true)),
            RouteSegment::Static { .. } | RouteSegment::Wildcard { name: None } => None,
        })
        .collect();
    if fields.len() != expected.len() {
        return Err(Error::new_spanned(
            route_name,
            format!(
                "route `{route_name}` declares {} field(s), but its pattern needs {} named parameter field(s)",
                fields.len(),
                expected.len()
            ),
        ));
    }
    for (expected_name, is_wildcard) in expected {
        let Some(field) = fields.iter().find(|field| field.name == expected_name) else {
            return Err(Error::new_spanned(
                route_name,
                format!("route `{route_name}` needs a field named `{expected_name}`"),
            ));
        };
        if is_wildcard && !is_path_tail_type(&field.ty) {
            return Err(Error::new_spanned(
                &field.ty,
                "wildcard route fields must use `PathTail`",
            ));
        }
    }
    Ok(())
}

fn is_path_tail_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(last) = type_path.path.segments.last() else {
        return false;
    };
    last.ident == "PathTail" && matches!(last.arguments, syn::PathArguments::None)
}

fn parse_pattern(
    path: &LitStr,
) -> syn::Result<(String, Vec<RouteSegment>, Vec<PatternKeySegment>)> {
    let value = path.value();
    let pattern = normalize_pattern(&value, path.span())?;
    let body = pattern.strip_prefix('/').unwrap_or_default();
    if body.is_empty() {
        return Ok((pattern, Vec::new(), Vec::new()));
    }

    let raw_segments: Vec<&str> = body.split('/').collect();
    let mut parameter_names = HashSet::new();
    let mut segments = Vec::with_capacity(raw_segments.len());
    let mut key = Vec::with_capacity(raw_segments.len());
    for (index, raw) in raw_segments.iter().enumerate() {
        if let Some(name) = raw.strip_prefix(':') {
            validate_parameter_name(&pattern, name, path.span())?;
            if !parameter_names.insert(name.to_string()) {
                return Err(Error::new_spanned(
                    path,
                    format!("route pattern `{pattern}` repeats parameter `{name}`"),
                ));
            }
            segments.push(RouteSegment::Param {
                name: name.to_string(),
            });
            key.push(PatternKeySegment::Param);
        } else if let Some(name) = raw.strip_prefix('*') {
            if index + 1 != raw_segments.len() {
                return Err(Error::new_spanned(
                    path,
                    "wildcard route parameters must be the final segment",
                ));
            }
            let name = if name.is_empty() {
                None
            } else {
                validate_parameter_name(&pattern, name, path.span())?;
                if !parameter_names.insert(name.to_string()) {
                    return Err(Error::new_spanned(
                        path,
                        format!("route pattern `{pattern}` repeats parameter `{name}`"),
                    ));
                }
                Some(name.to_string())
            };
            segments.push(RouteSegment::Wildcard { name });
            key.push(PatternKeySegment::Wildcard);
        } else {
            let _ = decode_segment(raw, path.span())?;
            segments.push(RouteSegment::Static {
                raw: (*raw).to_string(),
            });
            key.push(PatternKeySegment::Static(decode_segment(raw, path.span())?));
        }
    }
    Ok((pattern, segments, key))
}

fn normalize_pattern(value: &str, span: proc_macro2::Span) -> syn::Result<String> {
    if value.is_empty() || value == "/" {
        return Ok(String::from("/"));
    }
    if value.contains(['?', '#']) {
        return Err(Error::new(
            span,
            "route patterns cannot contain query strings or fragments",
        ));
    }
    if !value.starts_with('/') {
        return Err(Error::new(span, "route patterns must start with `/`"));
    }
    if value.contains("//") {
        return Err(Error::new(
            span,
            "route patterns cannot contain empty path segments",
        ));
    }
    Ok(value.strip_suffix('/').unwrap_or(value).to_string())
}

fn validate_parameter_name(pattern: &str, name: &str, span: proc_macro2::Span) -> syn::Result<()> {
    let mut chars = name.chars();
    let valid_first = chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    let valid_rest = chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !valid_first || !valid_rest {
        return Err(Error::new(
            span,
            format!("invalid parameter name `{name}` in route pattern `{pattern}`"),
        ));
    }
    Ok(())
}

fn decode_segment(value: &str, span: proc_macro2::Span) -> syn::Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(Error::new(
                    span,
                    "route pattern contains invalid percent encoding",
                ));
            }
            let Some(high) = hex_value(bytes[index + 1]) else {
                return Err(Error::new(
                    span,
                    "route pattern contains invalid percent encoding",
                ));
            };
            let Some(low) = hex_value(bytes[index + 2]) else {
                return Err(Error::new(
                    span,
                    "route pattern contains invalid percent encoding",
                ));
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded)
        .map_err(|_| Error::new(span, "percent-decoded route segment is not valid UTF-8"))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn join_patterns(parent: &str, child: &str) -> String {
    if parent == "/" {
        child.to_string()
    } else if child == "/" {
        parent.to_string()
    } else {
        format!("{parent}{child}")
    }
}

fn generate_enum_definitions(tree: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let variants = tree.nodes.iter().map(|node| match node {
        RouteNode::Leaf(route) => {
            let variant = &route.name;
            let fields = route.fields.iter().map(|field| {
                let name = &field.name;
                let ty = &field.ty;
                quote! { #name: #ty }
            });
            if route.fields.is_empty() {
                quote! { #variant }
            } else {
                quote! { #variant { #(#fields),* } }
            }
        }
        RouteNode::Nested(nested) => {
            let variant = &nested.name;
            let enum_name = &nested.enum_name;
            quote! { #variant(#enum_name) }
        }
    });
    let nested = tree.nodes.iter().filter_map(|node| match node {
        RouteNode::Leaf(_) => None,
        RouteNode::Nested(nested) => Some(generate_enum_definitions(&nested.children)),
    });
    quote! {
        #(#nested)*
        #visibility enum #name {
            #(#variants),*
        }
    }
}

fn generate_enum_implementations(
    silex: &TokenStream,
    tree: &RouteTree,
    is_root: bool,
    root: &RouteTree,
) -> TokenStream {
    let path_impl = generate_path_impl(silex, tree);
    let nested_impls = tree.nodes.iter().filter_map(|node| match node {
        RouteNode::Leaf(_) => None,
        RouteNode::Nested(nested) => Some(generate_enum_implementations(
            silex,
            &nested.children,
            false,
            root,
        )),
    });
    let root_table = if is_root {
        generate_table_impl(silex, tree, root)
    } else {
        TokenStream::new()
    };
    let root_match = if is_root {
        generate_match_path_impl(silex, tree)
    } else {
        TokenStream::new()
    };
    quote! {
        #path_impl
        #root_table
        #root_match
        #(#nested_impls)*
    }
}

fn generate_path_impl(silex: &TokenStream, tree: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let arms = tree.nodes.iter().map(|node| match node {
        RouteNode::Leaf(route) => {
            let variant = &route.name;
            let bindings = route.fields.iter().map(|field| &field.name);
            if route
                .segments
                .iter()
                .any(|segment| matches!(segment, RouteSegment::Wildcard { name: None }))
            {
                return quote! {
                    Self::#variant => Err(
                        #silex::router::PathParamError::InvalidValue(
                            "anonymous wildcard routes cannot be encoded".to_string(),
                        )
                    )
                };
            }
            let pushes = route.segments.iter().map(|segment| match segment {
                RouteSegment::Static { raw } => quote! {
                    __silex_path.push_static(#raw)?;
                },
                RouteSegment::Param { name } => {
                    let field = route_field(route, name);
                    let ident = &field.name;
                    quote! { __silex_path.push_param(#ident)?; }
                }
                RouteSegment::Wildcard { name: Some(name) } => {
                    let field = route_field(route, name);
                    let ident = &field.name;
                    quote! { __silex_path.push_param(#ident)?; }
                }
                RouteSegment::Wildcard { name: None } => quote! {
                    return Err(#silex::router::PathParamError::InvalidValue(
                        "anonymous wildcard routes cannot be encoded".to_string(),
                    ));
                },
            });
            let pattern = if route.fields.is_empty() {
                quote! { Self::#variant }
            } else {
                quote! { Self::#variant { #(#bindings),* } }
            };
            quote! {
                #pattern => {
                    let mut __silex_path = #silex::router::RoutePathBuilder::new();
                    #(#pushes)*
                    __silex_path.finish()
                }
            }
        }
        RouteNode::Nested(nested) => {
            let variant = &nested.name;
            let prefix = &nested.prefix;
            quote! {
                Self::#variant(__silex_child) => {
                    let __silex_child_path = __silex_child.path()?;
                    #silex::router::join_route_paths(#prefix, __silex_child_path.as_str())
                        .map_err(#silex::router::PathParamError::from)
                }
            }
        }
    });
    quote! {
        impl #name {
            #visibility fn path(&self) -> ::std::result::Result<
                #silex::router::RoutePath,
                #silex::router::PathParamError,
            > {
                match self {
                    #(#arms),*
                }
            }
        }
    }
}

fn route_field<'a>(route: &'a RouteLeaf, name: &str) -> &'a RouteField {
    route
        .fields
        .iter()
        .find(|field| field.name == name)
        .expect("validated route field")
}

fn generate_match_path_impl(silex: &TokenStream, tree: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let leaves = collect_leaves(tree);
    let patterns = leaves.iter().map(|leaf| {
        let pattern = &leaf.full_pattern;
        quote! { #pattern }
    });
    let arms = leaves.iter().enumerate().map(|(route_id, leaf)| {
        let constructor = route_constructor(tree, leaf, quote! { __silex_match });
        quote! { #route_id => #constructor }
    });
    quote! {
        impl #name {
            #visibility fn match_path(
                path: &str,
            ) -> ::std::result::Result<::std::option::Option<Self>, #silex::router::PathError> {
                let __silex_matcher = #silex::router::RouteMatcher::from_patterns([
                    #(#patterns),*
                ])
                .map_err(|error| #silex::router::PathError::InvalidPath(error.to_string()))?;
                let __silex_result = __silex_matcher.matches(path)?.into_iter().find_map(|__silex_match| {
                    match __silex_match.route_id() {
                        #(#arms,)*
                        _ => None,
                    }
                });
                Ok(__silex_result)
            }
        }
    }
}

fn collect_leaves(tree: &RouteTree) -> Vec<&RouteLeaf> {
    let mut leaves = Vec::new();
    collect_leaves_into(tree, &mut leaves);
    leaves
}

fn collect_leaves_into<'a>(tree: &'a RouteTree, leaves: &mut Vec<&'a RouteLeaf>) {
    for node in &tree.nodes {
        match node {
            RouteNode::Leaf(leaf) => leaves.push(leaf),
            RouteNode::Nested(nested) => collect_leaves_into(&nested.children, leaves),
        }
    }
}

fn route_constructor(tree: &RouteTree, leaf: &RouteLeaf, matched: TokenStream) -> TokenStream {
    let mut path: Vec<(Option<Ident>, Ident)> = Vec::new();
    if let Some(constructor) = find_constructor(tree, None, leaf, &matched, &mut path) {
        constructor
    } else {
        quote! { None }
    }
}

fn find_constructor(
    tree: &RouteTree,
    current_enum: Option<&Ident>,
    target: &RouteLeaf,
    matched: &TokenStream,
    nested_variants: &mut Vec<(Option<Ident>, Ident)>,
) -> Option<TokenStream> {
    for node in &tree.nodes {
        match node {
            RouteNode::Leaf(leaf) if std::ptr::eq(leaf, target) => {
                let variant = &leaf.name;
                let fields = leaf.fields.iter().map(|field| {
                    let name = &field.name;
                    let ty = &field.ty;
                    let parameter_name = LitStr::new(&name.to_string(), Span::call_site());
                    quote! { #name: #matched.parse::<#ty>(#parameter_name).ok()? }
                });
                let constructor = if let Some(current_enum) = current_enum {
                    if leaf.fields.is_empty() {
                        quote! { #current_enum::#variant }
                    } else {
                        quote! { #current_enum::#variant { #(#fields),* } }
                    }
                } else if leaf.fields.is_empty() {
                    quote! { Self::#variant }
                } else {
                    quote! { Self::#variant { #(#fields),* } }
                };
                let mut constructor = constructor;
                for (parent_enum, nested_variant) in nested_variants.iter().rev() {
                    if let Some(parent_enum) = parent_enum {
                        constructor = quote! { #parent_enum::#nested_variant(#constructor) };
                    } else {
                        constructor = quote! { Self::#nested_variant(#constructor) };
                    }
                }
                return Some(quote! { Some(#constructor) });
            }
            RouteNode::Leaf(_) => {}
            RouteNode::Nested(nested) => {
                nested_variants.push((current_enum.cloned(), nested.name.clone()));
                if let Some(constructor) = find_constructor(
                    &nested.children,
                    Some(&nested.enum_name),
                    target,
                    matched,
                    nested_variants,
                ) {
                    return Some(constructor);
                }
                nested_variants.pop();
            }
        }
    }
    None
}

fn generate_table_impl(silex: &TokenStream, tree: &RouteTree, root: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let table = generate_table_expression(silex, tree, quote! { __silex_render }, root);
    quote! {
        impl #name {
            #visibility fn table<'scope, F>(
                render: F,
            ) -> ::std::result::Result<#silex::router::RouteTable<'scope>, #silex::router::RoutePatternError>
            where
                F: Fn(Self, #silex::router::RouterContext<'scope>)
                    -> #silex::dom::view::AnyView<'scope>
                    + 'scope,
            {
                let __silex_render = ::std::rc::Rc::new(render);
                Ok(#table)
            }
        }
    }
}

fn generate_table_expression(
    silex: &TokenStream,
    tree: &RouteTree,
    render: TokenStream,
    root: &RouteTree,
) -> TokenStream {
    let entries = tree.nodes.iter().filter_map(|node| match node {
        RouteNode::Leaf(leaf) => Some(generate_entry(silex, root, leaf, &render)),
        RouteNode::Nested(_) => None,
    });
    let mut expression = quote! {
        #silex::router::RouteTable::from_entries(::std::vec![#(#entries?),*])?
    };
    for node in &tree.nodes {
        let RouteNode::Nested(nested) = node else {
            continue;
        };
        let prefix = &nested.prefix;
        let child = generate_table_expression(silex, &nested.children, render.clone(), root);
        let layout = &nested.layout;
        expression = quote! {
            #expression.nest(
                #prefix,
                #child,
                move |__silex_context, __silex_outlet| {
                    #silex::dom::view::View::into_any(
                        (#layout)(__silex_context, __silex_outlet)
                    )
                },
            )?
        };
    }
    expression
}

fn generate_entry(
    silex: &TokenStream,
    tree: &RouteTree,
    leaf: &RouteLeaf,
    render: &TokenStream,
) -> TokenStream {
    let pattern = &leaf.pattern;
    let constructor = route_constructor(tree, leaf, quote! { __silex_match });
    quote! {
        #silex::router::RouteEntry::new(
            #pattern,
            {
                let __silex_render = ::std::rc::Rc::clone(&#render);
                move |__silex_match, __silex_context| {
                    let __silex_route = #constructor?;
                    Some(__silex_render(__silex_route, __silex_context))
                }
            },
        )
    }
}
