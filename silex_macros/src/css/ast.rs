use proc_macro2::{Delimiter, TokenStream, TokenTree};
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
}

impl Parse for CssRule {
    fn parse(input: ParseStream) -> Result<Self> {
        // We peek to differentiate.
        // It's an @-rule if it starts with @.
        if input.peek(Token![@]) {
            return input.parse().map(CssRule::AtRule);
        }

        // To differentiate between Declaration and Nested rule, we need to inspect tokens
        // until we find a `{` or a `:`. However, `syn` streams are mostly forward-only without `fork`.
        let fork = input.fork();
        let mut is_nested = false;

        while !fork.is_empty() {
            if fork.peek(token::Brace) {
                is_nested = true;
                break;
            }
            if fork.peek(Token![:]) {
                // Not necessarily a declaration, pseudo-classes use `:`.
                // If we see `:` but eventually see `{` before `;`, it's nested.
            }
            if fork.peek(Token![;]) {
                break; // definitely a declaration
            }
            // advance fork
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
    #[allow(dead_code)]
    pub colon_token: Token![:],
    pub values: TokenStream,
    #[allow(dead_code)]
    pub semi_token: Option<Token![;]>,
}

impl Parse for CssDeclaration {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut prop_str = String::new();

        // Parse property name (idents and hyphens)
        loop {
            if input.peek(Ident::peek_any) {
                let id = Ident::parse_any(input)?;
                prop_str.push_str(&id.to_string());
            } else if input.peek(Token![-]) {
                let _dash: Token![-] = input.parse()?;
                prop_str.push('-');
            } else {
                break;
            }
        }

        if prop_str.is_empty() {
            return Err(input.error("Expected CSS property name"));
        }

        let colon_token: Token![:] = input.parse()?;

        // Parse values until `;` or EOF (or `}` if it's the last declaration in a block without a semicolon)
        let mut value_tokens = TokenStream::new();
        while !input.is_empty() && !input.peek(Token![;]) && !input.peek(token::Brace) {
            let tt: TokenTree = input.parse()?;
            value_tokens.extend(std::iter::once(tt));
        }

        // We'll treat the whole chunk of tokens as one Unparsed block,
        // to delegate the dynamic value extraction to the compiler phase or treat it as a stream.
        let values = value_tokens;

        let semi_token = if input.peek(Token![;]) {
            Some(input.parse()?)
        } else {
            None
        };

        Ok(CssDeclaration {
            property: prop_str,
            colon_token,
            values,
            semi_token,
        })
    }
}

/// A nested CSS rule like `&:hover { color: red; }`
#[derive(Clone)]
pub struct CssNested {
    pub selectors: TokenStream,
    #[allow(dead_code)]
    pub brace_token: token::Brace,
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
        let brace_token = syn::braced!(content in input);
        let block: CssBlock = content.parse()?;

        Ok(CssNested {
            selectors,
            brace_token,
            block,
        })
    }
}

/// An @-rule like `@media (max-width: 600px) { ... }`
#[derive(Clone)]
#[allow(dead_code)]
pub struct CssAtRule {
    pub at_token: Token![@],
    pub name: Ident,
    pub params: TokenStream,
    pub brace_token: token::Brace,
    pub block: CssBlock,
}

impl Parse for CssAtRule {
    fn parse(input: ParseStream) -> Result<Self> {
        let at_token: Token![@] = input.parse()?;
        let name: Ident = input.parse()?;

        let mut params = TokenStream::new();
        while !input.peek(token::Brace) && !input.is_empty() {
            let tt: TokenTree = input.parse()?;
            params.extend(std::iter::once(tt));
        }

        let content;
        let brace_token = syn::braced!(content in input);
        let block: CssBlock = content.parse()?;

        Ok(CssAtRule {
            at_token,
            name,
            params,
            brace_token,
            block,
        })
    }
}
