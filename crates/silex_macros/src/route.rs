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
        layout: Box<ExprClosure>,
        child_type: Box<Type>,
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

        let fields_or_nested = if input.peek(syn::token::Paren) {
            let child_content;
            syn::parenthesized!(child_content in input);
            let child_type = child_content.parse()?;
            if !child_content.is_empty() {
                return Err(child_content.error("unexpected tokens in nested route child type"));
            }
            let content;
            syn::braced!(content in input);
            let nested = RouteNodeKindInput::Nested {
                prefix: parse_nested_prefix(&content)?,
                layout: Box::new(parse_nested_layout(&content)?),
                child_type: Box::new(child_type),
            };
            if !content.is_empty() {
                return Err(content.error(
                    "nested route body only accepts `prefix` and `layout`; declare children in a separate `router!` enum",
                ));
            }
            nested
        } else if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            RouteNodeKindInput::Leaf {
                fields: parse_named_fields(&content)?,
                path: {
                    input.parse::<Token![=>]>()?;
                    input.parse()?
                },
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
    segments: Vec<RouteSegment>,
}

struct NestedRoute {
    name: Ident,
    prefix: String,
    layout: Box<ExprClosure>,
    child_type: Box<Type>,
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

pub fn router_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let input: RouterInput = syn::parse2(input)?;
    let tree = compile_tree(
        input.name,
        input.visibility,
        input.nodes.into_iter().collect(),
    )?;
    let silex = crate::crate_path::silex();
    let definitions = generate_enum_definitions(&tree);
    let implementations = generate_enum_implementations(&silex, &tree);
    Ok(quote! {
        #definitions
        #implementations
    })
}

fn compile_tree(
    name: Ident,
    visibility: Visibility,
    definitions: Vec<RouteNodeInput>,
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
                if !local_patterns.insert(key) {
                    return Err(Error::new_spanned(
                        path,
                        "route patterns in one enum must be unique by shape",
                    ));
                }
                nodes.push(RouteNode::Leaf(RouteLeaf {
                    name: definition.name,
                    fields,
                    pattern,
                    segments,
                }));
            }
            RouteNodeKindInput::Nested {
                prefix,
                layout,
                child_type,
            } => {
                matcher_type_for(&child_type)?;
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
                nodes.push(RouteNode::Nested(NestedRoute {
                    name: definition.name,
                    prefix,
                    layout,
                    child_type,
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
            let child_type = &nested.child_type;
            quote! { #variant(#child_type) }
        }
    });
    quote! {
        #visibility enum #name {
            #(#variants),*
        }
    }
}

fn generate_enum_implementations(silex: &TokenStream, tree: &RouteTree) -> TokenStream {
    let path_impl = generate_path_impl(silex, tree);
    let table = generate_table_impl(silex, tree);
    let matcher = generate_typed_matcher_impl(silex, tree);
    quote! {
        #path_impl
        #table
        #matcher
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
                        #silex::router::PathParamError::recoverable(
                            #silex::router::PathParamErrorKind::InvalidValue(
                            "anonymous wildcard routes cannot be encoded".to_string(),
                            ),
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
                    return Err(#silex::router::PathParamError::recoverable(
                        #silex::router::PathParamErrorKind::InvalidValue(
                            "anonymous wildcard routes cannot be encoded".to_string(),
                        ),
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

fn generate_typed_matcher_impl(silex: &TokenStream, tree: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let matcher_name = matcher_ident(name);
    let patterns = tree.nodes.iter().map(|node| {
        let pattern = match node {
            RouteNode::Leaf(leaf) => leaf.pattern.clone(),
            RouteNode::Nested(nested) => nested_pattern(&nested.prefix),
        };
        let pattern = LitStr::new(&pattern, Span::call_site());
        quote! { #pattern }
    });
    let child_fields = tree
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(route_id, node)| {
            let RouteNode::Nested(nested) = node else {
                return None;
            };
            let field = child_matcher_field(route_id);
            let matcher_type = matcher_type_for(&nested.child_type).expect("validated child type");
            Some(quote! { #field: #matcher_type })
        });
    let child_initializers = tree
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(route_id, node)| {
            let RouteNode::Nested(nested) = node else {
                return None;
            };
            let field = child_matcher_field(route_id);
            let child_type = &nested.child_type;
            Some(quote! { #field: #child_type::compile()? })
        });
    let arms = tree
        .nodes
        .iter()
        .enumerate()
        .map(|(route_id, node)| match node {
            RouteNode::Leaf(leaf) => {
                let constructor = route_constructor(leaf, quote! { __silex_match }, name);
                quote! { #route_id => #constructor }
            }
            RouteNode::Nested(nested) => {
                let variant = &nested.name;
                let prefix = &nested.prefix;
                let child_field = child_matcher_field(route_id);
                quote! {
                    #route_id => {
                        match #silex::router::strip_route_prefix(
                            #prefix,
                            __silex_match.path(),
                        ) {
                            Some(__silex_relative_path) => {
                                match self.#child_field.match_path(&__silex_relative_path)? {
                                    Some(__silex_child) => Some(#name::#variant(__silex_child)),
                                    None => None,
                                }
                            }
                            None => None,
                        }
                    }
                }
            }
        });
    quote! {
        #visibility struct #matcher_name {
            __silex_matcher: #silex::router::RouteMatcher,
            #(#child_fields),*
        }

        impl #name {
            #visibility fn patterns() -> &'static [&'static str] {
                &[#(#patterns),*]
            }

            #visibility fn compile() -> ::std::result::Result<
                #matcher_name,
                #silex::router::RoutePatternError,
            > {
                Ok(#matcher_name {
                    __silex_matcher: #silex::router::RouteMatcher::from_patterns(
                        Self::patterns().iter().copied()
                    )?,
                    #(#child_initializers),*
                })
            }
        }

        impl #matcher_name {
            #visibility fn match_path(
                &self,
                path: &str,
            ) -> ::std::result::Result<::std::option::Option<#name>, #silex::router::PathError> {
                for __silex_match in self.__silex_matcher.matches(path)? {
                    let __silex_result = match __silex_match.route_id() {
                        #(#arms,)*
                        _ => None,
                    };
                    if __silex_result.is_some() {
                        return Ok(__silex_result);
                    }
                }
                Ok(None)
            }
        }
    }
}

fn matcher_ident(name: &Ident) -> Ident {
    format_ident!("{name}Matcher")
}

fn child_matcher_field(route_id: usize) -> Ident {
    format_ident!("__silex_child_matcher_{route_id}")
}

fn matcher_type_for(child_type: &Type) -> syn::Result<Type> {
    let Type::Path(type_path) = child_type else {
        return Err(Error::new_spanned(
            child_type,
            "nested route child type must be a route enum path",
        ));
    };
    let mut matcher_path = type_path.clone();
    let Some(last) = matcher_path.path.segments.last_mut() else {
        return Err(Error::new_spanned(
            child_type,
            "nested route child type must be a route enum path",
        ));
    };
    if !matches!(last.arguments, syn::PathArguments::None) {
        return Err(Error::new_spanned(
            child_type,
            "nested route child type cannot have generic arguments",
        ));
    }
    last.ident = matcher_ident(&last.ident);
    Ok(Type::Path(matcher_path))
}

fn nested_pattern(prefix: &str) -> String {
    if prefix == "/" {
        String::from("/*")
    } else {
        format!("{prefix}/*")
    }
}

fn route_constructor(leaf: &RouteLeaf, matched: TokenStream, route_type: &Ident) -> TokenStream {
    let variant = &leaf.name;
    let fields = leaf.fields.iter().map(|field| {
        let name = &field.name;
        let ty = &field.ty;
        let parameter_name = LitStr::new(&name.to_string(), Span::call_site());
        quote! { #name: #matched.parse::<#ty>(#parameter_name).ok()? }
    });
    let constructor = if leaf.fields.is_empty() {
        quote! { #route_type::#variant }
    } else {
        quote! { #route_type::#variant { #(#fields),* } }
    };
    quote! {
        (|| {
            Some(#constructor)
        })()
    }
}

fn generate_table_impl(silex: &TokenStream, tree: &RouteTree) -> TokenStream {
    let visibility = &tree.visibility;
    let name = &tree.name;
    let table = generate_table_expression(silex, tree, quote! { __silex_render });
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
                #table
            }
        }
    }
}

fn generate_table_expression(
    silex: &TokenStream,
    tree: &RouteTree,
    render: TokenStream,
) -> TokenStream {
    let entries = tree.nodes.iter().filter_map(|node| match node {
        RouteNode::Leaf(leaf) => Some(generate_entry(silex, leaf, &render, &tree.name)),
        RouteNode::Nested(_) => None,
    });
    let mut statements = quote! {
        let mut __silex_table = #silex::router::RouteTable::from_entries(
            ::std::vec![#(#entries?),*]
        )?;
    };
    for node in &tree.nodes {
        let RouteNode::Nested(nested) = node else {
            continue;
        };
        let prefix = &nested.prefix;
        let child_type = &nested.child_type;
        let variant = &nested.name;
        let child = quote! {
            #child_type::table({
                let __silex_render = ::std::rc::Rc::clone(&#render);
                move |__silex_child, __silex_ctx| {
                    __silex_render(Self::#variant(__silex_child), __silex_ctx)
                }
            })?
        };
        let layout = &nested.layout;
        statements = quote! {
            #statements
            __silex_table = __silex_table.nest(
                #prefix,
                #child,
                move |__silex_ctx, __silex_outlet| {
                    #silex::dom::view::View::into_any(
                        (#layout)(__silex_ctx, __silex_outlet)
                    )
                },
            )?;
        };
    }
    quote! {
        #statements
        Ok(__silex_table)
    }
}

fn generate_entry(
    silex: &TokenStream,
    leaf: &RouteLeaf,
    render: &TokenStream,
    route_type: &Ident,
) -> TokenStream {
    let pattern = &leaf.pattern;
    let constructor = route_constructor(leaf, quote! { __silex_match }, route_type);
    quote! {
        #silex::router::RouteEntry::new(
            #pattern,
            {
                let __silex_render = ::std::rc::Rc::clone(&#render);
                move |__silex_match, __silex_ctx| {
                    let __silex_route = #constructor?;
                    Some(__silex_render(__silex_route, __silex_ctx))
                }
            },
        )
    }
}
