use crate::css::ast::{CssBlock, CssRule};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::token_stream::IntoIter;
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;
use syn::Result;

pub struct DynamicRule {
    pub template: String,
    pub expressions: Vec<(String, TokenStream)>,
}

pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub static_id: String,
    pub static_css: String,    // Fully static CSS (font-face, etc.)
    pub component_css: String, // CSS scoped to this component (with dynamic vars)
    pub expressions: Vec<(String, TokenStream)>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub theme_refs: Vec<(String, String)>,
}

struct ParserState {
    static_css: String,
    lifted_css: String,
    expressions: Vec<(String, TokenStream)>,
    dynamic_rules: Vec<DynamicRule>,
    theme_refs: Vec<(String, String)>,
    class_name: String,
    theme_prefix: String,
    is_unsafe: bool,
}

#[derive(Clone, Copy)]
struct DynamicContext<'a> {
    class_name: &'a str,
    theme_prefix: &'a str,
    is_unsafe: bool,
}

pub struct CssCompiler;

impl CssCompiler {
    pub fn compile(
        ts: TokenStream,
        span: Span,
        theme_prefix: Option<String>,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        Self::compile_internal(ts, span, theme_prefix, true, is_unsafe)
    }

    pub fn compile_global(
        ts: TokenStream,
        span: Span,
        theme_prefix: Option<String>,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        Self::compile_internal(ts, span, theme_prefix, false, is_unsafe)
    }

    fn compile_internal(
        ts: TokenStream,
        span: Span,
        theme_prefix: Option<String>,
        wrap_in_class: bool,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        let ts_string = ts.to_string();
        let hash = silex_hash::css::hash_one(&ts_string);
        let mut buf = [0u8; 13];
        let class_base = silex_hash::css::encode_base36(hash, &mut buf);
        let class_name = format!("slx-{}", class_base);
        let style_id = format!("style-{}", class_name);

        let mut state = ParserState {
            static_css: String::new(),
            lifted_css: String::new(),
            expressions: Vec::new(),
            dynamic_rules: Vec::new(),
            theme_refs: Vec::new(),
            class_name: if wrap_in_class {
                class_name.clone()
            } else {
                "".to_string()
            },
            theme_prefix: theme_prefix.unwrap_or_else(|| "slx-theme".to_string()),
            is_unsafe,
        };

        let block: CssBlock = syn::parse2(ts)?;
        process_css_block(&block, &mut state)?;

        let final_static_css = if state.lifted_css.is_empty() {
            "".to_string()
        } else {
            let mut stylesheet = StyleSheet::parse(&state.lifted_css, ParserOptions::default())
                .map_err(|e| {
                    crate::css::error::report_lightning_error(format!("Static CSS: {}", e), span)
                })?;
            stylesheet.minify(MinifyOptions::default()).ok();
            stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets: Targets::default(),
                    ..PrinterOptions::default()
                })
                .map_err(|e| {
                    crate::css::error::report_lightning_error(
                        format!("Static CSS Printing: {}", e),
                        span,
                    )
                })?
                .code
        };

        let final_component_css = if wrap_in_class && !state.static_css.trim().is_empty() {
            let wrapped = format!(".{} {{ {} }}", class_name, state.static_css);
            let mut stylesheet =
                StyleSheet::parse(&wrapped, ParserOptions::default()).map_err(|e| {
                    crate::css::error::report_lightning_error(format!("Component CSS: {}", e), span)
                })?;
            stylesheet.minify(MinifyOptions::default()).ok();
            stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets: Targets::default(),
                    ..PrinterOptions::default()
                })
                .map_err(|e| {
                    crate::css::error::report_lightning_error(
                        format!("Component CSS Printing: {}", e),
                        span,
                    )
                })?
                .code
        } else if !wrap_in_class && !state.static_css.trim().is_empty() {
            state.static_css.clone()
        } else {
            "".to_string()
        };

        let static_id = if !final_static_css.is_empty() {
            format!("static-{}", silex_hash::css::hash_one(&final_static_css))
        } else {
            "".to_string()
        };

        Ok(CssCompileResult {
            class_name,
            style_id,
            static_id,
            static_css: final_static_css,
            component_css: final_component_css,
            expressions: state.expressions,
            dynamic_rules: state.dynamic_rules,
            theme_refs: state.theme_refs,
        })
    }
}

fn process_css_block(block: &CssBlock, state: &mut ParserState) -> Result<()> {
    for rule in &block.rules {
        let ctx = DynamicContext {
            class_name: &state.class_name,
            theme_prefix: &state.theme_prefix,
            is_unsafe: state.is_unsafe,
        };
        match rule {
            CssRule::Declaration(decl) => {
                state.static_css.push_str(&decl.property);
                state.static_css.push_str(": ");

                let prop_for_expr = if state.is_unsafe {
                    "any"
                } else {
                    &decl.property
                };
                let val = extract_dynamic_value(
                    &decl.values,
                    &mut state.expressions,
                    &mut state.theme_refs,
                    prop_for_expr,
                    &ctx,
                );
                state.static_css.push_str(&val);

                if decl.semi_token.is_some() {
                    state.static_css.push_str("; ");
                }
            }
            CssRule::Unsafe(u) => {
                let old = state.is_unsafe;
                state.is_unsafe = true;
                process_css_block(&u.block, state)?;
                state.is_unsafe = old;
            }
            CssRule::Nested(nested) => {
                if contains_dynamic_selector(&nested.selectors) {
                    let mut selector_exprs = Vec::new();
                    let template = build_dynamic_template(
                        nested,
                        &mut selector_exprs,
                        &mut state.expressions,
                        &mut state.theme_refs,
                        &DynamicContext {
                            is_unsafe: false,
                            ..ctx
                        },
                    );
                    state.dynamic_rules.push(DynamicRule {
                        template,
                        expressions: selector_exprs,
                    });
                } else {
                    let sel_str = build_static_selector(&nested.selectors, &state.class_name);
                    state.static_css.push_str(&sel_str);
                    state.static_css.push_str(" { ");
                    process_css_block(&nested.block, state)?;
                    state.static_css.push_str(" } ");
                }
            }
            CssRule::AtRule(at) => {
                let is_lifted =
                    (at.name == "keyframes" || at.name == "font-face" || at.name == "import")
                        && !state.class_name.is_empty();

                let params =
                    extract_at_rule_params(&at.params, &mut state.theme_refs, &state.theme_prefix);

                let mut rule_str = String::new();
                rule_str.push('@');
                rule_str.push_str(&at.name.to_string());
                rule_str.push(' ');
                rule_str.push_str(&params);
                rule_str.push_str(" { ");

                // For nested rules inside @keyframes, we shouldn't use the class name.
                // We create a temporary state with empty class_name for the inner block.
                let mut inner_state = ParserState {
                    static_css: String::new(),
                    lifted_css: String::new(),
                    expressions: state.expressions.clone(),
                    dynamic_rules: Vec::new(),
                    theme_refs: Vec::new(),
                    class_name: if at.name == "keyframes" {
                        "".to_string()
                    } else {
                        state.class_name.clone()
                    },
                    theme_prefix: state.theme_prefix.clone(),
                    is_unsafe: state.is_unsafe,
                };

                process_css_block(&at.block, &mut inner_state)?;
                rule_str.push_str(&inner_state.static_css);
                rule_str.push_str(" } ");

                // Sync back state
                state.expressions = inner_state.expressions;
                for tr in inner_state.theme_refs {
                    state.theme_refs.push(tr);
                }
                // Dynamic rules inside @-rules is not fully supported yet in this implementation,
                // but we should probably collect them anyway.
                for dr in inner_state.dynamic_rules {
                    state.dynamic_rules.push(dr);
                }

                if is_lifted {
                    state.lifted_css.push_str(&rule_str);
                    state.lifted_css.push('\n');
                } else {
                    state.static_css.push_str(&rule_str);
                }
            }
        }
    }
    Ok(())
}

fn build_dynamic_template(
    nested: &crate::css::ast::CssNested,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    ctx: &DynamicContext,
) -> String {
    let mut template = extract_dynamic_selector(&nested.selectors, selector_exprs, theme_refs, ctx);
    template.push_str(" { ");
    build_dynamic_block_recursive(
        &nested.block,
        &mut template,
        selector_exprs,
        global_expressions,
        theme_refs,
        ctx,
    );
    template.push_str(" }");
    template
}

fn build_dynamic_block_recursive(
    block: &CssBlock,
    template: &mut String,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    ctx: &DynamicContext,
) {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                template.push_str(&decl.property);
                template.push_str(": ");
                let prop_for_expr = if ctx.is_unsafe { "any" } else { &decl.property };
                let val = extract_dynamic_value(
                    &decl.values,
                    global_expressions,
                    theme_refs,
                    prop_for_expr,
                    ctx,
                );
                template.push_str(&val);
                if decl.semi_token.is_some() {
                    template.push_str("; ");
                }
            }
            CssRule::Nested(nested) => {
                let sel = extract_dynamic_selector(
                    &nested.selectors,
                    selector_exprs,
                    theme_refs,
                    &DynamicContext {
                        class_name: "",
                        ..*ctx
                    },
                );
                template.push_str(&sel);
                template.push_str(" { ");
                build_dynamic_block_recursive(
                    &nested.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    theme_refs,
                    ctx,
                );
                template.push_str(" } ");
            }
            CssRule::AtRule(at) => {
                template.push('@');
                template.push_str(&at.name.to_string());
                template.push(' ');
                template.push_str(&append_token_stream_strings(&at.params));
                template.push_str(" { ");
                build_dynamic_block_recursive(
                    &at.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    theme_refs,
                    ctx,
                );
                template.push_str(" } ");
            }
            CssRule::Unsafe(u) => {
                build_dynamic_block_recursive(
                    &u.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    theme_refs,
                    &DynamicContext {
                        is_unsafe: true,
                        ..*ctx
                    },
                );
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

// --- Unified Token Stream Processing ---

fn process_tokens<F>(ts: &TokenStream, handler: &mut F) -> String
where
    F: FnMut(&TokenTree, &mut Peekable<IntoIter>, &mut String, bool) -> bool,
{
    let mut iter = ts.clone().into_iter().peekable();
    process_tokens_iter(&mut iter, handler)
}

fn process_tokens_iter<F>(iter: &mut Peekable<IntoIter>, handler: &mut F) -> String
where
    F: FnMut(&TokenTree, &mut Peekable<IntoIter>, &mut String, bool) -> bool,
{
    let mut out = String::new();
    let mut prev_tt: Option<TokenTree> = None;

    while let Some(tt) = iter.next() {
        let mut space_before = false;
        if let Some(prev) = &prev_tt {
            match (prev, &tt) {
                (TokenTree::Ident(_), TokenTree::Ident(_))
                | (TokenTree::Ident(_), TokenTree::Literal(_))
                | (TokenTree::Literal(_), TokenTree::Literal(_)) => space_before = true,
                _ => {}
            }
        }

        if handler(&tt, iter, &mut out, space_before) {
            prev_tt = Some(tt);
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
                let mut sub_iter = g.stream().into_iter().peekable();
                out.push_str(&process_tokens_iter(&mut sub_iter, handler));
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
                let s = lit.to_string();
                if s.starts_with('"') && s.ends_with('"') {
                    out.push_str(&s[1..s.len() - 1]);
                } else {
                    out.push_str(&s);
                }
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
    out
}

fn handle_theme_path(
    iter: &mut Peekable<IntoIter>,
    theme_prefix: &str,
    theme_refs: &mut Vec<(String, String)>,
    prop_name: &str,
) -> Option<String> {
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
            *iter = sub_iter;
            let joined_path = path.join(".");
            theme_refs.push((prop_name.to_string(), joined_path.clone()));
            return Some(format!("var(--{}-{})", theme_prefix, path.join("-")));
        }
    }
    None
}

pub fn append_token_stream_strings(ts: &TokenStream) -> String {
    // Basic version used for @-rules and such, no special $ or & handling
    process_tokens(ts, &mut |_, _, _, _| false)
}

fn extract_at_rule_params(
    ts: &TokenStream,
    theme_refs: &mut Vec<(String, String)>,
    theme_prefix: &str,
) -> String {
    process_tokens(ts, &mut |tt, iter, out, space_before| {
        if matches!(tt, TokenTree::Punct(p) if p.as_char() == '$')
            && let Some(var) = handle_theme_path(iter, theme_prefix, theme_refs, "at-rule")
        {
            if space_before {
                out.push(' ');
            }
            out.push_str(&var);
            return true;
        }
        false
    })
}

fn build_static_selector(ts: &TokenStream, class_name: &str) -> String {
    process_tokens(ts, &mut |tt, _, out, space_before| {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == '&'
            && !class_name.is_empty()
        {
            if space_before {
                out.push(' ');
            }
            out.push_str(&format!(".{}", class_name));
            return true;
        }
        false
    })
}

fn extract_dynamic_selector(
    ts: &TokenStream,
    exprs: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    ctx: &DynamicContext,
) -> String {
    process_tokens(ts, &mut |tt, iter, out, space_before| {
        if let TokenTree::Punct(p) = tt {
            if p.as_char() == '$' {
                if let Some(TokenTree::Group(g)) = iter.peek()
                    && g.delimiter() == Delimiter::Parenthesis
                {
                    if space_before {
                        out.push(' ');
                    }
                    out.push_str("{}");
                    exprs.push(("any".to_string(), g.stream()));
                    iter.next();
                    return true;
                }
                if let Some(var) = handle_theme_path(iter, ctx.theme_prefix, theme_refs, "any") {
                    if space_before {
                        out.push(' ');
                    }
                    out.push_str(&var);
                    return true;
                }
            } else if p.as_char() == '&' && !ctx.class_name.is_empty() {
                if space_before {
                    out.push(' ');
                }
                out.push_str(&format!(".{}", ctx.class_name));
                return true;
            }
        }
        false
    })
}

fn extract_dynamic_value(
    ts: &TokenStream,
    exprs: &mut Vec<(String, TokenStream)>,
    theme_refs: &mut Vec<(String, String)>,
    prop_name: &str,
    ctx: &DynamicContext,
) -> String {
    process_tokens(ts, &mut |tt, iter, out, space_before| {
        if let TokenTree::Punct(p) = tt
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
                if !ctx.class_name.is_empty() {
                    out.push_str(&format!("var(--{}-{})", ctx.class_name, idx));
                } else {
                    out.push_str(&format!("var(--dyn-{})", idx));
                }
                iter.next();
                return true;
            }
            if let Some(var) = handle_theme_path(iter, ctx.theme_prefix, theme_refs, prop_name) {
                if space_before {
                    out.push(' ');
                }
                out.push_str(&var);
                return true;
            }
        }
        false
    })
}
