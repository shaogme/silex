use crate::css::ast::{CssBlock, CssRule};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::Result;

pub struct DynamicRule {
    pub template: String,
    pub expressions: Vec<(String, TokenStream)>,
}

pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub final_css: String,
    pub expressions: Vec<(String, TokenStream)>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub theme_refs: Vec<(String, String)>,
    pub hash: u64,
}

struct ParserState {
    static_css: String,
    expressions: Vec<(String, TokenStream)>,
    dynamic_rules: Vec<DynamicRule>,
    theme_refs: Vec<(String, String)>,
    class_name: String,
}

pub struct CssCompiler;

impl CssCompiler {
    pub fn compile(ts: TokenStream, span: Span) -> Result<CssCompileResult> {
        let mut hasher = DefaultHasher::new();
        ts.to_string().hash(&mut hasher);
        let hash = hasher.finish();
        let class_name = format!("slx-{:x}", hash);
        let style_id = format!("style-{}", class_name);

        let mut state = ParserState {
            static_css: String::new(),
            expressions: Vec::new(),
            dynamic_rules: Vec::new(),
            theme_refs: Vec::new(),
            class_name: class_name.clone(),
        };

        let block: CssBlock = syn::parse2(ts)?;

        process_css_block(&block, &mut state)?;

        let final_source_css = state.static_css;

        let wrapped_css = format!(".{} {{ {} }}", class_name, final_source_css);

        let res = if final_source_css.trim().is_empty() {
            "".to_string()
        } else {
            let mut stylesheet = StyleSheet::parse(&wrapped_css, ParserOptions::default())
                .map_err(|e| syn::Error::new(span, format!("Invalid CSS: {}", e)))?;

            stylesheet
                .minify(MinifyOptions::default())
                .map_err(|e| syn::Error::new(span, format!("CSS Minification failed: {}", e)))?;

            stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets: Targets::default(),
                    ..PrinterOptions::default()
                })
                .map_err(|e| syn::Error::new(span, format!("CSS Printing failed: {}", e)))?
                .code
        };

        Ok(CssCompileResult {
            class_name,
            style_id,
            final_css: res,
            expressions: state.expressions,
            dynamic_rules: state.dynamic_rules,
            theme_refs: state.theme_refs,
            hash,
        })
    }
}

fn process_css_block(block: &CssBlock, state: &mut ParserState) -> Result<()> {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                state.static_css.push_str(&decl.property);
                state.static_css.push_str(": ");

                state.static_css.push(' ');
                let mut local_out = String::new();
                extract_dynamic_value(
                    &decl.values,
                    &mut local_out,
                    &mut state.expressions,
                    &mut state.theme_refs,
                    &decl.property,
                    &state.class_name,
                );
                state.static_css.push_str(&local_out);

                if decl.semi_token.is_some() {
                    state.static_css.push_str("; ");
                }
            }
            CssRule::Nested(nested) => {
                let has_dynamic_sel = contains_dynamic_selector(&nested.selectors);
                if has_dynamic_sel {
                    let mut template = String::new();
                    let mut selector_exprs = Vec::new();

                    extract_dynamic_selector(
                        &nested.selectors,
                        &mut template,
                        &mut selector_exprs,
                        &mut state.theme_refs,
                        &state.class_name,
                    );
                    template.push_str(" { ");
                    build_dynamic_block(
                        &nested.block,
                        &mut template,
                        &mut selector_exprs,
                        &mut state.expressions,
                        &mut state.theme_refs,
                        &state.class_name,
                    );
                    template.push_str(" }");

                    state.dynamic_rules.push(DynamicRule {
                        template,
                        expressions: selector_exprs,
                    });
                } else {
                    let mut sel_str = String::new();
                    build_static_selector(&nested.selectors, &mut sel_str, &state.class_name);
                    state.static_css.push_str(&sel_str);
                    state.static_css.push_str(" { ");
                    process_css_block(&nested.block, state)?;
                    state.static_css.push_str(" } ");
                }
            }
            CssRule::AtRule(at) => {
                state.static_css.push('@');
                state.static_css.push_str(&at.name.to_string());
                state.static_css.push(' ');
                let ts_str = append_token_stream_strings(&at.params);
                state.static_css.push_str(&ts_str);
                state.static_css.push_str(" { ");
                process_css_block(&at.block, state)?;
                state.static_css.push_str(" } ");
            }
        }
    }
    Ok(())
}

fn build_dynamic_block(
    block: &CssBlock,
    template: &mut String,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    class_name: &str,
) {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                template.push_str(&decl.property);
                template.push_str(": ");

                template.push(' ');
                extract_dynamic_value(
                    &decl.values,
                    template,
                    global_expressions,
                    theme_refs,
                    &decl.property,
                    class_name,
                );

                if decl.semi_token.is_some() {
                    template.push_str("; ");
                }
            }
            CssRule::Nested(nested) => {
                extract_dynamic_selector(
                    &nested.selectors,
                    template,
                    selector_exprs,
                    theme_refs,
                    "",
                );
                template.push_str(" { ");
                build_dynamic_block(
                    &nested.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    theme_refs,
                    class_name,
                );
                template.push_str(" } ");
            }
            CssRule::AtRule(at) => {
                template.push('@');
                template.push_str(&at.name.to_string());
                template.push(' ');
                template.push_str(&append_token_stream_strings(&at.params));
                template.push_str(" { ");
                build_dynamic_block(
                    &at.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    theme_refs,
                    class_name,
                );
                template.push_str(" } ");
            }
        }
    }
}

fn contains_dynamic_selector(ts: &TokenStream) -> bool {
    let mut iter = ts.clone().into_iter().peekable();
    while let Some(tt) = iter.next() {
        if let TokenTree::Punct(p) = &tt
            && p.as_char() == '$'
        {
            if let Some(TokenTree::Group(g)) = iter.peek()
                && g.delimiter() == Delimiter::Parenthesis
            {
                return true;
            }
            let mut sub = iter.clone();
            if let Some(TokenTree::Ident(id)) = sub.next()
                && id == "theme"
            {
                return true;
            }
        }
    }
    false
}

fn append_token_stream_strings(ts: &TokenStream) -> String {
    let mut out = String::new();
    let iter = ts.clone().into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;
    for tt in iter {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_))
                | (TokenTree::Ident(_), TokenTree::Literal(_))
                | (TokenTree::Literal(_), TokenTree::Ident(_))
                | (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }
        if space_before {
            out.push(' ');
        }
        match tt {
            TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                out.push_str(&append_token_stream_strings(&g.stream()));
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
    out
}

fn build_static_selector(ts: &TokenStream, out: &mut String, class_name: &str) {
    let iter = ts.clone().into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;

    for tt in iter {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_))
                | (TokenTree::Ident(_), TokenTree::Literal(_))
                | (TokenTree::Literal(_), TokenTree::Ident(_))
                | (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }

        if let TokenTree::Punct(ref p) = tt
            && p.as_char() == '&'
        {
            if space_before {
                out.push(' ');
            }
            out.push_str(&format!(".{}", class_name));
            prev_tt = Some(TokenTree::Ident(proc_macro2::Ident::new(
                "dummy",
                Span::call_site(),
            )));
            continue;
        }

        if space_before {
            out.push(' ');
        }

        match tt {
            TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                out.push_str(&append_token_stream_strings(&g.stream()));
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
}

fn extract_dynamic_selector(
    ts: &TokenStream,
    out: &mut String,
    exprs: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    class_name: &str,
) {
    let mut iter = ts.clone().into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;

    while let Some(tt) = iter.next() {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_))
                | (TokenTree::Ident(_), TokenTree::Literal(_))
                | (TokenTree::Literal(_), TokenTree::Ident(_))
                | (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }

        if let TokenTree::Punct(ref p) = tt {
            if p.as_char() == '$' {
                if let Some(TokenTree::Group(g)) = iter.peek()
                    && g.delimiter() == Delimiter::Parenthesis
                {
                    if space_before {
                        out.push(' ');
                    }
                    out.push_str("{}");
                    exprs.push(("any".to_string(), g.stream()));
                    prev_tt = Some(iter.next().unwrap());
                    continue;
                }

                // Theme support in selectors: $theme.key or $theme.a.b
                let mut sub_iter = iter.clone();
                if let Some(TokenTree::Ident(id)) = sub_iter.next()
                    && id == "theme"
                    && let Some(TokenTree::Punct(dot)) = sub_iter.next()
                    && dot.as_char() == '.'
                {
                    let mut path = Vec::new();
                    // First segment is mandatory
                    if let Some(TokenTree::Ident(key)) = sub_iter.next() {
                        path.push(key.to_string());
                        // Continue parsing dots and idents
                        while let Some(dot_peek) = sub_iter.peek()
                            && let TokenTree::Punct(p) = dot_peek
                            && p.as_char() == '.'
                        {
                            sub_iter.next(); // consume dot
                            if let Some(TokenTree::Ident(id)) = sub_iter.next() {
                                path.push(id.to_string());
                            } else {
                                break;
                            }
                        }

                        iter = sub_iter;
                        if space_before {
                            out.push(' ');
                        }
                        let joined_key = path.join("-");
                        out.push_str(&format!("var(--slx-theme-{})", joined_key));
                        theme_refs.push(("any".to_string(), path.join(".")));
                        prev_tt = Some(TokenTree::Ident(proc_macro2::Ident::new(
                            "dummy",
                            Span::call_site(),
                        )));
                        continue;
                    }
                }
            } else if p.as_char() == '&' && !class_name.is_empty() {
                if space_before {
                    out.push(' ');
                }
                out.push_str(&format!(".{}", class_name));
                prev_tt = Some(TokenTree::Ident(proc_macro2::Ident::new(
                    "dummy",
                    Span::call_site(),
                )));
                continue;
            }
        }

        if space_before {
            out.push(' ');
        }

        match tt {
            TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                extract_dynamic_selector(&g.stream(), out, exprs, theme_refs, class_name);
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
}

fn extract_dynamic_value(
    ts: &TokenStream,
    out: &mut String,
    exprs: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    prop_name: &str,
    class_name: &str,
) {
    let mut iter = ts.clone().into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;

    while let Some(tt) = iter.next() {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_))
                | (TokenTree::Ident(_), TokenTree::Literal(_))
                | (TokenTree::Literal(_), TokenTree::Ident(_))
                | (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }

        if let TokenTree::Punct(ref p) = tt
            && p.as_char() == '$'
        {
            if let Some(TokenTree::Group(g)) = iter.peek()
                && g.delimiter() == Delimiter::Parenthesis
            {
                if space_before {
                    out.push(' ');
                }
                let idx = exprs.len();
                exprs.push((prop_name.to_string(), g.stream()));
                use std::fmt::Write;
                if !class_name.is_empty() {
                    let _ = write!(out, "var(--{}-{})", class_name, idx);
                } else {
                    let _ = write!(out, "var(--dyn-{})", idx);
                }

                prev_tt = Some(iter.next().unwrap());
                continue;
            }

            // Theme support: $theme.key or $theme.a.b
            let mut sub_iter = iter.clone();
            if let Some(TokenTree::Ident(id)) = sub_iter.next()
                && id == "theme"
                && let Some(TokenTree::Punct(dot)) = sub_iter.next()
                && dot.as_char() == '.'
            {
                let mut path = Vec::new();
                if let Some(TokenTree::Ident(key)) = sub_iter.next() {
                    path.push(key.to_string());
                    while let Some(dot_peek) = sub_iter.peek()
                        && let TokenTree::Punct(p) = dot_peek
                        && p.as_char() == '.'
                    {
                        sub_iter.next();
                        if let Some(TokenTree::Ident(id)) = sub_iter.next() {
                            path.push(id.to_string());
                        } else {
                            break;
                        }
                    }

                    iter = sub_iter;
                    if space_before {
                        out.push(' ');
                    }
                    let joined_key = path.join("-");
                    use std::fmt::Write;
                    let _ = write!(out, "var(--slx-theme-{})", joined_key);
                    theme_refs.push((prop_name.to_string(), path.join(".")));
                    prev_tt = Some(TokenTree::Ident(proc_macro2::Ident::new(
                        "dummy",
                        Span::call_site(),
                    )));
                    continue;
                }
            }
        }

        if space_before {
            out.push(' ');
        }

        match tt {
            TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                extract_dynamic_value(&g.stream(), out, exprs, theme_refs, prop_name, class_name);
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&lit.to_string());
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
}
