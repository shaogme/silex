use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, Result, Token, token};

/// Represents an entire block of CSS rules.
#[derive(Clone)]
pub struct CssBlock {
    pub rules: Vec<CssRule>,
}

impl Parse for CssBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut rules = Vec::new();
        while !input.is_empty() {
            rules.push(input.parse()?);
        }
        Ok(CssBlock { rules })
    }
}

/// A single CSS rule, either a property declaration, a nested rule, or an @-rule.
#[derive(Clone)]
pub enum CssRule {
    Declaration(CssDeclaration),
    Nested(CssNested),
    AtRule(CssAtRule),
    Unsafe(CssUnsafe),
    Apply(CssApply),
}

impl Parse for CssRule {
    fn parse(input: ParseStream) -> Result<Self> {
        // Fast path for @apply or @-rules
        if input.peek(Token![@]) {
            let fork = input.fork();
            let _: Token![@] = fork.parse()?;
            if fork.peek(Ident) {
                let id: Ident = fork.parse()?;
                if id == "apply" {
                    return input.parse().map(CssRule::Apply);
                }
            }
            return input.parse().map(CssRule::AtRule);
        }

        // Fast path for common nested selectors
        if input.peek(Token![&])
            || input.peek(Token![.])
            || input.peek(Token![#])
            || input.peek(Token![*])
            || input.peek(token::Bracket)
        {
            return input.parse().map(CssRule::Nested);
        }

        // Fast path for unsafe blocks
        if input.peek(Token![unsafe]) && input.peek2(token::Brace) {
            return input.parse().map(CssRule::Unsafe);
        }

        // Fallback to fork for ambiguous cases (like ident-based selectors vs properties)
        let fork = input.fork();
        let mut is_nested = false;

        while !fork.is_empty() {
            if fork.peek(token::Brace) {
                is_nested = true;
                break;
            }
            if fork.peek(Token![;]) {
                break; // Definitely a declaration
            }
            // Skip to the next potential marker
            let _: TokenTree = fork.parse()?;
        }

        if is_nested {
            input.parse().map(CssRule::Nested)
        } else {
            input.parse().map(CssRule::Declaration)
        }
    }
}

/// A CSS declaration like `background-color: red;`
#[derive(Clone)]
pub struct CssDeclaration {
    pub property: String,
    pub values: TokenStream,
    pub semi_token: Option<Token![;]>,
}

impl Parse for CssDeclaration {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut prop_str = String::new();

        // Parse property name (idents and hyphens)
        while input.peek(Ident::peek_any) || input.peek(Token![-]) {
            if input.peek(Ident::peek_any) {
                let id = Ident::parse_any(input)?;
                prop_str.push_str(&id.to_string());
            } else {
                let _: Token![-] = input.parse()?;
                prop_str.push('-');
            }
        }

        if prop_str.is_empty() {
            return Err(input.error("Expected CSS property name"));
        }

        let _colon_token: Token![:] = input.parse()?;

        // Parse values until `;` or EOF or `}`
        let mut value_tokens = TokenStream::new();
        while !input.is_empty() && !input.peek(Token![;]) && !input.peek(token::Brace) {
            value_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }

        let semi_token = if input.peek(Token![;]) {
            Some(input.parse()?)
        } else {
            None
        };

        Ok(CssDeclaration {
            property: prop_str,
            values: value_tokens,
            semi_token,
        })
    }
}

/// A nested CSS rule like `&:hover { color: red; }`
#[derive(Clone)]
pub struct CssNested {
    pub selectors: TokenStream,
    pub block: CssBlock,
}

impl Parse for CssNested {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut selectors = TokenStream::new();
        while !input.peek(token::Brace) && !input.is_empty() {
            if input.peek(Token![$]) {
                let fork = input.fork();
                let _: Token![$] = fork.parse()?;
                if fork.peek(token::Paren) {
                    let _: Token![$] = input.parse()?;
                    let content;
                    syn::parenthesized!(content in input);
                    let ts = content.parse::<TokenStream>()?;

                    // We treat dynamic selector chunks by expanding them back to `$ ( ... )` for TokenStream
                    let mut dollar_paren = TokenStream::new();
                    use proc_macro2::{Group, Punct, Spacing};
                    dollar_paren.extend(std::iter::once(TokenTree::Punct(Punct::new(
                        '$',
                        Spacing::Joint,
                    ))));
                    dollar_paren.extend(std::iter::once(TokenTree::Group(Group::new(
                        Delimiter::Parenthesis,
                        ts,
                    ))));
                    selectors.extend(dollar_paren);
                    continue;
                }
            }

            let tt: TokenTree = input.parse()?;
            selectors.extend(std::iter::once(tt));
        }

        let content;
        let _brace_token = syn::braced!(content in input);
        let block: CssBlock = content.parse()?;

        Ok(CssNested { selectors, block })
    }
}

/// An @-rule like `@media (max-width: 600px) { ... }`
///
/// The name is a `String` rather than an `Ident` because at-rule names may contain
/// hyphens (`@font-face`, `@starting-style`, `@counter-style`) and Rust identifiers
/// cannot. Parsing `@font-face` as a single `Ident` used to stop at `font`, leaving
/// `-face` in `params` — which silently broke both the `is_lifted` check in the
/// compiler and the emitted CSS.
#[derive(Clone)]
pub struct CssAtRule {
    pub name: String,
    pub params: TokenStream,
    pub block: CssBlock,
}

impl Parse for CssAtRule {
    fn parse(input: ParseStream) -> Result<Self> {
        let _at_token: Token![@] = input.parse()?;

        let mut name = String::new();
        loop {
            let id = Ident::parse_any(input)?;
            name.push_str(&id.to_string());
            // 只有紧跟标识符的 `-` 才是名字的一部分（`@font-face`），
            // `@media (min-width: 1px)` 的参数里不会出现这种形状
            if input.peek(Token![-]) && input.peek2(Ident::peek_any) {
                let _: Token![-] = input.parse()?;
                name.push('-');
                continue;
            }
            break;
        }

        let mut params = TokenStream::new();
        while !input.peek(token::Brace) && !input.is_empty() {
            let tt: TokenTree = input.parse()?;
            params.extend(std::iter::once(tt));
        }

        let content;
        let _brace_token = syn::braced!(content in input);
        let block: CssBlock = content.parse()?;

        Ok(CssAtRule {
            name,
            params,
            block,
        })
    }
}

/// An unsafe block like `unsafe { ... }` where validation is disabled.
#[derive(Clone)]
pub struct CssUnsafe {
    pub block: CssBlock,
}

impl Parse for CssUnsafe {
    fn parse(input: ParseStream) -> Result<Self> {
        let _unsafe_token: Token![unsafe] = input.parse()?;
        let content;
        let _brace_token = syn::braced!(content in input);
        let block: CssBlock = content.parse()?;
        Ok(CssUnsafe { block })
    }
}

/// An `@apply` directive like `@apply flex items-center px-4 py-2;`
#[derive(Clone)]
pub struct CssApply {
    pub classes: String,
    #[allow(dead_code)]
    pub semi_token: Option<Token![;]>,
    pub span: proc_macro2::Span,
}

pub fn format_tailwind_token_stream(ts: &TokenStream) -> String {
    fn is_compact_token(tt: &TokenTree) -> bool {
        matches!(
            tt,
            TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Group(_)
        )
    }

    fn stringify_group(g: &proc_macro2::Group) -> String {
        let inner = format_tailwind_token_stream(&g.stream());
        match g.delimiter() {
            Delimiter::Parenthesis => format!("({})", inner.trim()),
            Delimiter::Brace => format!("{{{}}}", inner.trim()),
            Delimiter::Bracket => format!("[{}]", inner.trim()),
            Delimiter::None => inner,
        }
    }

    let mut out = String::new();
    let mut prev_tt: Option<TokenTree> = None;

    for tt in ts.clone() {
        if let Some(ref prev) = prev_tt {
            let starts_new_utility = is_compact_token(&tt)
                || matches!(&tt, TokenTree::Punct(p) if p.as_char() == '@' || p.as_char() == '*');
            if is_compact_token(prev) && starts_new_utility {
                out.push(' ');
            }
        }

        match &tt {
            TokenTree::Group(g) => {
                out.push_str(&stringify_group(g));
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
            }
            TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
            }
        }

        prev_tt = Some(tt);
    }

    out
}

impl Parse for CssApply {
    fn parse(input: ParseStream) -> Result<Self> {
        let at_token: Token![@] = input.parse()?;
        let apply_ident: Ident = input.parse()?;
        if apply_ident != "apply" {
            return Err(syn::Error::new(
                apply_ident.span(),
                "Expected `apply` after `@`",
            ));
        }

        let start_span = at_token.span;
        let mut classes_tokens = TokenStream::new();
        while !input.is_empty() && !input.peek(Token![;]) && !input.peek(token::Brace) {
            classes_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }

        let semi_token = if input.peek(Token![;]) {
            Some(input.parse()?)
        } else {
            None
        };

        let classes_str = format_tailwind_token_stream(&classes_tokens);

        Ok(CssApply {
            classes: classes_str,
            semi_token,
            span: start_span,
        })
    }
}

impl ToTokens for CssBlock {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for rule in &self.rules {
            rule.to_tokens(tokens);
        }
    }
}

impl ToTokens for CssRule {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            CssRule::Declaration(d) => d.to_tokens(tokens),
            CssRule::Nested(n) => n.to_tokens(tokens),
            CssRule::AtRule(a) => a.to_tokens(tokens),
            CssRule::Unsafe(u) => u.to_tokens(tokens),
            CssRule::Apply(ap) => ap.to_tokens(tokens),
        }
    }
}

impl ToTokens for CssDeclaration {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let prop_ts: TokenStream = self.property.parse().unwrap_or_default();
        let vals = &self.values;
        tokens.extend(quote::quote! {
            #prop_ts : #vals ;
        });
    }
}

impl ToTokens for CssNested {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let selectors = &self.selectors;
        let block = &self.block;
        tokens.extend(quote::quote! {
            #selectors {
                #block
            }
        });
    }
}

impl ToTokens for CssAtRule {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // `starting-style` 拆成 `starting - style` 三个 token，由上面的 hyphen-aware
        // `Parse` 原样收回；直接 `Ident::new` 会因名字含 `-` 而 panic
        let name: TokenStream = self.name.parse().unwrap_or_default();
        let params = &self.params;
        let block = &self.block;
        tokens.extend(quote::quote! {
            @ #name #params {
                #block
            }
        });
    }
}

impl ToTokens for CssUnsafe {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let block = &self.block;
        tokens.extend(quote::quote! {
            unsafe {
                #block
            }
        });
    }
}

impl ToTokens for CssApply {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let classes: TokenStream = self.classes.parse().unwrap_or_default();
        tokens.extend(quote::quote! {
            @apply #classes ;
        });
    }
}
