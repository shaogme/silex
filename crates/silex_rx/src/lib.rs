#![allow(linker_messages)]

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote};
use std::{
    collections::{HashMap, HashSet},
    iter::once,
};
use syn::{
    Block, Error, Expr, ExprBlock, Ident, Macro, Result, parse2,
    token::Move,
    visit_mut::{VisitMut, visit_expr_mut, visit_macro_mut},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum SourceKind {
    Shorthand,
    Explicit,
}

struct SourceBinding {
    id: usize,
    source: TokenStream2,
    marker: Ident,
    reference: Ident,
    promoted: Ident,
}

#[derive(Default)]
struct SourceRegistry {
    bindings: Vec<SourceBinding>,
    keys: HashMap<(SourceKind, String), usize>,
    markers: HashMap<String, usize>,
}

impl SourceRegistry {
    fn register_shorthand(&mut self, identifier: Ident) -> Ident {
        let source = quote!(#identifier);
        let key = (SourceKind::Shorthand, source.to_string());
        if let Some(id) = self.keys.get(&key) {
            return self.bindings[*id].marker.clone();
        }

        let marker = format_ident!("__silex_rx_sig_{}", identifier, span = identifier.span());
        let reference = format_ident!("__ref_{}", identifier, span = identifier.span());
        self.register(key, source, marker.clone(), reference);
        marker
    }

    fn register_explicit(&mut self, source: TokenStream2, span: Span) -> Ident {
        let key = (SourceKind::Explicit, source.to_string());
        if let Some(id) = self.keys.get(&key) {
            return self.bindings[*id].marker.clone();
        }

        let id = self.bindings.len();
        let marker = format_ident!("__silex_rx_src_{id}", span = span);
        let reference = format_ident!("__silex_rx_ref_{id}", span = span);
        self.register(key, source, marker.clone(), reference);
        marker
    }

    fn register(
        &mut self,
        key: (SourceKind, String),
        source: TokenStream2,
        marker: Ident,
        reference: Ident,
    ) {
        let id = self.bindings.len();
        self.keys.insert(key, id);
        self.markers.insert(marker.to_string(), id);
        self.bindings.push(SourceBinding {
            id,
            source,
            marker,
            reference,
            promoted: format_ident!("__silex_rx_source_{id}"),
        });
    }

    fn binding_for_marker(&self, identifier: &Ident) -> Option<&SourceBinding> {
        self.markers
            .get(&identifier.to_string())
            .and_then(|id| self.bindings.get(*id))
    }

    fn active_bindings<'a>(&'a self, used: &HashSet<usize>) -> Vec<&'a SourceBinding> {
        self.bindings
            .iter()
            .filter(|binding| used.contains(&binding.id))
            .collect()
    }
}

struct SignalVisitor<'registry> {
    registry: &'registry SourceRegistry,
    used: HashSet<usize>,
}

impl SignalVisitor<'_> {
    fn rewrite_identifier(&mut self, identifier: &mut Ident) {
        let Some(binding) = self.registry.binding_for_marker(identifier) else {
            return;
        };
        self.used.insert(binding.id);
        *identifier = binding.reference.clone();
    }
}

impl VisitMut for SignalVisitor<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Path(path) = expression
            && let Some(segment) = path.path.segments.last_mut()
        {
            self.rewrite_identifier(&mut segment.ident);
        }
        visit_expr_mut(self, expression);
    }

    fn visit_macro_mut(&mut self, macro_call: &mut Macro) {
        macro_call.tokens =
            rewrite_tokens(macro_call.tokens.clone(), self.registry, &mut self.used);
        visit_macro_mut(self, macro_call);
    }
}

fn rewrite_tokens(
    tokens: TokenStream2,
    registry: &SourceRegistry,
    used: &mut HashSet<usize>,
) -> TokenStream2 {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Ident(mut identifier) => {
                if let Some(binding) = registry.binding_for_marker(&identifier) {
                    used.insert(binding.id);
                    identifier = binding.reference.clone();
                }
                TokenTree::Ident(identifier)
            }
            TokenTree::Group(group) => {
                let inner = rewrite_tokens(group.stream(), registry, used);
                let mut rewritten = Group::new(group.delimiter(), inner);
                rewritten.set_span(group.span());
                TokenTree::Group(rewritten)
            }
            other => other,
        })
        .collect()
}

fn preprocess_tokens(tokens: TokenStream2, registry: &mut SourceRegistry) -> Result<TokenStream2> {
    let mut output = TokenStream2::new();
    let mut tokens = tokens.into_iter().peekable();

    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(punct) if punct.as_char() == '$' => {
                if let Some(TokenTree::Ident(identifier)) = tokens.peek() {
                    let identifier = identifier.clone();
                    tokens.next();
                    let marker = registry.register_shorthand(identifier);
                    output.extend(once(TokenTree::Ident(marker)));
                } else if let Some(TokenTree::Group(group)) = tokens.peek()
                    && group.delimiter() == Delimiter::Parenthesis
                {
                    let group = group.clone();
                    tokens.next();
                    let source: Expr = parse2(group.stream()).map_err(|_| {
                        Error::new(
                            group.span(),
                            "explicit reactive source must be a path or field access",
                        )
                    })?;
                    validate_source_expression(&source)?;
                    let marker = registry.register_explicit(group.stream(), group.span());
                    output.extend(once(TokenTree::Ident(marker)));
                } else {
                    return Err(Error::new(
                        punct.span(),
                        "invalid reactive source: use $name or $(source)",
                    ));
                }
            }
            TokenTree::Group(group) => {
                let inner = preprocess_tokens(group.stream(), registry)?;
                let mut rewritten = Group::new(group.delimiter(), inner);
                rewritten.set_span(group.span());
                output.extend(once(TokenTree::Group(rewritten)));
            }
            other => output.extend(once(other)),
        }
    }

    Ok(output)
}

fn validate_source_expression(expression: &Expr) -> Result<()> {
    match expression {
        Expr::Path(_) => Ok(()),
        Expr::Field(field) => validate_source_expression(&field.base),
        Expr::Paren(paren) => validate_source_expression(&paren.expr),
        _ => Err(Error::new_spanned(
            expression,
            "explicit reactive source must be a path or field access; put method calls outside $(...)",
        )),
    }
}

fn split_at_semicolon(tokens: &mut impl Iterator<Item = TokenTree>) -> Option<TokenStream2> {
    let mut part = TokenStream2::new();
    for token in tokens {
        if matches!(&token, TokenTree::Punct(punct) if punct.as_char() == ';') {
            return Some(part);
        }
        part.extend(once(token));
    }
    None
}

fn nested_reads(bindings: &[&SourceBinding], body: &TokenStream2, promoted: bool) -> TokenStream2 {
    let Some(binding) = bindings.first() else {
        return quote! {{ #body }};
    };
    let source = if promoted {
        let promoted = &binding.promoted;
        quote!(#promoted)
    } else {
        let source = &binding.source;
        quote!(#source)
    };
    let reference = &binding.reference;
    let rest = nested_reads(&bindings[1..], body, promoted);
    quote! {
        (#source).with(|#reference| #rest)
    }
}

fn source_setup(bindings: &[&SourceBinding]) -> TokenStream2 {
    let sources = bindings.iter().map(|binding| {
        let promoted = &binding.promoted;
        let source = &binding.source;
        quote! {
            let #promoted = __silex_scope.promote(#source);
        }
    });
    quote! {
        #(#sources)*
    }
}

fn input_set(prefix: &TokenStream2, bindings: &[&SourceBinding]) -> TokenStream2 {
    let sources = bindings.iter().map(|binding| {
        let promoted = &binding.promoted;
        quote! {
            __silex_inputs.extend(&#promoted.runtime_inputs());
        }
    });
    quote! {
        {
            let mut __silex_inputs = #prefix::RuntimeInputs::new();
            #(#sources)*
            __silex_inputs
        }
    }
}

fn expand(input: TokenStream2) -> Result<TokenStream2> {
    let mut tokens = input.into_iter();
    let prefix = split_at_semicolon(&mut tokens).ok_or_else(|| {
        Error::new(
            proc_macro2::Span::call_site(),
            "rx! requires prefix; scope; body",
        )
    })?;
    if prefix.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "rx! prefix cannot be empty",
        ));
    }
    let scope_tokens = split_at_semicolon(&mut tokens).ok_or_else(|| {
        Error::new(
            proc_macro2::Span::call_site(),
            "rx! requires an explicit scope before the body",
        )
    })?;
    if scope_tokens.is_empty() {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "rx! scope cannot be empty",
        ));
    }
    let scope: Expr = parse2(scope_tokens)?;
    let body: TokenStream2 = tokens.collect();
    if body.is_empty() {
        return Ok(quote! {{ let __silex_scope = #scope; __silex_scope.constant(()) }});
    }

    let mut registry = SourceRegistry::default();
    let processed = preprocess_tokens(body.clone(), &mut registry)?;

    let mut force_derived = false;
    let mut expression_tokens = processed.clone();
    let mut expression_iter = processed.into_iter().peekable();
    if matches!(expression_iter.peek(), Some(TokenTree::Punct(punct)) if punct.as_char() == '@') {
        expression_iter.next();
        if matches!(expression_iter.peek(), Some(TokenTree::Ident(identifier)) if identifier == "fn")
        {
            expression_iter.next();
            force_derived = true;
            expression_tokens = expression_iter.collect();
        }
    }

    let mut expression = match parse2::<Expr>(expression_tokens.clone()) {
        Ok(expression) => expression,
        Err(_) => Expr::Block(ExprBlock {
            attrs: Vec::new(),
            label: None,
            block: parse2::<Block>(expression_tokens.clone())?,
        }),
    };

    let mut visitor = SignalVisitor {
        registry: &registry,
        used: HashSet::new(),
    };
    visitor.visit_expr_mut(&mut expression);
    let bindings = registry.active_bindings(&visitor.used);
    let scope_binding = quote! { let __silex_scope = #scope; };

    if let Expr::Closure(mut closure) = expression {
        let closure_body = *closure.body;
        let closure_body = quote! { #closure_body };
        let reads = nested_reads(&bindings, &closure_body, false);
        closure.capture = Some(Move::default());
        *closure.body = parse2(quote! { Ok(#reads) })?;
        let constructor = if closure.inputs.is_empty() {
            let setup = source_setup(&bindings);
            let inputs = input_set(&prefix, &bindings);
            quote! {
                #setup
                __silex_scope.derived_from(
                    #inputs,
                    #closure,
                    __silex_scope.error_handler(|error| panic!("rx! derived failed: {error}")),
                ).unwrap_or_else(|error| panic!("创建 rx! derived 失败: {error}"))
            }
        } else {
            quote! { __silex_scope.callback(#closure) }
        };
        return Ok(quote! {{ #scope_binding #constructor }});
    }

    let expression = quote! { #expression };
    let reads = nested_reads(&bindings, &expression, true);

    if !force_derived
        && bindings.is_empty()
        && matches!(
            expression_tokens.clone().into_iter().next(),
            Some(TokenTree::Literal(_))
        )
        && parse2::<syn::ExprLit>(expression_tokens.clone()).is_ok()
    {
        return Ok(quote! {{ #scope_binding __silex_scope.constant(#expression) }});
    }

    let setup = source_setup(&bindings);
    let inputs = input_set(&prefix, &bindings);
    Ok(quote! {{
        #scope_binding
        #setup
        __silex_scope.derived_from(
            #inputs,
            move || Ok(#reads),
            __silex_scope.error_handler(|error| panic!("rx! derived failed: {error}")),
        ).unwrap_or_else(|error| panic!("创建 rx! derived 失败: {error}"))
    }})
}

/// `rx!` process macro. The first section is a dependency prefix, the second
/// section is the explicit scope expression.
#[proc_macro]
pub fn rx(input: TokenStream) -> TokenStream {
    match expand(TokenStream2::from(input)) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn preprocesses_dollar_identifiers() {
        let body: TokenStream2 = "$count + $user.name".parse().unwrap();
        let mut registry = SourceRegistry::default();
        let tokens = preprocess_tokens(body, &mut registry).unwrap();
        assert!(tokens.to_string().contains("__silex_rx_sig_count"));
    }

    #[test]
    fn preprocesses_parenthesized_source_expression() {
        let body: TokenStream2 = "$(settings.theme)".parse().unwrap();
        let mut registry = SourceRegistry::default();
        let tokens = preprocess_tokens(body, &mut registry).unwrap();

        assert!(tokens.to_string().contains("__silex_rx_src_0"));
        assert_eq!(registry.bindings.len(), 1);
        assert_eq!(registry.bindings[0].source.to_string(), "settings . theme");
    }

    #[test]
    fn rejects_non_source_parenthesized_expression() {
        let input: TokenStream2 = "::silex_core; scope; $(settings.theme.clone())"
            .parse()
            .unwrap();
        let error = expand(input).unwrap_err();

        assert!(error.to_string().contains("path or field access"));
    }

    #[test]
    fn rewrites_explicit_source_inside_nested_macro() {
        let input: TokenStream2 = "::silex_core; scope; format!(\"Theme: {}\", $(settings.theme))"
            .parse()
            .unwrap();
        let output = expand(input).unwrap().to_string();

        assert!(output.contains("settings . theme"));
        assert!(output.contains("__silex_rx_ref_0"));
        assert_eq!(output.matches("promote").count(), 1);
    }

    #[test]
    fn keeps_legacy_field_access_on_the_root_source() {
        let output = expand(quote! {
            ::silex_core;
            scope;
            $state.name.clone()
        })
        .unwrap()
        .to_string();

        assert!(output.contains("promote"));
        assert!(output.contains("state"));
        assert!(output.contains("__ref_state"));
        assert!(output.contains("name"));
    }

    #[test]
    fn deduplicates_repeated_explicit_sources() {
        let input: TokenStream2 = "::silex_core; scope; $(settings.theme) == $(settings.theme)"
            .parse()
            .unwrap();
        let output = expand(input).unwrap().to_string();

        assert_eq!(output.matches("promote").count(), 1);
    }

    #[test]
    fn rejects_missing_scope_section() {
        let error = expand(quote! { ::silex_core; $count }).unwrap_err();
        assert!(error.to_string().contains("explicit scope"));
    }

    #[test]
    fn emits_only_scoped_constructors() {
        let output = expand(quote! { ::silex_core; scope; $count + 1 })
            .unwrap()
            .to_string();
        assert!(output.contains("derived"));
        assert!(!output.contains("new_op"));
        let old_ref_count = ["R", "c"].concat();
        assert!(!output.contains(&old_ref_count));
    }

    #[test]
    fn routes_parameterless_closures_to_derived() {
        let output = expand(quote! { ::silex_core; scope; || $count + 1 })
            .unwrap()
            .to_string();
        assert!(output.contains("derived"));
        assert!(!output.contains("callback"));
    }

    #[test]
    fn routes_parameterized_closures_to_callback() {
        let output = expand(quote! { ::silex_core; scope; |value: i32| value + 1 })
            .unwrap()
            .to_string();
        assert!(output.contains("callback"));
        assert!(!output.contains("derived"));
    }

    #[test]
    fn keeps_at_fn_on_the_typed_derived_path() {
        let output = expand(quote! {
            ::silex_core;
            scope;
            @fn $a + $b + $c + $d
        })
        .unwrap()
        .to_string();
        assert!(output.contains("derived"));
        assert!(!output.contains("map1_static"));
        assert!(!output.contains("map2_static"));
        assert!(!output.contains("map3_static"));
    }
}
