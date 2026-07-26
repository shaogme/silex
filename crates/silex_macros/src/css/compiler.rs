use crate::css::ast::{CssBlock, CssRule};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::token_stream::IntoIter;
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;
use std::ops::Range;
use std::rc::Rc;
use syn::Result;

#[derive(Debug, Clone)]
pub struct DynamicRule {
    pub template: String,
    pub expressions: Vec<(String, TokenStream)>,
}

#[derive(Debug, Clone)]
pub struct CssWarning {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CssCompileResult {
    pub class_name: String,
    pub style_id: String,
    pub static_id: String,
    pub static_css: String,    // Fully static CSS (font-face, etc.)
    pub component_css: String, // CSS scoped to this component (with dynamic vars)
    pub expressions: Vec<(String, TokenStream)>,
    pub dynamic_rules: Vec<DynamicRule>,
    pub warnings: Vec<CssWarning>,
}

impl CssCompileResult {
    /// Generates TokenStream for injecting static and component CSS styles
    pub fn generate_inits(&self) -> TokenStream {
        let __silex = crate::crate_path::silex();
        let static_id = &self.static_id;
        let static_css = &self.static_css;
        let style_id = &self.style_id;
        let component_css = &self.component_css;

        quote::quote! {
            if !#static_css.is_empty() {
                #__silex::css::inject_style(#static_id, #static_css);
            }
            if !#component_css.is_empty() {
                #__silex::css::inject_style(#style_id, #component_css);
            }
        }
    }
}

struct ParserState {
    static_css: String,
    lifted_css: String,
    expressions: Vec<(String, TokenStream)>,
    dynamic_rules: Vec<DynamicRule>,
    warnings: Vec<CssWarning>,
    class_name: String,
    is_unsafe: bool,
    /// 整个宏调用的源码，用于恢复 token 之间的空白（见 [`crate::css::spacing`]）
    region: Option<Rc<str>>,
}

#[derive(Clone)]
struct DynamicContext<'a> {
    class_name: &'a str,
    is_unsafe: bool,
    region: Option<Rc<str>>,
}

pub struct CssCompiler;

impl CssCompiler {
    pub fn compile_block(
        block: &CssBlock,
        span: Span,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        Self::compile_block_with_prefix(block, span, is_unsafe, "slx-tw-")
    }

    pub fn compile_block_with_prefix(
        block: &CssBlock,
        span: Span,
        is_unsafe: bool,
        prefix: &str,
    ) -> Result<CssCompileResult> {
        let ts_string = quote::quote!(#block).to_string();
        Self::compile_block_internal(
            block,
            ts_string,
            span,
            true,
            is_unsafe,
            prefix,
            macro_region(),
        )
    }

    pub fn compile(ts: TokenStream, span: Span, is_unsafe: bool) -> Result<CssCompileResult> {
        Self::compile_with_prefix(ts, span, is_unsafe, "slx-tw-")
    }

    /// 用显式给定的原文编译。
    ///
    /// 真实宏展开时原文来自 `Span::call_site().source_text()`；单元测试里
    /// token 是 `parse_str` 出来的、没有调用点可言，于是把源码直接递进来，
    /// 让测试与生产走同一条空白恢复路径。
    #[cfg(test)]
    pub fn compile_with_source(
        source: &str,
        span: Span,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        let ts: TokenStream = source.parse().map_err(|e| syn::Error::new(span, e))?;
        let block: CssBlock = syn::parse2(ts.clone())?;
        Self::compile_block_internal(
            &block,
            ts.to_string(),
            span,
            true,
            is_unsafe,
            "slx-tw-",
            Some(Rc::from(source)),
        )
    }

    /// 同上，但走全局模式（不包 `.class { }`）。
    #[cfg(test)]
    pub fn compile_global_with_source(
        source: &str,
        span: Span,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        let ts: TokenStream = source.parse().map_err(|e| syn::Error::new(span, e))?;
        let block: CssBlock = syn::parse2(ts.clone())?;
        Self::compile_block_internal(
            &block,
            ts.to_string(),
            span,
            false,
            is_unsafe,
            "slx-",
            Some(Rc::from(source)),
        )
    }

    pub fn compile_with_prefix(
        ts: TokenStream,
        span: Span,
        is_unsafe: bool,
        prefix: &str,
    ) -> Result<CssCompileResult> {
        let block: CssBlock = syn::parse2(ts.clone())?;
        Self::compile_block_internal(
            &block,
            ts.to_string(),
            span,
            true,
            is_unsafe,
            prefix,
            macro_region(),
        )
    }

    pub fn compile_global(
        ts: TokenStream,
        span: Span,
        is_unsafe: bool,
    ) -> Result<CssCompileResult> {
        let block: CssBlock = syn::parse2(ts.clone())?;
        Self::compile_block_internal(
            &block,
            ts.to_string(),
            span,
            false,
            is_unsafe,
            "slx-",
            macro_region(),
        )
    }

    fn compile_block_internal(
        block: &CssBlock,
        ts_string: String,
        span: Span,
        wrap_in_class: bool,
        is_unsafe: bool,
        prefix: &str,
        region: Option<Rc<str>>,
    ) -> Result<CssCompileResult> {
        let hash = silex_hash::css::hash_one(&ts_string);
        let mut buf = [0u8; 13];
        let class_base = silex_hash::css::encode_base36(hash, &mut buf);
        let class_name = format!("{}{}", prefix, class_base);
        let style_id = format!("style-{}", class_name);

        let mut state = ParserState {
            static_css: String::new(),
            lifted_css: String::new(),
            expressions: Vec::new(),
            dynamic_rules: Vec::new(),
            warnings: Vec::new(),
            class_name: if wrap_in_class {
                class_name.clone()
            } else {
                "".to_string()
            },
            is_unsafe,
            region,
        };

        process_css_block(block, &mut state)?;

        let final_static_css = if state.lifted_css.is_empty() {
            "".to_string()
        } else {
            let mut stylesheet = StyleSheet::parse(&state.lifted_css, ParserOptions::default())
                .map_err(|e| {
                    crate::css::error::report_lightning_error(format!("Static CSS: {}", e), span)
                })?;
            stylesheet.minify(MinifyOptions::default()).map_err(|e| {
                crate::css::error::report_lightning_error(format!("Static CSS Minify: {}", e), span)
            })?;
            stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets: get_compiler_targets(),
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
            let layer_name = match prefix {
                "slx-twv-" | "slx-st-" => "components",
                _ => "utilities",
            };
            let wrapped = format!(
                "@layer {} {{ .{} {{ {} }} }}",
                layer_name, class_name, state.static_css
            );
            let mut stylesheet =
                StyleSheet::parse(&wrapped, ParserOptions::default()).map_err(|e| {
                    crate::css::error::report_lightning_error(format!("Component CSS: {}", e), span)
                })?;
            stylesheet.minify(MinifyOptions::default()).map_err(|e| {
                crate::css::error::report_lightning_error(
                    format!("Component CSS Minify: {}", e),
                    span,
                )
            })?;
            stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    targets: get_compiler_targets(),
                    ..Default::default()
                })
                .map_err(|e| {
                    crate::css::error::report_lightning_error(
                        format!("Component CSS Printing: {}", e),
                        span,
                    )
                })?
                .code
        } else if !wrap_in_class && !state.static_css.trim().is_empty() {
            // Run global styles through lightningcss for consistency (flattens nesting, minifies)
            match StyleSheet::parse(&state.static_css, ParserOptions::default()) {
                Ok(stylesheet) => stylesheet
                    .to_css(PrinterOptions {
                        minify: true,
                        ..Default::default()
                    })
                    .map(|o| o.code)
                    .map_err(|e| {
                        crate::css::error::report_lightning_error(
                            format!("Global CSS Printing: {}", e),
                            span,
                        )
                    })?,
                Err(e) => {
                    return Err(crate::css::error::report_lightning_error(
                        format!("Global CSS Parsing: {}", e),
                        span,
                    ));
                }
            }
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
            warnings: state.warnings,
        })
    }
}

fn process_css_block(block: &CssBlock, state: &mut ParserState) -> Result<()> {
    for rule in &block.rules {
        let ctx = DynamicContext {
            class_name: &state.class_name,
            is_unsafe: state.is_unsafe,
            region: state.region.clone(),
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
                    &mut state.warnings,
                    prop_for_expr,
                    &ctx,
                )?;
                state.static_css.push_str(&val);

                if decl.semi_token.is_some() {
                    state.static_css.push_str("; ");
                }
            }
            CssRule::Apply(ap) => {
                #[cfg(feature = "tw")]
                {
                    let raw_str = ap.classes.trim().trim_matches('"');
                    let anchor = crate::css::tw::parser::TokenAnchor::whole(raw_str, ap.span);
                    let rules = crate::css::tw::parser::parse_class_list(&anchor, &mut Vec::new())?;
                    let apply_block = crate::css::tw::codegen::build_css_block_from_rules(rules)?;
                    process_css_block(&apply_block, state)?;
                }
                #[cfg(not(feature = "tw"))]
                {
                    return Err(syn::Error::new(
                        ap.span,
                        "The `@apply` directive requires the `tw` feature flag to be enabled in `silex_macros`.",
                    ));
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
                        &mut state.warnings,
                        &DynamicContext {
                            is_unsafe: false,
                            ..ctx.clone()
                        },
                    )?;
                    state.dynamic_rules.push(DynamicRule {
                        template,
                        expressions: selector_exprs,
                    });
                } else {
                    let sel_str = match lone_string_literal(&nested.selectors) {
                        Some(raw) => raw,
                        None => append_token_stream_strings(
                            &nested.selectors,
                            state.region.clone(),
                            &mut state.warnings,
                        )?,
                    };
                    state.static_css.push_str(&sel_str);
                    state.static_css.push_str(" { ");
                    process_css_block(&nested.block, state)?;
                    state.static_css.push_str(" } ");
                }
            }
            CssRule::AtRule(at) => {
                let params = extract_at_rule_params(
                    &at.params,
                    state.region.clone(),
                    &mut state.warnings,
                    &at.name,
                )?;
                let prelude = format!("@{} {}", at.name, params);

                // `@import` / `@charset` / `@layer a, b;` 这类没有块的语句式
                // at-rule 不能被塞进 `.class { }` 里，一律提到全局。
                let Some(at_block) = &at.block else {
                    state.lifted_css.push_str(&prelude);
                    state.lifted_css.push_str(";\n");
                    continue;
                };

                // 这几条规则在 CSS 里不允许嵌在样式规则内部，必须提到 `.class { }` 之外
                let is_lifted = matches!(at.name.as_str(), "keyframes" | "font-face")
                    && !state.class_name.is_empty();

                let mut inner_state = ParserState {
                    static_css: String::new(),
                    lifted_css: String::new(),
                    expressions: state.expressions.clone(),
                    dynamic_rules: Vec::new(),
                    warnings: state.warnings.clone(),
                    class_name: state.class_name.clone(),
                    is_unsafe: state.is_unsafe,
                    region: state.region.clone(),
                };

                process_css_block(at_block, &mut inner_state)?;

                // Sync back state
                state.expressions = inner_state.expressions;
                state.warnings = inner_state.warnings;
                // Dynamic rules inside @-rules is collected
                for dr in inner_state.dynamic_rules {
                    state.dynamic_rules.push(dr);
                }

                let body = inner_state.static_css;
                if !body.trim().is_empty() {
                    let rule_str = format!("{} {{ {} }} ", prelude, body);
                    if is_lifted {
                        state.lifted_css.push_str(&rule_str);
                        state.lifted_css.push('\n');
                    } else {
                        state.static_css.push_str(&rule_str);
                    }
                }

                // 内层提升出来的内容（`@media (…) { @font-face { … } }`）此前从不回传，
                // 整个 `@font-face` 会凭空消失且不报错。这里补回：条件组规则要把提升
                // 出来的内容重新包回自己，否则 `@media` 的条件就丢了。
                if !inner_state.lifted_css.trim().is_empty() {
                    if is_lifted {
                        state.lifted_css.push_str(&inner_state.lifted_css);
                    } else {
                        state
                            .lifted_css
                            .push_str(&format!("{} {{ {} }}\n", prelude, inner_state.lifted_css));
                    }
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
    warnings: &mut Vec<CssWarning>,
    ctx: &DynamicContext,
) -> Result<String> {
    let mut template = extract_dynamic_selector(&nested.selectors, selector_exprs, warnings, ctx)?;
    template.push_str(" { ");
    build_dynamic_block_recursive(
        &nested.block,
        &mut template,
        selector_exprs,
        global_expressions,
        warnings,
        ctx,
    )?;
    template.push_str(" }");
    Ok(template)
}

fn build_dynamic_block_recursive(
    block: &CssBlock,
    template: &mut String,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    warnings: &mut Vec<CssWarning>,
    ctx: &DynamicContext,
) -> Result<()> {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                template.push_str(&decl.property);
                template.push_str(": ");
                let prop_for_expr = if ctx.is_unsafe { "any" } else { &decl.property };
                let val = extract_dynamic_value(
                    &decl.values,
                    global_expressions,
                    warnings,
                    prop_for_expr,
                    ctx,
                )?;
                template.push_str(&val);
                if decl.semi_token.is_some() {
                    template.push_str("; ");
                }
            }
            CssRule::Nested(nested) => {
                let sel = extract_dynamic_selector(
                    &nested.selectors,
                    selector_exprs,
                    warnings,
                    &DynamicContext {
                        class_name: "",
                        ..ctx.clone()
                    },
                )?;
                template.push_str(&sel);
                template.push_str(" { ");
                build_dynamic_block_recursive(
                    &nested.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    warnings,
                    ctx,
                )?;
                template.push_str(" } ");
            }
            CssRule::AtRule(at) => {
                let params =
                    extract_at_rule_params(&at.params, ctx.region.clone(), warnings, &at.name)?;
                let Some(at_block) = &at.block else {
                    // 语句式 at-rule 不能出现在动态规则内部（它是全局声明）
                    return Err(syn::Error::new(
                        at.span,
                        format!(
                            "`@{}` is a statement-level at-rule and cannot appear inside a rule with a dynamic selector.",
                            at.name
                        ),
                    ));
                };
                template.push('@');
                template.push_str(&at.name);
                template.push(' ');
                template.push_str(&params);
                template.push_str(" { ");
                build_dynamic_block_recursive(
                    at_block,
                    template,
                    selector_exprs,
                    global_expressions,
                    warnings,
                    ctx,
                )?;
                template.push_str(" } ");
            }
            CssRule::Apply(ap) => {
                #[cfg(feature = "tw")]
                {
                    let raw_str = ap.classes.trim().trim_matches('"');
                    let anchor = crate::css::tw::parser::TokenAnchor::whole(raw_str, ap.span);
                    let rules = crate::css::tw::parser::parse_class_list(&anchor, &mut Vec::new())?;
                    let apply_block = crate::css::tw::codegen::build_css_block_from_rules(rules)?;
                    build_dynamic_block_recursive(
                        &apply_block,
                        template,
                        selector_exprs,
                        global_expressions,
                        warnings,
                        ctx,
                    )?;
                }
                #[cfg(not(feature = "tw"))]
                {
                    return Err(syn::Error::new(
                        ap.span,
                        "The `@apply` directive requires the `tw` feature flag to be enabled in `silex_macros`.",
                    ));
                }
            }
            CssRule::Unsafe(u) => {
                build_dynamic_block_recursive(
                    &u.block,
                    template,
                    selector_exprs,
                    global_expressions,
                    warnings,
                    &DynamicContext {
                        is_unsafe: true,
                        ..ctx.clone()
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// 选择器里是否含运行时片段。
///
/// `$` 后面跟标识符或 `$(…)` 都算。此前这里要求那个标识符**字面等于 `theme`**，
/// 于是 `.x $sel { … }` 会被当成静态选择器，`$` 直接喂给 lightningcss 报
/// `Unexpected token Delim('$')`——把变量改名叫 `theme` 才能用。
fn contains_dynamic_selector(ts: &TokenStream) -> bool {
    let mut iter = ts.clone().into_iter().peekable();
    while let Some(tt) = iter.next() {
        if let TokenTree::Punct(p) = &tt
            && p.as_char() == '$'
        {
            match iter.peek() {
                Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                    return true;
                }
                Some(TokenTree::Ident(_)) => return true,
                _ => {}
            }
        }
    }
    false
}

// --- Unified Token Stream Processing ---

/// 一层 token 的游标，同时携带从原文恢复出来的空白信息。
///
/// 直接迭代 `TokenStream` 会丢掉 token 之间的空白，而 CSS 里空白有语义
/// （见 [`crate::css::spacing`]）。这个游标把「下一个 token 前原文里是否有空白」
/// 和迭代绑在一起，`handler` 自行消费 token 时下标也能跟着走。
#[derive(Clone)]
pub(crate) struct CssTokens {
    iter: Peekable<IntoIter>,
    /// `info[i]`：第 i 个 token 在原文中的位置信息；`None` 表示原文不可得
    info: Option<Vec<crate::css::spacing::TokenSpacing>>,
    idx: usize,
    /// 本层 token 所处的原文片段
    region: Option<Rc<str>>,
}

impl CssTokens {
    fn new(ts: &TokenStream, region: Option<Rc<str>>) -> Self {
        let info = region.as_deref().and_then(|src| {
            let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
            crate::css::spacing::recover(&tokens, src)
        });
        Self {
            iter: ts.clone().into_iter().peekable(),
            info,
            idx: 0,
            region,
        }
    }

    /// 进入一个 `Group`：用匹配时算出的组内范围切出子片段。
    /// 定位失败时子层也没有原文可依据，退回启发式。
    fn descend(&self, g: &proc_macro2::Group, inner: Option<Range<usize>>) -> Self {
        let region = match (&self.region, inner) {
            (Some(src), Some(range)) => src.get(range).map(Rc::<str>::from),
            _ => None,
        };
        Self::new(&g.stream(), region)
    }

    pub(crate) fn next(&mut self) -> Option<TokenTree> {
        let tt = self.iter.next();
        if tt.is_some() {
            self.idx += 1;
        }
        tt
    }

    pub(crate) fn peek(&mut self) -> Option<&TokenTree> {
        self.iter.peek()
    }

    /// 下一个 token 在原文中的位置信息；`None` = 无法确定。
    fn info_of_next(&self) -> Option<crate::css::spacing::TokenSpacing> {
        self.info.as_ref().and_then(|v| v.get(self.idx).cloned())
    }

    /// 下一个 token 前原文里是否有空白；`None` = 无法确定。
    pub(crate) fn space_before_next(&self) -> Option<bool> {
        self.info_of_next().map(|i| i.space_before)
    }
}

/// 整个宏调用的源码。stable 工具链上这是恢复空白的唯一依据。
fn macro_region() -> Option<Rc<str>> {
    Span::call_site().source_text().map(Rc::<str>::from)
}

fn process_tokens<F>(
    ts: &TokenStream,
    region: Option<Rc<str>>,
    warnings: &mut Vec<CssWarning>,
    handler: &mut F,
) -> Result<String>
where
    F: FnMut(&TokenTree, &mut CssTokens, &mut String, bool) -> Result<bool>,
{
    let mut cursor = CssTokens::new(ts, region);
    process_tokens_iter(&mut cursor, warnings, handler)
}

/// 原文不可得时的保守回退：只按 token 类型猜。
///
/// 这条路只在 token 不来自用户源码时才会走到（例如 `@apply` 由 tw 展开出来的
/// 规则），那些 token 的形状是我们自己生成的、可控的。
fn guess_space_between(prev: &TokenTree, cur: &TokenTree) -> bool {
    /// 媒体/特性查询里的逻辑关键字，后面跟括号时必须留空格，
    /// 否则 `screen and (…)` 会被拼成函数调用 `and(…)`。
    ///
    /// 只收那些**不可能是 CSS 函数名**的词：`not` / `selector` 同时也是伪类函数
    /// （`:not(.a)`、`@supports selector(…)`），给它们补空格会把 `:not ([hidden])`
    /// 写成非法选择器。用户手写的媒体查询能从原文恢复空白，走不到这条回退。
    const QUERY_KEYWORDS: [&str; 3] = ["and", "or", "only"];

    match (prev, cur) {
        (TokenTree::Ident(_), TokenTree::Ident(_))
        | (TokenTree::Ident(_), TokenTree::Literal(_))
        | (TokenTree::Literal(_), TokenTree::Ident(_))
        | (TokenTree::Literal(_), TokenTree::Literal(_))
        | (TokenTree::Group(_), TokenTree::Ident(_))
        | (TokenTree::Group(_), TokenTree::Literal(_))
        | (TokenTree::Group(_), TokenTree::Group(_)) => true,
        // `and (min-width: 1px)`：关键字与括号之间必须有空格
        (TokenTree::Ident(id), TokenTree::Group(g))
            if g.delimiter() == Delimiter::Parenthesis
                && QUERY_KEYWORDS.contains(&id.to_string().as_str()) =>
        {
            true
        }
        (TokenTree::Ident(_), TokenTree::Punct(p)) if p.as_char() == '&' => true,
        // `& span`：`&` 后紧跟元素名时按后代选择器处理。复合形式（`&span`，
        // 即「自身同时是该元素」）远比后代少见，需要时用字符串字面量选择器书写。
        (TokenTree::Punct(p), TokenTree::Ident(_)) if p.as_char() == '&' => true,
        (TokenTree::Punct(p), TokenTree::Ident(_))
        | (TokenTree::Punct(p), TokenTree::Literal(_))
            if p.as_char() == '$' =>
        {
            true
        }
        (TokenTree::Punct(p1), TokenTree::Punct(p2))
            if p2.as_char() == '&'
                && (p1.as_char() == '~' || p1.as_char() == '>' || p1.as_char() == '+') =>
        {
            true
        }
        (TokenTree::Punct(p1), _)
            if p1.as_char() == '~' || p1.as_char() == '>' || p1.as_char() == '+' =>
        {
            true
        }
        _ => false,
    }
}

fn process_tokens_iter<F>(
    cursor: &mut CssTokens,
    warnings: &mut Vec<CssWarning>,
    handler: &mut F,
) -> Result<String>
where
    F: FnMut(&TokenTree, &mut CssTokens, &mut String, bool) -> Result<bool>,
{
    let mut out = String::new();
    let mut prev_tt: Option<TokenTree> = None;

    loop {
        let info = cursor.info_of_next();
        let Some(tt) = cursor.next() else { break };

        let space_before = match (&prev_tt, &info) {
            (None, _) => false,
            (Some(_), Some(known)) => known.space_before,
            (Some(prev), None) => guess_space_between(prev, &tt),
        };

        if handler(&tt, cursor, &mut out, space_before)? {
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
                let mut sub = cursor.descend(&g, info.and_then(|i| i.inner));
                out.push_str(&process_tokens_iter(&mut sub, warnings, handler)?);
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                if p.as_char() == '?' {
                    warnings.push(CssWarning {
                        message: "[Silex CSS Warning] Potentially ambiguous token '?' in CSS stream. If this is a Rust expression, wrap it in $(...).".to_string(),
                        span: p.span(),
                    });
                }
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&render_literal(&lit));
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
    Ok(out)
}

/// 把 Rust 字面量转成 CSS 里的等价写法。
///
/// 字符串字面量**保留引号**：`content: "hello"`、`grid-template-areas: "a b" "c d"`、
/// `[data-x="1"]`、`url("a b.png")` 都依赖它。此前这里无条件剥离引号，产出的
/// `content:hello` 是无效声明、`quotes:" "` 更是把两个字符串并成了一个。
///
/// 走 `syn::Lit` 拿到字面量的真实内容（顺带支持 `r"…"` / `r#"…"#`），再按 CSS 的
/// 转义规则重新写出，而不是原样透传 Rust 的转义序列。
fn render_literal(lit: &proc_macro2::Literal) -> String {
    match syn::Lit::new(lit.clone()) {
        syn::Lit::Str(s) => escape_css_string(&s.value()),
        // 字节串是代码生成器的「逐字 CSS 文本」标记，见 `ast::verbatim_literal`
        syn::Lit::ByteStr(b) => String::from_utf8(b.value()).unwrap_or_default(),
        syn::Lit::Char(c) => escape_css_string(&c.value().to_string()),
        syn::Lit::CStr(_) | syn::Lit::Byte(_) => lit.to_string(),
        _ => lit.to_string(),
    }
}

/// 按 CSS 的 `<string>` 语法写出一个带引号的字符串。
pub(crate) fn escape_css_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            // CSS 字符串里不允许裸换行，必须写成 Unicode 转义
            '\n' => out.push_str("\\A "),
            '\r' => out.push_str("\\D "),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn handle_dollar_path(iter: &mut CssTokens) -> syn::Result<Option<TokenStream>> {
    let mut sub_iter = iter.clone();
    if let Some(TokenTree::Ident(id)) = sub_iter.next() {
        // Try parsing as a path
        let mut tokens = vec![TokenTree::Ident(id)];
        while let Some(TokenTree::Punct(p)) = sub_iter.peek()
            && p.as_char() == ':'
        {
            let p1 = sub_iter.next().unwrap();
            if let Some(tt2) = sub_iter.next() {
                if let TokenTree::Punct(ref p2) = tt2
                    && p2.as_char() == ':'
                {
                    tokens.push(p1);
                    tokens.push(tt2);
                    if let Some(TokenTree::Ident(next_id)) = sub_iter.next() {
                        tokens.push(TokenTree::Ident(next_id));
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        *iter = sub_iter;
        return Ok(Some(tokens.into_iter().collect()));
    }
    Ok(None)
}

pub fn append_token_stream_strings(
    ts: &TokenStream,
    region: Option<Rc<str>>,
    warnings: &mut Vec<CssWarning>,
) -> Result<String> {
    // Basic version used for @-rules and such, no special $ or & handling
    process_tokens(ts, region, warnings, &mut |_, _, _, _| Ok(false))
}

/// 整段写成一个字符串字面量时（`"div > p" { … }`、`@media "(width >= 600px)"`）
/// 取其裸内容。
///
/// 这是 token 流无法表达的写法（复合元素选择器 `&div`、`:not(.a .b)` 这种依赖
/// 精确空白的选择器，以及 `(width >= 600px)` 这类会被 Rust 词法重排的条件）的
/// 逃生舱，所以这里——也只有这里——才剥引号。`tw` 的 codegen 正是走这条路把
/// 选择器与查询条件原样递给编译器的。
fn lone_string_literal(ts: &TokenStream) -> Option<String> {
    let mut iter = ts.clone().into_iter();
    let first = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    let TokenTree::Literal(lit) = first else {
        return None;
    };
    match syn::Lit::new(lit) {
        syn::Lit::Str(s) => Some(s.value()),
        _ => None,
    }
}

/// `$var` 后面跟什么算合法。
///
/// 判据是原文里有没有空白：`$theme.field` 是字段访问（必须写成 `$(…)`），
/// `$theme .x` 是后代选择器、`$c !important` 是优先级标记，两者都合法。
/// 原文不可得时保持从严——宁可要求用户显式写 `$(…)`，也不猜。
fn check_unexpected_complex_tokens(iter: &mut CssTokens) -> syn::Result<()> {
    let separated = iter.space_before_next() == Some(true);
    if let Some(next_tt) = iter.peek() {
        match next_tt {
            TokenTree::Punct(p_next)
                if matches!(p_next.as_char(), '.' | '!' | '?' | ':') && !separated =>
            {
                return Err(syn::Error::new(
                    p_next.span(),
                    format!(
                        "Unexpected '{}' after dynamic variable. Complex expressions like method calls, array indexing, or field access must be wrapped in $(...).",
                        p_next.as_char()
                    ),
                ));
            }
            // `(` 紧跟变量一律视为调用；`[` 只在紧贴时视为索引
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Parenthesis
                    || (g.delimiter() == Delimiter::Bracket && !separated) =>
            {
                return Err(syn::Error::new(
                    g.span(),
                    "Unexpected brackets/parentheses after dynamic variable. Complex expressions like method calls, array indexing, or field access must be wrapped in $(...).",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// at-rule 的参数（`@media (…)`、`@keyframes name`、`@supports (…)`）。
///
/// 这里**不接受**运行时值：媒体查询的条件在 CSS 里不允许出现 `var()`，
/// 之前把 `$w` 替换成 `var(--cls-0)` 的实现无论如何都产不出可用结果，
/// 只会以 `Invalid media query` 的形式炸在 lightningcss 里。直接给出可读报错。
fn extract_at_rule_params(
    ts: &TokenStream,
    region: Option<Rc<str>>,
    warnings: &mut Vec<CssWarning>,
    at_name: &str,
) -> Result<String> {
    if let Some(raw) = lone_string_literal(ts) {
        return Ok(raw);
    }
    process_tokens(ts, region, warnings, &mut |tt, _iter, _out, _space| {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == '$'
        {
            return Err(syn::Error::new(
                p.span(),
                format!(
                    "`@{at_name}` parameters cannot contain runtime values: CSS does not allow \
                     `var()` inside at-rule preludes, so there is no way to make this work. \
                     Use a container query, or toggle a class / data attribute from Rust and \
                     branch on it inside the rule body."
                ),
            ));
        }
        Ok(false)
    })
}

/// 动态选择器。选择器里的运行时片段统一用位置占位符 `{}` 表示，
/// 由 `styled!` / `global!` 侧按顺序填回（见 `expand_dynamic_rule`）。
fn extract_dynamic_selector(
    ts: &TokenStream,
    exprs: &mut Vec<(String, TokenStream)>,
    warnings: &mut Vec<CssWarning>,
    ctx: &DynamicContext,
) -> Result<String> {
    if let Some(raw) = lone_string_literal(ts) {
        return Ok(raw);
    }
    process_tokens(
        ts,
        ctx.region.clone(),
        warnings,
        &mut |tt, iter, out, space_before| {
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
                        return Ok(true);
                    }
                    if let Some(path) = handle_dollar_path(iter)? {
                        check_unexpected_complex_tokens(iter)?;
                        if space_before {
                            out.push(' ');
                        }
                        out.push_str("{}");
                        exprs.push(("any".to_string(), path));
                        return Ok(true);
                    }
                    return Err(syn::Error::new(
                        p.span(),
                        "Invalid dynamic expression syntax after '$'. Expected $ident, $path, or $(expression).",
                    ));
                } else if p.as_char() == '&' && !ctx.class_name.is_empty() {
                    if space_before {
                        out.push(' ');
                    }
                    out.push_str(&format!(".{}", ctx.class_name));
                    return Ok(true);
                }
            }
            Ok(false)
        },
    )
}

/// 声明值里的运行时片段。
///
/// 组件模式下走元素上的 CSS 变量 `var(--<class>-N)`；全局模式下没有可挂变量的
/// 元素，改用 `var(--slx-dyn-N)` 作为**文本占位符**，由 `global_impl` 直接替换。
/// 注意 `$(expr)` 与 `$path` 两条分支必须产出同一种占位符——此前 `$(expr)` 在
/// 全局模式下吐的是 `{}`，而 `global_impl` 只替换 `var(--slx-dyn-N)`，
/// 于是 `global!{ body { color: $(c); } }` 编译直接失败。
fn extract_dynamic_value(
    ts: &TokenStream,
    exprs: &mut Vec<(String, TokenStream)>,
    warnings: &mut Vec<CssWarning>,
    prop_name: &str,
    ctx: &DynamicContext,
) -> Result<String> {
    let placeholder = |idx: usize| {
        if ctx.class_name.is_empty() {
            format!("var(--slx-dyn-{})", idx)
        } else {
            format!("var(--{}-{})", ctx.class_name, idx)
        }
    };
    process_tokens(
        ts,
        ctx.region.clone(),
        warnings,
        &mut |tt, iter, out, space_before| {
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
                    out.push_str(&placeholder(idx));
                    iter.next();
                    return Ok(true);
                }
                if let Some(path) = handle_dollar_path(iter)? {
                    check_unexpected_complex_tokens(iter)?;
                    if space_before {
                        out.push(' ');
                    }
                    let idx = exprs.len();
                    exprs.push((prop_name.to_string(), path));
                    out.push_str(&placeholder(idx));
                    return Ok(true);
                }
                return Err(syn::Error::new(
                    p.span(),
                    "Invalid dynamic expression syntax after '$'. Expected $ident, $path, or $(expression).",
                ));
            }
            Ok(false)
        },
    )
}

fn get_compiler_targets() -> Targets {
    Targets {
        browsers: Some(lightningcss::targets::Browsers {
            chrome: Some(80 << 16),
            safari: Some(13 << 16),
            firefox: Some(75 << 16),
            ..Default::default()
        }),
        ..Targets::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编译一段 CSS 并返回 `static_css + component_css`，方便断言。
    fn compile_all(src: &str) -> String {
        let res = CssCompiler::compile_with_source(src, Span::call_site(), false).unwrap();
        format!("{}{}", res.static_css, res.component_css)
    }

    fn compile_err(src: &str) -> String {
        CssCompiler::compile_with_source(src, Span::call_site(), false)
            .unwrap_err()
            .to_string()
    }

    // --- P0-1：选择器与媒体查询必须按原文的空白还原 ---

    /// `& span` 是后代选择器。此前空白被丢掉、拼成 `&span`，
    /// lightningcss 展开为 `span.cls`——仍是合法选择器，只是匹配的是
    /// 完全不同的一批元素，不报错也不告警。
    #[test]
    fn descendant_selector_stays_a_descendant() {
        let css = compile_all("& span { color: red; }");
        assert!(css.contains(" span{"), "{css}");
        assert!(!css.contains("span.slx-"), "{css}");
    }

    #[test]
    fn compound_selector_stays_compound() {
        // 没有空白时仍是复合选择器：`.cls` 自身就是 `span`
        let css = compile_all("&span { color: red; }");
        assert!(css.contains("span.slx-"), "{css}");
    }

    #[test]
    fn selector_list_keeps_every_branch_a_descendant() {
        let css = compile_all("& p, & span { color: red; }");
        assert!(css.contains(" p,"), "{css}");
        assert!(css.contains(" span{"), "{css}");
    }

    /// `:not(.a .b)`（后代）与 `:not(.a.b)`（复合）是两回事
    #[test]
    fn whitespace_inside_functional_pseudo_class_survives() {
        let css = compile_all("&:not(.a .b) { color: red; }");
        assert!(css.contains(".a .b"), "{css}");
    }

    /// `screen and (min-width: 1px)` 此前会被拼成函数调用 `and(…)`，
    /// 直接编译失败（`Unexpected token Function("and")`）
    #[test]
    fn compound_media_queries_compile() {
        let css = compile_all("@media screen and (min-width: 1px) { color: red; }");
        assert!(css.contains("@media screen and (min-width:1px)"), "{css}");

        let css = compile_all("@media (min-width: 1px) and (max-width: 9px) { color: red; }");
        assert!(css.contains("and"), "{css}");
    }

    /// token 流表达不了的选择器可以整体写成字符串字面量
    #[test]
    fn string_literal_selectors_are_taken_verbatim() {
        let css = compile_all("\"div > p\" { color: red; }");
        assert!(css.contains("div>p") || css.contains("div > p"), "{css}");
    }

    // --- P0-2：字符串字面量保留引号 ---

    #[test]
    fn string_values_keep_their_quotes() {
        let css = compile_all("content: \"hello\";");
        assert!(css.contains("content:\"hello\""), "{css}");
    }

    /// 此前 `quotes: "\"" "\"";` 会被还原成 `quotes:" "`——
    /// 两个字符串被并成了一个含空格的字符串，语义与源码毫无关系
    #[test]
    fn adjacent_strings_stay_separate() {
        let css = compile_all(r#"grid-template-areas: "a b" "c d";"#);
        // 压缩后两个字符串紧邻，但仍是两个独立的 <string>
        assert!(css.contains(r#""a b""c d""#), "{css}");
    }

    /// 报告里最能说明问题的一行：转义被“还原”后引号又被当普通字符输出，
    /// `quotes: "\"" "\"";` 产出的 `quotes:" "` 与源码语义毫无关系
    #[test]
    fn escaped_quotes_survive_as_two_separate_strings() {
        let css = compile_all(r#"quotes: "\"" "\"";"#);
        assert!(css.contains(r#"quotes:"\"" "\"""#), "{css}");
    }

    #[test]
    fn attribute_selectors_and_quoted_urls_compile() {
        let css = compile_all("[data-x=\"1\"] & { color: red; }");
        assert!(css.contains("[data-x"), "{css}");

        let css = compile_all("background-image: url(\"a b.png\");");
        assert!(css.contains("a b.png"), "{css}");
    }

    // --- P0-3：嵌套在条件组规则里的 @font-face / @keyframes 不能丢 ---

    #[test]
    fn font_face_nested_in_media_is_not_dropped() {
        let css = compile_all(
            "@media (min-width: 1px) { @font-face { font-family: \"X\"; src: url(a.woff2); } }",
        );
        assert!(css.contains("@font-face"), "{css}");
        // 提升出 `.class { }` 的同时，`@media` 的条件必须保住
        assert!(css.contains("@media"), "{css}");
    }

    #[test]
    fn keyframes_nested_in_supports_is_not_dropped() {
        let css = compile_all("@supports (display: grid) { @keyframes k { 0% { opacity: 0; } } }");
        assert!(css.contains("@keyframes"), "{css}");
        assert!(css.contains("@supports"), "{css}");
    }

    /// 语句式 at-rule（无块）此前根本解析不了，编译器里那条 `import` 分支是死代码
    #[test]
    fn statement_at_rules_are_lifted() {
        let css = compile_all("@import url(\"a.css\"); color: red;");
        assert!(css.contains("@import"), "{css}");
    }

    // --- P0-5/6：动态选择器与 at-rule 参数 ---

    /// 变量名不再必须叫 `theme`
    #[test]
    fn dynamic_selectors_accept_any_variable_name() {
        let res =
            CssCompiler::compile_with_source(".x $sel { color: red; }", Span::call_site(), false)
                .unwrap();
        assert_eq!(res.dynamic_rules.len(), 1);
        assert!(
            res.dynamic_rules[0].template.contains("{}"),
            "{:?}",
            res.dynamic_rules[0]
        );
    }

    /// `$sel .x` 是后代选择器，不是字段访问
    #[test]
    fn dynamic_selector_can_be_followed_by_a_descendant() {
        let res =
            CssCompiler::compile_with_source("$sel .x { color: red; }", Span::call_site(), false)
                .unwrap();
        assert_eq!(res.dynamic_rules.len(), 1);
        assert!(
            res.dynamic_rules[0].template.contains("{} .x"),
            "{:?}",
            res.dynamic_rules[0]
        );
    }

    /// 紧贴的 `.` 仍然是字段访问，必须写成 `$(…)`
    #[test]
    fn field_access_after_a_dynamic_variable_is_still_rejected() {
        assert!(
            compile_err("color: $theme.primary;").contains("must be wrapped in $(...)"),
            "字段访问应当继续报错"
        );
    }

    /// 媒体查询里放不进 `var()`，这条路以前写得很完整却必然失败
    #[test]
    fn dynamic_values_in_at_rule_params_are_rejected_with_a_readable_error() {
        let err = compile_err("@media (min-width: $w) { color: red; }");
        assert!(err.contains("cannot contain runtime values"), "{err}");
        assert!(err.contains("container query"), "{err}");
    }

    // --- P0-4：global! 的动态占位符 ---

    /// `$(expr)` 与 `$path` 在全局模式下必须产出同一种占位符。
    /// 此前 `$(expr)` 吐 `{}`，`global_impl` 只替换 `var(--slx-dyn-N)`，
    /// 于是 `{}` 泄漏进 CSS，被 lightningcss 以 `Unexpected token CurlyBracketBlock` 拒绝。
    #[test]
    fn global_value_placeholders_agree_between_both_syntaxes() {
        for src in ["body { color: $(my_color); }", "body { color: $my_color; }"] {
            let res =
                CssCompiler::compile_global_with_source(src, Span::call_site(), false).unwrap();
            let css = format!("{}{}", res.static_css, res.component_css);
            assert!(css.contains("var(--slx-dyn-0)"), "{src} => {css}");
            assert!(!css.contains("{}"), "{src} => {css}");
        }
    }

    #[test]
    fn global_dynamic_selector_uses_positional_placeholder() {
        let res = CssCompiler::compile_global_with_source(
            ".x $theme { color: red; }",
            Span::call_site(),
            false,
        )
        .unwrap();
        assert_eq!(res.dynamic_rules.len(), 1);
        assert!(
            res.dynamic_rules[0].template.contains(".x {}"),
            "{:?}",
            res.dynamic_rules[0].template
        );
    }

    #[test]
    fn test_invalid_dollar_syntax_fails() {
        let ts = syn::parse_str("color: $;").unwrap();
        let err = CssCompiler::compile(ts, Span::call_site(), false).unwrap_err();
        assert!(
            err.to_string()
                .contains("Invalid dynamic expression syntax after '$'")
        );
    }

    #[test]
    fn test_unwrapped_indexing_fails() {
        let ts = syn::parse_str("color: $theme[0];").unwrap();
        let err = CssCompiler::compile(ts, Span::call_site(), false).unwrap_err();
        assert!(
            err.to_string()
                .contains("Unexpected brackets/parentheses after dynamic variable")
        );
    }

    #[test]
    fn test_spacing_between_var_and_ident() {
        let ts = syn::parse_str("border: $width solid $color;").unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        assert!(res.component_css.contains("solid"));
        assert_eq!(res.expressions.len(), 2);
    }

    #[test]
    fn test_keyframes_uses_class_prefix_for_vars() {
        let ts = syn::parse_str("@keyframes slide { 0% { margin-top: $val; } }").unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        assert!(
            res.static_css
                .contains(&format!("var(--{}-0)", res.class_name))
        );
        assert_eq!(res.expressions.len(), 1);
    }

    #[test]
    fn test_at_media_with_dynamic_value() {
        let ts = syn::parse_str("@media (min-width: 600px) { color: $color; }").unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        assert_eq!(res.expressions.len(), 1);
        assert!(
            res.component_css
                .contains(&format!("var(--{}-0)", res.class_name))
        );
    }

    #[test]
    #[cfg(feature = "tw")]
    fn test_apply_directive() {
        let ts = syn::parse_str("@apply flex items-center px-4 py-2;").unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        assert!(res.component_css.contains("display:flex"));
        assert!(res.component_css.contains("align-items:center"));
        assert!(res.component_css.contains("padding:.5rem 1rem"));
    }

    /// at-rule 名可以带连字符（`@font-face` / `@starting-style`）。
    ///
    /// 这类名字不是合法的 Rust 标识符，`name: Ident` 只能吃到 `font`，剩下的 `-face`
    /// 会漂到 params 里，产出 `@font -face { … }`；`is_lifted` 里那句
    /// `at.name == "font-face"` 也因此永远不成立，`@font-face` 不会被提到全局 CSS。
    #[test]
    fn hyphenated_at_rule_names_survive_parsing() {
        let ts = syn::parse_str(
            "@font-face { font-family: \"X\"; } @starting-style { opacity: 0; } @media (min-width: 600px) { color: red; }",
        )
        .unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        let all = format!("{}{}", res.static_css, res.component_css);
        assert!(all.contains("@font-face"), "{all}");
        assert!(all.contains("@starting-style"), "{all}");
        // `@media` 的参数里也有 `-`（`min-width`），不能被当成名字的一部分
        assert!(all.contains("@media (min-width:600px)"), "{all}");
    }

    #[test]
    fn test_warning_emitted_for_question_mark() {
        let ts = syn::parse_str("color: ?;").unwrap();
        let res = CssCompiler::compile(ts, Span::call_site(), false).unwrap();
        assert_eq!(res.warnings.len(), 1);
        assert!(
            res.warnings[0]
                .message
                .contains("Potentially ambiguous token '?'")
        );
    }
}
