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
    Unsafe(CssUnsafe),
}

impl Parse for CssRule {
    fn parse(input: ParseStream) -> Result<Self> {
        // Fast path for @-rules
        if input.peek(Token![@]) {
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
#[derive(Clone)]
pub struct CssAtRule {
    pub name: Ident,
    pub params: TokenStream,
    pub block: CssBlock,
}

impl Parse for CssAtRule {
    fn parse(input: ParseStream) -> Result<Self> {
        let _at_token: Token![@] = input.parse()?;
        let name: Ident = input.parse()?;

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
