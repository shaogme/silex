use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::parse::{Parse, ParseStream};
use syn::{Error, ExprClosure, Ident, LitStr, Pat, PathArguments, Token, Type};

struct RoutesInput {
    catalog: Ident,
    routes: syn::punctuated::Punctuated<RouteMacroDef, Token![,]>,
}

impl Parse for RoutesInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let catalog = input.parse()?;
        let content;
        syn::braced!(content in input);
        let routes = content.parse_terminated(RouteMacroDef::parse, Token![,])?;
        Ok(Self { catalog, routes })
    }
}

struct RouteMacroDef {
    name: Ident,
    path: LitStr,
    handler: ExprClosure,
    guards: Vec<syn::Path>,
}

impl Parse for RouteMacroDef {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        let path = input.parse()?;
        let guards_before = parse_guard_metadata(input)?;
        input.parse::<Token![=>]>()?;
        let handler = input.parse()?;
        let mut guards_after = parse_guard_metadata(input)?;
        if guards_after.is_none() && comma_precedes_guard_metadata(input) {
            input.parse::<Token![,]>()?;
            guards_after = parse_guard_metadata(input)?;
        }

        if guards_before.is_some() && guards_after.is_some() {
            return Err(Error::new_spanned(
                &name,
                "route guards may be declared only once",
            ));
        }

        Ok(Self {
            name,
            path,
            handler,
            guards: guards_before.or(guards_after).unwrap_or_default(),
        })
    }
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
    key: Vec<MacroPatternKey>,
    segments: Vec<MacroSegment>,
    params: Vec<HandlerPathParam>,
    handler: ExprClosure,
    guards: Vec<syn::Path>,
}

fn compile_macro_route(def: RouteMacroDef) -> syn::Result<MacroRoute> {
    let (pattern, segments, key) = parse_macro_pattern(&def.path)?;
    let params = validate_handler(&def.handler, &def.name, &segments)?;

    Ok(MacroRoute {
        name: def.name,
        pattern,
        key,
        segments,
        params,
        handler: def.handler,
        guards: def.guards,
    })
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
    let catalog = input.catalog.clone();
    let mut route_names = HashSet::new();
    let mut pattern_keys = HashSet::new();
    let mut routes = Vec::with_capacity(input.routes.len());

    for definition in input.routes {
        let route = compile_macro_route(definition)?;
        if route.name == "table" {
            return Err(Error::new_spanned(
                &route.name,
                "route name `table` is reserved by the generated catalog",
            ));
        }
        if !route_names.insert(route.name.to_string()) {
            return Err(Error::new_spanned(
                &route.name,
                "route names in one catalog must be unique",
            ));
        }
        if !pattern_keys.insert(route.key.clone()) {
            return Err(Error::new_spanned(
                &route.name,
                format!(
                    "route pattern `{}` conflicts with another normalized route pattern",
                    route.pattern
                ),
            ));
        }
        routes.push(route);
    }

    let silex = crate::crate_path::silex();
    let scope = syn::Lifetime::new("'__silex_routes_scope", proc_macro2::Span::call_site());
    let table_field = format_ident!("__silex_route_table");
    let entry_names: Vec<_> = (0..routes.len())
        .map(|index| format_ident!("__silex_routes_entry_{index}"))
        .collect();
    let handler_names: Vec<_> = (0..routes.len())
        .map(|index| format_ident!("__silex_routes_handler_{index}"))
        .collect();

    let handler_bindings = routes
        .iter()
        .zip(&handler_names)
        .map(|(route, handler_name)| {
            let handler = &route.handler;
            quote! {
                let #handler_name = #handler;
            }
        });

    let entry_bindings = routes
        .iter()
        .zip(entry_names.iter().zip(&handler_names))
        .map(|(route, (entry_name, handler_name))| {
            generate_entry_binding(&silex, route, entry_name, handler_name)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let methods = routes
        .iter()
        .filter_map(|route| generate_path_method(&silex, route))
        .collect::<Vec<_>>();

    Ok(quote! {{
        #(#handler_bindings)*
        #(#entry_bindings)*

        struct #catalog <#scope> {
            #table_field: #silex::router::RouteTable<#scope>,
        }

        impl <#scope> #catalog <#scope> {
            fn table(&self) -> #silex::router::RouteTable<#scope> {
                self.#table_field.clone()
            }

            #(#methods)*
        }

        let __silex_route_entries = ::std::vec![#(#entry_names),*];
        #catalog {
            #table_field: #silex::router::RouteTable::from_entries(__silex_route_entries)
                .expect("routes! generated an invalid route table"),
        }
    }})
}

fn generate_entry_binding(
    silex: &TokenStream,
    route: &MacroRoute,
    entry_name: &Ident,
    handler_name: &Ident,
) -> syn::Result<TokenStream> {
    let pattern = &route.pattern;
    let matched = format_ident!("__silex_route_match");
    let context = format_ident!("__silex_route_context");
    let arguments = route.params.iter().map(|param| {
        let name = &param.name;
        let ty = &param.ty;
        quote! { #matched.parse::<#ty>(#name).ok()? }
    });

    let mut view = quote! {
        #silex::dom::view::View::into_any((#handler_name)(#context, #(#arguments),*))
    };
    for guard in route.guards.iter().rev() {
        view = quote! {
            #silex::dom::view::View::into_any(#guard(#view))
        };
    }

    Ok(quote! {
        let #entry_name = #silex::router::RouteEntry::new(
            #pattern,
            move |#matched, #context| {
                let _ = &#matched;
                Some(#view)
            },
        )
        .expect("routes! generated an invalid route entry");
    })
}

fn generate_path_method(silex: &TokenStream, route: &MacroRoute) -> Option<TokenStream> {
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
    let mut path_parts = if route.segments.is_empty() {
        vec![quote! {
            __silex_route_path.push('/');
        }]
    } else {
        Vec::new()
    };

    for segment in &route.segments {
        path_parts.push(quote! {
            __silex_route_path.push('/');
        });
        match segment {
            MacroSegment::Static { raw } => path_parts.push(quote! {
                __silex_route_path.push_str(#raw);
            }),
            MacroSegment::Param { name } | MacroSegment::Wildcard { name: Some(name) } => {
                let param = route
                    .params
                    .iter()
                    .find(|param| param.name == *name)
                    .expect("validated route parameter is missing from handler parameters");
                let ident = &param.ident;
                let ty = &param.ty;
                path_parts.push(quote! {
                    let __silex_encoded_segment =
                        <#ty as #silex::router::PathParam>::encode_segment(&#ident)
                            .unwrap_or_else(|error| {
                                panic!("routes! could not encode a route parameter: {}", error)
                            });
                    __silex_route_path.push_str(&__silex_encoded_segment);
                });
            }
            MacroSegment::Wildcard { name: None } => {}
        }
    }

    Some(quote! {
        fn #name(&self, #(#arguments),*) -> #silex::router::RoutePath {
            let mut __silex_route_path = ::std::string::String::new();
            #(#path_parts)*
            #silex::router::RoutePath::new(__silex_route_path).unwrap_or_else(|error| {
                panic!("routes! generated an invalid destination path: {}", error)
            })
        }
    })
}
