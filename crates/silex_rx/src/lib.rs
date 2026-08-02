#![allow(linker_messages)]

use proc_macro::TokenStream;
use proc_macro2::{Group, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote};
use std::{collections::HashMap, iter::once};
use syn::{
    Block, Error, Expr, ExprBlock, Ident, Macro, Result, parse2,
    token::Move,
    visit_mut::{VisitMut, visit_expr_mut, visit_macro_mut},
};

struct SignalVisitor {
    signal_map: HashMap<Ident, Ident>,
}

impl VisitMut for SignalVisitor {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Path(path) = expression
            && let Some(segment) = path.path.segments.last_mut()
        {
            let name = segment.ident.to_string();
            if let Some(original) = name.strip_prefix("__silex_rx_sig_") {
                let original = format_ident!("{}", original, span = segment.ident.span());
                let reference = self
                    .signal_map
                    .entry(original.clone())
                    .or_insert_with(|| format_ident!("__ref_{}", original));
                segment.ident = reference.clone();
            }
        }
        visit_expr_mut(self, expression);
    }

    fn visit_macro_mut(&mut self, macro_call: &mut Macro) {
        fn rewrite_tokens(
            tokens: TokenStream2,
            signal_map: &mut HashMap<Ident, Ident>,
        ) -> TokenStream2 {
            tokens
                .into_iter()
                .map(|token| match token {
                    TokenTree::Ident(identifier) => {
                        let name = identifier.to_string();
                        if let Some(original) = name.strip_prefix("__silex_rx_sig_") {
                            let original = format_ident!("{}", original, span = identifier.span());
                            let reference = signal_map
                                .entry(original.clone())
                                .or_insert_with(|| format_ident!("__ref_{}", original));
                            TokenTree::Ident(reference.clone())
                        } else {
                            TokenTree::Ident(identifier)
                        }
                    }
                    TokenTree::Group(group) => {
                        let inner = rewrite_tokens(group.stream(), signal_map);
                        let mut rewritten = Group::new(group.delimiter(), inner);
                        rewritten.set_span(group.span());
                        TokenTree::Group(rewritten)
                    }
                    other => other,
                })
                .collect()
        }

        macro_call.tokens = rewrite_tokens(macro_call.tokens.clone(), &mut self.signal_map);
        visit_macro_mut(self, macro_call);
    }
}

fn preprocess_tokens(tokens: TokenStream2) -> (TokenStream2, bool) {
    let mut output = TokenStream2::new();
    let mut invalid_dollar = false;
    let mut tokens = tokens.into_iter().peekable();

    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(punct) if punct.as_char() == '$' => {
                if let Some(TokenTree::Ident(identifier)) = tokens.peek() {
                    let identifier = identifier.clone();
                    tokens.next();
                    output.extend(once(TokenTree::Ident(format_ident!(
                        "__silex_rx_sig_{}",
                        identifier,
                        span = identifier.span()
                    ))));
                } else {
                    invalid_dollar = true;
                    output.extend(once(TokenTree::Punct(punct)));
                }
            }
            TokenTree::Group(group) => {
                let (inner, inner_invalid) = preprocess_tokens(group.stream());
                invalid_dollar |= inner_invalid;
                let mut rewritten = Group::new(group.delimiter(), inner);
                rewritten.set_span(group.span());
                output.extend(once(TokenTree::Group(rewritten)));
            }
            other => output.extend(once(other)),
        }
    }

    (output, invalid_dollar)
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

fn nested_reads(pairs: &[(Ident, Ident)], body: &TokenStream2) -> TokenStream2 {
    let Some((signal, reference)) = pairs.first() else {
        return quote! {{ #body }};
    };
    let rest = nested_reads(&pairs[1..], body);
    quote! {
        (#signal).with(|#reference| #rest)
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

    let (processed, invalid_dollar) = preprocess_tokens(body.clone());
    if invalid_dollar {
        return Err(Error::new_spanned(
            body,
            "invalid signal identifier: '$' must be followed by an identifier",
        ));
    }

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
        signal_map: HashMap::new(),
    };
    visitor.visit_expr_mut(&mut expression);
    let mut pairs: Vec<_> = visitor.signal_map.into_iter().collect();
    pairs.sort_by_key(|(identifier, _)| identifier.to_string());
    let scope_binding = quote! { let __silex_scope = #scope; };

    if let Expr::Closure(mut closure) = expression {
        let closure_body = *closure.body;
        let closure_body = quote! { #closure_body };
        let reads = nested_reads(&pairs, &closure_body);
        closure.capture = Some(Move::default());
        *closure.body = parse2(reads)?;
        return Ok(quote! {{ #scope_binding __silex_scope.callback(#closure) }});
    }

    let expression = quote! { #expression };
    let reads = nested_reads(&pairs, &expression);

    if !force_derived
        && pairs.is_empty()
        && matches!(
            expression_tokens.clone().into_iter().next(),
            Some(TokenTree::Literal(_))
        )
        && parse2::<syn::ExprLit>(expression_tokens.clone()).is_ok()
    {
        return Ok(quote! {{ #scope_binding __silex_scope.constant(#expression) }});
    }

    Ok(quote! {{ #scope_binding __silex_scope.derived(move || #reads) }})
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
        let (tokens, invalid) = preprocess_tokens(quote! { $count + $user.name });
        assert!(!invalid);
        assert!(tokens.to_string().contains("__silex_rx_sig_count"));
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
}
