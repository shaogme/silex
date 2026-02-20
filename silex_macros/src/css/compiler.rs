use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::{Span, TokenStream};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::Result;

pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub final_css: String,
    pub expressions: Vec<TokenStream>,
    pub hash: u64,
}

pub struct CssCompiler;

impl CssCompiler {
    pub fn compile(css_content: &str, span: Span) -> Result<CssCompileResult> {
        let mut pre_css = String::new();
        let mut expressions = Vec::new();

        let mut chars = css_content.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '$' {
                if let Some(&'(') = chars.peek() {
                    chars.next(); // consume '('

                    let mut expr_str = String::new();
                    let mut depth = 1;
                    for inner_c in chars.by_ref() {
                        if inner_c == '(' {
                            depth += 1;
                        } else if inner_c == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        expr_str.push(inner_c);
                    }

                    if depth != 0 {
                        return Err(syn::Error::new(
                            span,
                            "Unbalanced parentheses in interpolation",
                        ));
                    }

                    let index = expressions.len();
                    let expr: TokenStream = syn::parse_str(&expr_str).map_err(|e| {
                        syn::Error::new(span, format!("Invalid expression in interpolation: {}", e))
                    })?;
                    expressions.push(expr);

                    pre_css.push_str(&format!("var(--slx-tmp-{})", index));
                } else {
                    pre_css.push('$');
                }
            } else {
                pre_css.push(c);
            }
        }

        let mut hasher = DefaultHasher::new();
        pre_css.hash(&mut hasher);
        let hash = hasher.finish();
        let class_name = format!("slx-{:x}", hash);
        let style_id = format!("style-{}", class_name);

        let mut final_source_css = pre_css.clone();
        for i in 0..expressions.len() {
            let placeholder = format!("--slx-tmp-{}", i);
            let real_var = format!("--slx-{:x}-{}", hash, i);
            final_source_css = final_source_css.replace(&placeholder, &real_var);
        }

        let wrapped_css = format!(".{} {{ {} }}", class_name, final_source_css);

        let mut stylesheet = StyleSheet::parse(&wrapped_css, ParserOptions::default())
            .map_err(|e| syn::Error::new(span, format!("Invalid CSS: {}", e)))?;

        stylesheet
            .minify(MinifyOptions::default())
            .map_err(|e| syn::Error::new(span, format!("CSS Minification failed: {}", e)))?;

        let res = stylesheet
            .to_css(PrinterOptions {
                minify: true,
                targets: Targets::default(),
                ..PrinterOptions::default()
            })
            .map_err(|e| syn::Error::new(span, format!("CSS Printing failed: {}", e)))?;

        Ok(CssCompileResult {
            class_name,
            style_id,
            final_css: res.code,
            expressions,
            hash,
        })
    }
}
