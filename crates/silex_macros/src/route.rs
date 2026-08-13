use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::parse::{Parse, ParseStream};
use syn::{Error, ExprClosure, Ident, LitStr, Pat, PathArguments, Token, Type};

struct RoutesInput {
    catalog: Ident,
    routes: syn::punctuated::Punctuated<RouteNodeInput, Token![,]>,
}

impl Parse for RoutesInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let catalog = input.parse()?;
        let content;
        syn::braced!(content in input);
        let routes = content.parse_terminated(RouteNodeInput::parse, Token![,])?;
        Ok(Self { catalog, routes })
    }
}

enum RouteNodeInput {
    Leaf(RouteMacroDef),
    Nest(NestedRouteMacroDef),
}

impl Parse for RouteNodeInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        if keyword == "nest" {
            let name: Ident = input.parse()?;
            let prefix: LitStr = input.parse()?;
            let guards_before = parse_guard_metadata(input)?;
            input.parse::<Token![=>]>()?;
            let layout = input.parse()?;
            let content;
            syn::braced!(content in input);
            let children = content
                .parse_terminated(RouteNodeInput::parse, Token![,])?
                .into_iter()
                .collect();
            let mut guards_after = parse_guard_metadata(input)?;
            if guards_after.is_none() && comma_precedes_guard_metadata(input) {
                input.parse::<Token![,]>()?;
                guards_after = parse_guard_metadata(input)?;
            }

            Ok(Self::Nest(NestedRouteMacroDef {
                name: name.clone(),
                prefix,
                layout,
                children,
                guards: merge_guard_metadata(&name, guards_before, guards_after)?,
            }))
        } else {
            let path = input.parse()?;
            let guards_before = parse_guard_metadata(input)?;
            input.parse::<Token![=>]>()?;
            let handler = input.parse()?;
            let mut guards_after = parse_guard_metadata(input)?;
            if guards_after.is_none() && comma_precedes_guard_metadata(input) {
                input.parse::<Token![,]>()?;
                guards_after = parse_guard_metadata(input)?;
            }

            Ok(Self::Leaf(RouteMacroDef {
                name: keyword.clone(),
                path,
                handler,
                guards: merge_guard_metadata(&keyword, guards_before, guards_after)?,
            }))
        }
    }
}

struct RouteMacroDef {
    name: Ident,
    path: LitStr,
    handler: ExprClosure,
    guards: Vec<syn::Path>,
}

struct NestedRouteMacroDef {
    name: Ident,
    prefix: LitStr,
    layout: ExprClosure,
    children: Vec<RouteNodeInput>,
    guards: Vec<syn::Path>,
}

fn parse_guard_metadata(input: ParseStream<'_>) -> syn::Result<Option<Vec<syn::Path>>> {
    if !input.peek(Ident) {
        return Ok(None);
    }

    let keyword: Ident = input.parse()?;
    if keyword != "guards" {
        return Err(Error::new_spanned(
            keyword,
            "expected `guards = [Guard, ...]` route metadata",
        ));
    }
    input.parse::<Token![=]>()?;
    let content;
    syn::bracketed!(content in input);
    let guards = content
        .parse_terminated(syn::Path::parse, Token![,])?
        .into_iter()
        .collect();
    Ok(Some(guards))
}

fn merge_guard_metadata(
    name: &Ident,
    before: Option<Vec<syn::Path>>,
    after: Option<Vec<syn::Path>>,
) -> syn::Result<Vec<syn::Path>> {
    if before.is_some() && after.is_some() {
        return Err(Error::new_spanned(
            name,
            "route guards may be declared only once",
        ));
    }
    Ok(before.or(after).unwrap_or_default())
}

fn comma_precedes_guard_metadata(input: ParseStream<'_>) -> bool {
    let fork = input.fork();
    if fork.parse::<Token![,]>().is_err() {
        return false;
    }
    let Ok(keyword) = fork.parse::<Ident>() else {
        return false;
    };
    keyword == "guards" && fork.peek(Token![=])
}

#[derive(Clone)]
enum MacroSegment {
    Static { raw: String },
    Param { name: String },
    Wildcard { name: Option<String> },
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum MacroPatternKey {
    Static(String),
    Param,
    Wildcard,
}

struct HandlerPathParam {
    name: String,
    ident: Ident,
    ty: Type,
}

struct MacroRoute {
    name: Ident,
    pattern: String,
    full_pattern: String,
    key: Vec<MacroPatternKey>,
    full_key: Vec<MacroPatternKey>,
    segments: Vec<MacroSegment>,
    destination_segments: Vec<MacroSegment>,
    params: Vec<HandlerPathParam>,
    handler: ExprClosure,
    guards: Vec<syn::Path>,
    handler_name: Ident,
    entry_name: Ident,
}

struct MacroNest {
    name: Ident,
    prefix: String,
    catalog: Ident,
    value_name: Ident,
    layout: ExprClosure,
    layout_name: Ident,
    guards: Vec<syn::Path>,
    children: Vec<MacroNode>,
}

enum MacroNode {
    Leaf(MacroRoute),
    Nest(MacroNest),
}

struct CompileCounters {
    route: usize,
    nest: usize,
}

fn compile_nodes(
    definitions: Vec<RouteNodeInput>,
    parent_prefix: &str,
    full_patterns: &mut HashSet<Vec<MacroPatternKey>>,
    counters: &mut CompileCounters,
) -> syn::Result<Vec<MacroNode>> {
    let mut names = HashSet::new();
    let mut table_patterns = HashSet::new();
    let mut nodes = Vec::with_capacity(definitions.len());

    for definition in definitions {
        let name = match &definition {
            RouteNodeInput::Leaf(route) => &route.name,
            RouteNodeInput::Nest(nest) => &nest.name,
        };
        if matches!(name.to_string().as_str(), "table" | "at" | "prefix") {
            return Err(Error::new_spanned(
                name,
                "route name is reserved by the generated catalog",
            ));
        }
        if !names.insert(name.to_string()) {
            return Err(Error::new_spanned(
                name,
                "route and nested route names in one catalog must be unique",
            ));
        }

        match definition {
            RouteNodeInput::Leaf(definition) => {
                let route = compile_macro_route(definition, parent_prefix, counters)?;
                if !table_patterns.insert(route.key.clone()) {
                    return Err(Error::new_spanned(
                        &route.name,
                        format!(
                            "route pattern `{}` conflicts with another normalized route pattern",
                            route.pattern
                        ),
                    ));
                }
                if !full_patterns.insert(route.full_key.clone()) {
                    return Err(Error::new_spanned(
                        &route.name,
                        format!(
                            "route pattern `{}` conflicts with another normalized complete route pattern",
                            route.full_pattern
                        ),
                    ));
                }
                nodes.push(MacroNode::Leaf(route));
            }
            RouteNodeInput::Nest(definition) => {
                let (prefix, prefix_segments, _) = parse_macro_pattern(&definition.prefix)?;
                if prefix_segments
                    .iter()
                    .any(|segment| !matches!(segment, MacroSegment::Static { .. }))
                {
                    return Err(Error::new_spanned(
                        &definition.prefix,
                        "nested route prefixes must contain only static segments",
                    ));
                }
                validate_nested_layout(&definition.layout, &definition.name)?;

                let synthetic_pattern = if prefix == "/" {
                    String::from("/*")
                } else {
                    format!("{prefix}/*")
                };
                let synthetic_literal = LitStr::new(&synthetic_pattern, definition.prefix.span());
                let (_, _, synthetic_key) = parse_macro_pattern(&synthetic_literal)?;
                if !table_patterns.insert(synthetic_key) {
                    return Err(Error::new_spanned(
                        &definition.name,
                        format!(
                            "nested route prefix `{prefix}` conflicts with another route pattern"
                        ),
                    ));
                }

                let full_prefix = join_route_patterns(parent_prefix, &prefix);
                let children =
                    compile_nodes(definition.children, &full_prefix, full_patterns, counters)?;
                let nest_index = counters.nest;
                counters.nest += 1;
                nodes.push(MacroNode::Nest(MacroNest {
                    name: definition.name,
                    prefix,
                    catalog: format_ident!("__silex_routes_nested_catalog_{nest_index}"),
                    value_name: format_ident!("__silex_routes_nested_value_{nest_index}"),
                    layout: definition.layout,
                    layout_name: format_ident!("__silex_routes_layout_{nest_index}"),
                    guards: definition.guards,
                    children,
                }));
            }
        }
    }

    Ok(nodes)
}

fn compile_macro_route(
    definition: RouteMacroDef,
    parent_prefix: &str,
    counters: &mut CompileCounters,
) -> syn::Result<MacroRoute> {
    let (pattern, segments, key) = parse_macro_pattern(&definition.path)?;
    let params = validate_handler(&definition.handler, &definition.name, &segments)?;
    let full_pattern = join_route_patterns(parent_prefix, &pattern);
    let full_literal = LitStr::new(&full_pattern, definition.path.span());
    let (_, destination_segments, full_key) = parse_macro_pattern(&full_literal)?;

    let route_index = counters.route;
    counters.route += 1;
    Ok(MacroRoute {
        name: definition.name,
        pattern,
        full_pattern,
        key,
        full_key,
        segments,
        destination_segments,
        params,
        handler: definition.handler,
        guards: definition.guards,
        handler_name: format_ident!("__silex_routes_handler_{route_index}"),
        entry_name: format_ident!("__silex_routes_entry_{route_index}"),
    })
}

fn join_route_patterns(parent: &str, child: &str) -> String {
    if parent == "/" {
        child.to_string()
    } else if child == "/" {
        parent.to_string()
    } else {
        format!("{parent}{child}")
    }
}

fn parse_macro_pattern(
    path: &LitStr,
) -> syn::Result<(String, Vec<MacroSegment>, Vec<MacroPatternKey>)> {
    let value = path.value();
    let pattern = normalize_macro_pattern(&value, path.span())?;
    let body = pattern.strip_prefix('/').unwrap_or_default();
    if body.is_empty() {
        return Ok((pattern, Vec::new(), Vec::new()));
    }

    let raw_segments: Vec<&str> = body.split('/').collect();
    let mut names = HashSet::new();
    let mut segments = Vec::with_capacity(raw_segments.len());
    let mut key = Vec::with_capacity(raw_segments.len());

    for (index, raw_segment) in raw_segments.iter().enumerate() {
        if let Some(name) = raw_segment.strip_prefix(':') {
            validate_macro_parameter_name(&pattern, name, path.span())?;
            if !names.insert(name.to_string()) {
                return Err(Error::new_spanned(
                    path,
                    format!("route pattern `{pattern}` repeats parameter `{name}`"),
                ));
            }
            segments.push(MacroSegment::Param {
                name: name.to_string(),
            });
            key.push(MacroPatternKey::Param);
        } else if let Some(name) = raw_segment.strip_prefix('*') {
            if index + 1 != raw_segments.len() {
                return Err(Error::new_spanned(
                    path,
                    "wildcard route parameters must be the final segment",
                ));
            }
            let name = if name.is_empty() {
                None
            } else {
                validate_macro_parameter_name(&pattern, name, path.span())?;
                if !names.insert(name.to_string()) {
                    return Err(Error::new_spanned(
                        path,
                        format!("route pattern `{pattern}` repeats parameter `{name}`"),
                    ));
                }
                Some(name.to_string())
            };
            segments.push(MacroSegment::Wildcard { name });
            key.push(MacroPatternKey::Wildcard);
        } else {
            let decoded = decode_macro_segment(raw_segment, path.span())?;
            segments.push(MacroSegment::Static {
                raw: (*raw_segment).to_string(),
            });
            key.push(MacroPatternKey::Static(decoded));
        }
    }

    Ok((pattern, segments, key))
}

fn normalize_macro_pattern(value: &str, span: proc_macro2::Span) -> syn::Result<String> {
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

    let normalized = value.strip_suffix('/').unwrap_or(value);
    if normalized.is_empty() {
        Ok(String::from("/"))
    } else {
        Ok(normalized.to_string())
    }
}

fn validate_macro_parameter_name(
    pattern: &str,
    name: &str,
    span: proc_macro2::Span,
) -> syn::Result<()> {
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

fn decode_macro_segment(value: &str, span: proc_macro2::Span) -> syn::Result<String> {
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
            let high = macro_hex_value(bytes[index + 1]);
            let low = macro_hex_value(bytes[index + 2]);
            let (Some(high), Some(low)) = (high, low) else {
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

fn macro_hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn validate_nested_layout(layout: &ExprClosure, name: &Ident) -> syn::Result<()> {
    if layout.inputs.len() != 2 {
        return Err(Error::new_spanned(
            layout,
            format!("nested route `{name}` layout must accept RouterContext and outlet"),
        ));
    }
    parse_handler_param(&layout.inputs[0])?;
    parse_handler_param(&layout.inputs[1])?;
    Ok(())
}

fn validate_handler(
    handler: &ExprClosure,
    route_name: &Ident,
    segments: &[MacroSegment],
) -> syn::Result<Vec<HandlerPathParam>> {
    let inputs = &handler.inputs;
    let Some(first) = inputs.first() else {
        return Err(Error::new_spanned(
            handler,
            format!("route `{route_name}` handler must accept RouterContext first"),
        ));
    };
    let _ = parse_handler_param(first)?;

    let expected: Vec<(&str, bool)> = segments
        .iter()
        .filter_map(|segment| match segment {
            MacroSegment::Param { name } => Some((name.as_str(), false)),
            MacroSegment::Wildcard { name: Some(name) } => Some((name.as_str(), true)),
            MacroSegment::Static { .. } | MacroSegment::Wildcard { name: None } => None,
        })
        .collect();

    if inputs.len() != expected.len() + 1 {
        return Err(Error::new_spanned(
            handler,
            format!(
                "route `{route_name}` handler must accept RouterContext plus {} path parameter(s)",
                expected.len()
            ),
        ));
    }

    let mut params = Vec::with_capacity(expected.len());
    for (input, (expected_name, is_wildcard)) in inputs.iter().skip(1).zip(expected) {
        let (ident, ty) = parse_handler_param(input)?;
        if ident != expected_name {
            return Err(Error::new_spanned(
                input,
                format!("route `{route_name}` expects handler parameter `{expected_name}` here"),
            ));
        }
        let Some(ty) = ty else {
            return Err(Error::new_spanned(
                input,
                format!("route parameter `{expected_name}` needs an explicit type"),
            ));
        };
        if matches!(ty, Type::Infer(_)) {
            return Err(Error::new_spanned(
                &ty,
                format!("route parameter `{expected_name}` needs an explicit type"),
            ));
        }
        if is_wildcard && !is_path_tail_type(&ty) {
            return Err(Error::new_spanned(
                &ty,
                "wildcard route parameters must use `PathTail`",
            ));
        }
        params.push(HandlerPathParam {
            name: expected_name.to_string(),
            ident,
            ty,
        });
    }
    Ok(params)
}

fn parse_handler_param(pattern: &Pat) -> syn::Result<(Ident, Option<Type>)> {
    match pattern {
        Pat::Ident(identifier) => {
            if identifier.by_ref.is_some() || identifier.subpat.is_some() {
                return Err(Error::new_spanned(
                    pattern,
                    "route handler parameters must be simple identifiers",
                ));
            }
            Ok((identifier.ident.clone(), None))
        }
        Pat::Type(typed) => {
            let (ident, nested_type) = parse_handler_param(&typed.pat)?;
            if nested_type.is_some() {
                return Err(Error::new_spanned(
                    pattern,
                    "route handler parameters must be simple identifiers",
                ));
            }
            Ok((ident, Some((*typed.ty).clone())))
        }
        _ => Err(Error::new_spanned(
            pattern,
            "route handler parameters must be simple identifiers",
        )),
    }
}

fn is_path_tail_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(last) = type_path.path.segments.last() else {
        return false;
    };
    last.ident == "PathTail" && matches!(last.arguments, PathArguments::None)
}

pub fn routes_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let input: RoutesInput = syn::parse2(input)?;
    let catalog = input.catalog;
    let mut counters = CompileCounters { route: 0, nest: 0 };
    let nodes = compile_nodes(
        input.routes.into_iter().collect(),
        "/",
        &mut HashSet::new(),
        &mut counters,
    )?;

    let silex = crate::crate_path::silex();
    let scope = syn::Lifetime::new("'__silex_routes_scope", proc_macro2::Span::call_site());
    let root_value = format_ident!("__silex_routes_root_value");

    let mut definitions = Vec::new();
    generate_catalog_definitions(&silex, &scope, &catalog, &nodes, &mut definitions, true);

    let handler_bindings = generate_handler_bindings(&silex, &nodes);
    let layout_bindings = generate_layout_bindings(&nodes);
    let entry_bindings = generate_entry_bindings(&silex, &nodes)?;
    let mut catalog_bindings = Vec::new();
    generate_catalog_binding(&silex, &catalog, &nodes, &root_value, &mut catalog_bindings);

    Ok(quote! {{
        #(#definitions)*
        #(#handler_bindings)*
        #(#layout_bindings)*
        #(#entry_bindings)*
        #(#catalog_bindings)*
        #root_value
    }})
}

fn generate_handler_bindings(silex: &TokenStream, nodes: &[MacroNode]) -> Vec<TokenStream> {
    let mut bindings = Vec::new();
    for node in nodes {
        match node {
            MacroNode::Leaf(route) => {
                let handler_name = &route.handler_name;
                let handler = &route.handler;
                let infer_name = format_ident!("{}_infer", handler_name);
                let scope = syn::Lifetime::new(
                    &format!("'{}_scope", infer_name),
                    proc_macro2::Span::call_site(),
                );
                let parameter_types = route.params.iter().map(|param| {
                    let ty = &param.ty;
                    quote! { #ty }
                });
                bindings.push(quote! {
                    fn #infer_name<#scope, F, V>(handler: F) -> F
                    where
                        F: Fn(
                            #silex::router::RouterContext<#scope>,
                            #(#parameter_types),*
                        ) -> V,
                    {
                        handler
                    }

                    let #handler_name = #infer_name(#handler);
                });
            }
            MacroNode::Nest(nest) => {
                bindings.extend(generate_handler_bindings(silex, &nest.children));
            }
        }
    }
    bindings
}

fn generate_layout_bindings(nodes: &[MacroNode]) -> Vec<TokenStream> {
    let mut bindings = Vec::new();
    for node in nodes {
        match node {
            MacroNode::Leaf(_) => {}
            MacroNode::Nest(nest) => {
                let layout_name = &nest.layout_name;
                let layout = &nest.layout;
                bindings.push(quote! {
                    let #layout_name = #layout;
                });
                bindings.extend(generate_layout_bindings(&nest.children));
            }
        }
    }
    bindings
}

fn generate_entry_bindings(
    silex: &TokenStream,
    nodes: &[MacroNode],
) -> syn::Result<Vec<TokenStream>> {
    let mut bindings = Vec::new();
    for node in nodes {
        match node {
            MacroNode::Leaf(route) => bindings.push(generate_entry_binding(silex, route)),
            MacroNode::Nest(nest) => {
                bindings.extend(generate_entry_bindings(silex, &nest.children)?);
            }
        }
    }
    Ok(bindings)
}

fn generate_catalog_definitions(
    silex: &TokenStream,
    scope: &syn::Lifetime,
    catalog: &Ident,
    nodes: &[MacroNode],
    definitions: &mut Vec<TokenStream>,
    is_root: bool,
) {
    for node in nodes {
        if let MacroNode::Nest(nest) = node {
            generate_catalog_definitions(
                silex,
                scope,
                &nest.catalog,
                &nest.children,
                definitions,
                false,
            );
        }
    }

    let table_field = format_ident!("__silex_route_table");
    let mounted_catalog = mounted_catalog_ident(catalog);
    let child_fields = nodes.iter().filter_map(|node| match node {
        MacroNode::Leaf(_) => None,
        MacroNode::Nest(nest) => {
            let field = nested_catalog_field(&nest.name);
            let catalog = &nest.catalog;
            Some(quote! { #field: #catalog<#scope> })
        }
    });
    let methods = nodes
        .iter()
        .flat_map(|node| match node {
            MacroNode::Leaf(route) => generate_path_method(silex, route).into_iter().collect(),
            MacroNode::Nest(nest) => {
                let name = &nest.name;
                let field = nested_catalog_field(name);
                let catalog = &nest.catalog;
                vec![quote! {
                    fn #name(&self) -> &#catalog<#scope> {
                        &self.#field
                    }
                }]
            }
        })
        .collect::<Vec<_>>();
    let mount_method =
        generate_catalog_mount_method(silex, scope, nodes, &mounted_catalog, is_root);
    let mounted_child_fields = nodes.iter().filter_map(|node| match node {
        MacroNode::Leaf(_) => None,
        MacroNode::Nest(nest) => {
            let field = nested_catalog_field(&nest.name);
            let catalog = mounted_catalog_ident(&nest.catalog);
            Some(quote! { #field: #catalog<#scope> })
        }
    });
    let mounted_methods = nodes
        .iter()
        .flat_map(|node| match node {
            MacroNode::Leaf(route) => generate_mounted_path_method(silex, route)
                .into_iter()
                .collect(),
            MacroNode::Nest(nest) => {
                let name = &nest.name;
                let field = nested_catalog_field(name);
                let catalog = mounted_catalog_ident(&nest.catalog);
                vec![quote! {
                    fn #name(&self) -> &#catalog<#scope> {
                        &self.#field
                    }
                }]
            }
        })
        .collect::<Vec<_>>();

    definitions.push(quote! {
        #[allow(dead_code)]
        struct #catalog<#scope> {
            #table_field: #silex::router::RouteTable<#scope>,
            #(#child_fields),*
        }

        #[allow(dead_code)]
        impl<#scope> #catalog<#scope> {
            fn table(&self) -> #silex::router::RouteTable<#scope> {
                self.#table_field.clone()
            }

            #(#methods)*
            #mount_method
        }

        #[allow(dead_code)]
        struct #mounted_catalog<#scope> {
            __silex_mounted: #silex::router::MountedCatalog<#scope>,
            #(#mounted_child_fields),*
        }

        #[allow(dead_code)]
        impl<#scope> #mounted_catalog<#scope> {
            fn table(&self) -> #silex::router::RouteTable<#scope> {
                self.__silex_mounted.table()
            }

            fn prefix(&self) -> &#silex::router::RoutePath {
                self.__silex_mounted.prefix()
            }

            #(#mounted_methods)*
        }
    });
}

fn nested_catalog_field(name: &Ident) -> Ident {
    format_ident!("__silex_routes_child_{name}")
}

fn mounted_catalog_ident(catalog: &Ident) -> Ident {
    format_ident!("__silex_routes_mounted_{catalog}")
}

fn generate_catalog_mount_method(
    silex: &TokenStream,
    scope: &syn::Lifetime,
    nodes: &[MacroNode],
    mounted_catalog: &Ident,
    is_root: bool,
) -> TokenStream {
    let mount_method = format_ident!("__silex_routes_mount");
    let mut child_bindings = Vec::new();
    let mut child_fields = Vec::new();
    for node in nodes {
        let MacroNode::Nest(nest) = node else {
            continue;
        };
        let field = nested_catalog_field(&nest.name);
        let prefix = format_ident!("__silex_routes_prefix_{}", nest.name);
        let value = format_ident!("__silex_routes_mounted_child_{}", nest.name);
        let nested_prefix = &nest.prefix;
        let nested_mount = quote! {
            let #prefix = __silex_mounted.child_prefix(#nested_prefix)?;
            let #value = self.#field.#mount_method(#prefix)?;
        };
        child_bindings.push(nested_mount);
        child_fields.push(quote! { #field: #value });
    }

    let at_method = if is_root {
        quote! {
            fn at(
                self,
                prefix: &'static str,
            ) -> ::std::result::Result<#mounted_catalog<#scope>, #silex::router::PathError> {
                let prefix = #silex::router::RoutePath::new(prefix)?;
                self.#mount_method(prefix)
            }
        }
    } else {
        quote! {}
    };

    quote! {
        fn #mount_method(
            self,
            prefix: #silex::router::RoutePath,
        ) -> ::std::result::Result<#mounted_catalog<#scope>, #silex::router::PathError> {
            let __silex_mounted = #silex::router::MountedCatalog::from_parts(
                prefix,
                self.__silex_route_table.clone(),
            );
            #(#child_bindings)*
            Ok(#mounted_catalog {
                __silex_mounted,
                #(#child_fields),*
            })
        }

        #at_method
    }
}

fn generate_catalog_binding(
    silex: &TokenStream,
    catalog: &Ident,
    nodes: &[MacroNode],
    value_name: &Ident,
    bindings: &mut Vec<TokenStream>,
) {
    for node in nodes {
        if let MacroNode::Nest(nest) = node {
            generate_catalog_binding(
                silex,
                &nest.catalog,
                &nest.children,
                &nest.value_name,
                bindings,
            );
        }
    }

    let table_field = format_ident!("__silex_route_table");
    let table = generate_table_expression(silex, nodes);
    let child_fields = nodes.iter().filter_map(|node| match node {
        MacroNode::Leaf(_) => None,
        MacroNode::Nest(nest) => {
            let field = nested_catalog_field(&nest.name);
            let value_name = &nest.value_name;
            Some(quote! { #field: #value_name? })
        }
    });

    bindings.push(quote! {
        let #value_name: ::std::result::Result<_, #silex::router::RoutePatternError> = (|| {
            Ok(#catalog {
                #table_field: #table,
                #(#child_fields),*
            })
        })();
    });
}

fn generate_table_expression(silex: &TokenStream, nodes: &[MacroNode]) -> TokenStream {
    let entry_values = nodes
        .iter()
        .filter_map(|node| match node {
            MacroNode::Leaf(route) => {
                let entry_name = &route.entry_name;
                Some(quote! { #entry_name? })
            }
            MacroNode::Nest(_) => None,
        })
        .collect::<Vec<_>>();
    let mut table = quote! {{
        let __silex_route_entries = ::std::vec![#(#entry_values),*];
        #silex::router::RouteTable::from_entries(__silex_route_entries)?
    }};

    for node in nodes {
        let MacroNode::Nest(nest) = node else {
            continue;
        };
        let prefix = &nest.prefix;
        let child_value = &nest.value_name;
        let layout_name = &nest.layout_name;
        let child_table = quote! {
            #child_value
                .as_ref()
                .map_err(|error| error.clone())?
                .table()
        };
        let context = format_ident!("__silex_nested_context");
        let outlet = format_ident!("__silex_nested_outlet");
        let mut layout_view = quote! {
            #silex::dom::view::View::into_any(
                (#layout_name)(#context, #outlet)
            )
        };
        for guard in nest.guards.iter().rev() {
            layout_view = quote! {
                #silex::dom::view::View::into_any(#guard(#layout_view))
            };
        }
        table = quote! {
            #table.nest(
                #prefix,
                #child_table,
                move |#context, #outlet| {
                    #layout_view
                },
            )?
        };
    }

    table
}

fn generate_entry_binding(silex: &TokenStream, route: &MacroRoute) -> TokenStream {
    let pattern = &route.pattern;
    let entry_name = &route.entry_name;
    let handler_name = &route.handler_name;
    let matched = format_ident!("__silex_route_match");
    let context = format_ident!("__silex_route_context");
    let arguments = route.params.iter().map(|param| {
        let name = &param.name;
        let ty = &param.ty;
        quote! { #matched.parse::<#ty>(#name).ok()? }
    });

    let mut view = quote! {
        #silex::dom::view::View::into_any((#handler_name)(
            #context,
            #(#arguments),*
        ))
    };
    for guard in route.guards.iter().rev() {
        view = quote! {
            #silex::dom::view::View::into_any(#guard(#view))
        };
    }

    quote! {
        let #entry_name = #silex::router::RouteEntry::new(
            #pattern,
            move |#matched, #context| {
                let _ = &#matched;
                Some(#view)
            },
        )
        ;
    }
}

fn generate_mounted_path_method(silex: &TokenStream, route: &MacroRoute) -> Option<TokenStream> {
    if route
        .segments
        .iter()
        .any(|segment| matches!(segment, MacroSegment::Wildcard { name: None }))
    {
        return None;
    }

    let name = &route.name;
    let arguments = route.params.iter().map(|param| {
        let ident = &param.ident;
        let ty = &param.ty;
        quote! { #ident: #ty }
    });
    let mut path_parts = Vec::new();

    for segment in &route.segments {
        path_parts.push(quote! {
            __silex_route_path.push('/');
        });
        match segment {
            MacroSegment::Static { raw } => path_parts.push(quote! {
                __silex_route_path.push_str(#raw);
            }),
            MacroSegment::Param { name } | MacroSegment::Wildcard { name: Some(name) } => {
                let param = route.params.iter().find(|param| param.name == *name);
                let param = param?;
                let ident = &param.ident;
                let ty = &param.ty;
                path_parts.push(quote! {
                    let __silex_encoded_segment =
                        <#ty as #silex::router::PathParam>::encode_segment(&#ident)
                            .map_err(|error| {
                                #silex::router::PathParamError::InvalidValue(error.to_string())
                            })?;
                    __silex_route_path.push_str(&__silex_encoded_segment);
                });
            }
            MacroSegment::Wildcard { name: None } => {}
        }
    }

    Some(quote! {
        fn #name(
            &self,
            #(#arguments),*
        ) -> ::std::result::Result<#silex::router::RoutePath, #silex::router::PathParamError> {
            let mut __silex_route_path =
                self.__silex_mounted.prefix().as_str().to_string();
            #(#path_parts)*
            #silex::router::RoutePath::new(__silex_route_path)
                .map_err(#silex::router::PathParamError::from)
        }
    })
}

fn generate_path_method(silex: &TokenStream, route: &MacroRoute) -> Option<TokenStream> {
    if route
        .destination_segments
        .iter()
        .any(|segment| matches!(segment, MacroSegment::Wildcard { name: None }))
    {
        return None;
    }

    let name = &route.name;
    let arguments = route.params.iter().map(|param| {
        let ident = &param.ident;
        let ty = &param.ty;
        quote! { #ident: #ty }
    });
    let mut path_parts = if route.destination_segments.is_empty() {
        vec![quote! {
            __silex_route_path.push('/');
        }]
    } else {
        Vec::new()
    };

    for segment in &route.destination_segments {
        path_parts.push(quote! {
            __silex_route_path.push('/');
        });
        match segment {
            MacroSegment::Static { raw } => path_parts.push(quote! {
                __silex_route_path.push_str(#raw);
            }),
            MacroSegment::Param { name } | MacroSegment::Wildcard { name: Some(name) } => {
                let param = route.params.iter().find(|param| param.name == *name);
                let param = param?;
                let ident = &param.ident;
                let ty = &param.ty;
                path_parts.push(quote! {
                    let __silex_encoded_segment =
                        <#ty as #silex::router::PathParam>::encode_segment(&#ident)
                            .map_err(|error| {
                                #silex::router::PathParamError::InvalidValue(error.to_string())
                            })?;
                    __silex_route_path.push_str(&__silex_encoded_segment);
                });
            }
            MacroSegment::Wildcard { name: None } => {}
        }
    }

    Some(quote! {
        fn #name(
            &self,
            #(#arguments),*
        ) -> ::std::result::Result<#silex::router::RoutePath, #silex::router::PathParamError> {
            let mut __silex_route_path = ::std::string::String::new();
            #(#path_parts)*
            #silex::router::RoutePath::new(__silex_route_path)
                .map_err(#silex::router::PathParamError::from)
        }
    })
}
