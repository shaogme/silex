use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::{Delimiter, Group, Span, TokenStream, TokenTree};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::Result;

pub struct DynamicRule {
    pub template: String,
    pub expressions: Vec<TokenStream>,
}

pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub final_css: String,
    pub expressions: Vec<TokenStream>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub hash: u64,
}

struct ParserState {
    static_css: String,
    expressions: Vec<TokenStream>,
    dynamic_rules: Vec<DynamicRule>,
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
            class_name: class_name.clone(),
        };

        parse_css_ast(ts, &mut state)?;

        let mut final_source_css = state.static_css.clone();
        for i in 0..state.expressions.len() {
            let placeholder = format!("--slx-tmp-{}", i);
            let real_var = format!("--slx-{:x}-{}", hash, i);
            final_source_css = final_source_css.replace(&placeholder, &real_var);
        }

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
            hash,
        })
    }
}

fn parse_css_ast(ts: TokenStream, state: &mut ParserState) -> Result<()> {
    let mut buffer = Vec::new();
    let iter = ts.into_iter().peekable();

    for tt in iter {
        match &tt {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                let mut is_dynamic = false;
                let mut i = 0;
                while i < buffer.len() {
                    if let TokenTree::Punct(p) = &buffer[i]
                        && p.as_char() == '$'
                        && i + 1 < buffer.len()
                        && let TokenTree::Group(ig) = &buffer[i + 1]
                        && ig.delimiter() == Delimiter::Parenthesis
                    {
                        is_dynamic = true;
                        break;
                    }
                    i += 1;
                }

                if is_dynamic {
                    let (template, exprs) = build_dynamic_template(&buffer, g, &state.class_name)?;
                    state.dynamic_rules.push(DynamicRule {
                        template,
                        expressions: exprs,
                    });
                    buffer.clear();
                } else {
                    let ts_buf: TokenStream = buffer.drain(..).collect();
                    let prelude_str = process_static_property_stream(ts_buf, state)?;
                    state.static_css.push_str(&prelude_str);
                    state.static_css.push_str(" { ");
                    parse_css_ast(g.stream(), state)?;
                    state.static_css.push_str(" } ");
                }
            }
            TokenTree::Punct(p) if p.as_char() == ';' => {
                buffer.push(tt);
                let ts_buf: TokenStream = buffer.drain(..).collect();
                let prop_str = process_static_property_stream(ts_buf, state)?;
                state.static_css.push_str(&prop_str);
            }
            _ => {
                buffer.push(tt);
            }
        }
    }

    if !buffer.is_empty() {
        let ts_buf: TokenStream = buffer.drain(..).collect();
        let prop_str = process_static_property_stream(ts_buf, state)?;
        state.static_css.push_str(&prop_str);
    }

    Ok(())
}

fn process_static_property_stream(ts: TokenStream, state: &mut ParserState) -> Result<String> {
    let mut out = String::new();
    let mut iter = ts.into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;

    while let Some(tt) = iter.next() {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_)) => space_before = true,
                (TokenTree::Ident(_), TokenTree::Literal(_)) => space_before = true,
                (TokenTree::Literal(_), TokenTree::Ident(_)) => space_before = true,
                (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }

        if let TokenTree::Punct(ref p) = tt
            && p.as_char() == '$'
            && let Some(TokenTree::Group(g)) = iter.peek()
            && g.delimiter() == Delimiter::Parenthesis
        {
            if space_before {
                out.push(' ');
            }
            let expr_ts = g.stream();
            let idx = state.expressions.len();
            state.expressions.push(expr_ts);
            out.push_str(&format!("var(--slx-tmp-{})", idx));

            prev_tt = Some(iter.next().unwrap()); // consume group
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
                out.push_str(&process_static_property_stream(g.stream(), state)?);
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
    Ok(out)
}

fn build_dynamic_template(
    prelude: &[TokenTree],
    body: &Group,
    class_name: &str,
) -> Result<(String, Vec<TokenStream>)> {
    let mut template = String::new();
    let mut exprs = Vec::new();

    let mut has_ampersand = false;
    let mut is_at_rule = false;

    for tt in prelude {
        if let TokenTree::Punct(p) = tt {
            if p.as_char() == '&' {
                has_ampersand = true;
            } else if p.as_char() == '@' {
                is_at_rule = true;
            }
        }
    }

    if !is_at_rule && !has_ampersand {
        template.push_str(&format!(".{} ", class_name));
    }

    let prelude_stream: TokenStream = prelude.iter().cloned().collect();
    extract_dynamic_template(
        prelude_stream,
        &mut template,
        &mut exprs,
        class_name,
        is_at_rule,
    );

    template.push_str(" { ");
    extract_dynamic_template(body.stream(), &mut template, &mut exprs, class_name, false);
    template.push_str(" } ");

    Ok((template, exprs))
}

fn extract_dynamic_template(
    ts: TokenStream,
    out: &mut String,
    exprs: &mut Vec<TokenStream>,
    class_name: &str,
    is_at_rule: bool,
) {
    let mut iter = ts.into_iter().peekable();
    let mut prev_tt: Option<TokenTree> = None;

    while let Some(tt) = iter.next() {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_)) => space_before = true,
                (TokenTree::Ident(_), TokenTree::Literal(_)) => space_before = true,
                (TokenTree::Literal(_), TokenTree::Ident(_)) => space_before = true,
                (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
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
                    exprs.push(g.stream());
                    prev_tt = Some(iter.next().unwrap());
                    continue;
                }
            } else if p.as_char() == '&' && !is_at_rule {
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
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                extract_dynamic_template(g.stream(), out, exprs, class_name, is_at_rule);
                out.push_str(close);
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
