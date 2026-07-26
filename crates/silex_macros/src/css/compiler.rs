use crate::css::ast::{CssBlock, CssRule};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::token_stream::IntoIter;
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;
use std::ops::Range;
use std::rc::Rc;
use syn::Result;

/// 级联层的名字。
///
/// 这四个常量必须与 `silex_css::layers` 里的同名常量保持一致——proc-macro
/// crate 不能依赖运行时 crate，只能各写一份。层序的完整说明在
/// `silex_css/src/layers.rs`。
pub(crate) const LAYER_BASE: &str = "base";
pub(crate) const LAYER_COMPONENTS: &str = "components";
pub(crate) const LAYER_UTILITIES: &str = "utilities";

#[derive(Debug, Clone)]
pub struct DynamicRule {
    /// 结构化模板：`\u{1}` 是组件类名占位，`\u{2}` 是第 n 个运行时取值占位
    /// （按出现顺序）。见 [`template_parts`]。
    pub template: String,
    pub expressions: Vec<(String, TokenStream)>,
}

/// 动态模板里的占位符。
///
/// 用控制字符而不是 `{}` / 类名文本，是为了让「哪里要填东西」这件事在编译期
/// 就确定下来：运行时只做拼接，不再做模式匹配，也就不会误伤 `.foo-bar` 这种
/// 以基类名开头的选择器，或值内容里恰好出现的 `{}`。
/// [`escape_css_string`] 会把用户字符串里的控制字符转义掉，所以这两个字符
/// 不可能来自源码。
pub(crate) const PLACEHOLDER_CLASS: char = '\u{1}';
pub(crate) const PLACEHOLDER_VALUE: char = '\u{2}';

/// 类名在编译期的占位。
///
/// 类名要写进产物（`.slx-xxx { … }`、`var(--slx-xxx-0)`），而产物又要用来算类名，
/// 这是个环。解法是先用这个占位符跑完整个生成过程，拿产物取哈希得到真正的类名，
/// 再把占位符逐字换回去。见 [`CssCompiler::compile_block_internal`]。
///
/// 和上面两个占位符同理，用控制字符是为了让它不可能来自源码——[`escape_css_string`]
/// 会把用户字符串里的控制字符转义掉。
const PLACEHOLDER_PENDING_CLASS: &str = "\u{3}";

/// 模板的一个片段，与 `silex_css::runtime::template::CssPart` 一一对应。
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    Lit(String),
    Class,
    Val(usize),
}

/// 把带占位符的模板切成片段。
pub fn template_parts(template: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut next_val = 0;
    for ch in template.chars() {
        match ch {
            PLACEHOLDER_CLASS | PLACEHOLDER_VALUE => {
                if !lit.is_empty() {
                    parts.push(TemplatePart::Lit(std::mem::take(&mut lit)));
                }
                if ch == PLACEHOLDER_CLASS {
                    parts.push(TemplatePart::Class);
                } else {
                    parts.push(TemplatePart::Val(next_val));
                    next_val += 1;
                }
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        parts.push(TemplatePart::Lit(lit));
    }
    parts
}

/// 把片段展开成 `&'static [silex::css::CssPart]`。
pub fn template_parts_tokens(template: &str) -> TokenStream {
    let __silex = crate::crate_path::silex();
    let items = template_parts(template).into_iter().map(|p| match p {
        TemplatePart::Lit(s) => quote::quote! { #__silex::css::CssPart::Lit(#s) },
        TemplatePart::Class => quote::quote! { #__silex::css::CssPart::Class },
        TemplatePart::Val(i) => quote::quote! { #__silex::css::CssPart::Val(#i) },
    });
    quote::quote! { &[ #(#items),* ] }
}

#[derive(Debug, Clone)]
pub struct CssWarning {
    pub message: String,
    pub span: Span,
}

/// 一条静态声明的编译期类型断言。
///
/// `css!{ color: 10px }` 这样的写法此前完全不经过类型系统——`ValidFor` 只在
/// `$expr` 分支起作用，静态声明是纯字符串拼接。这里把「取值一眼能定型」的
/// 静态声明也接回类型系统。
#[derive(Debug, Clone)]
pub struct StaticAssertion {
    /// CSS 属性名（kebab-case 或短别名）
    pub property: String,
    /// 取值对应的 CSS 值类型名（`silex::css::types::` 下的类型）
    pub value_type: &'static str,
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
    pub assertions: Vec<StaticAssertion>,
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
    assertions: Vec<StaticAssertion>,
    class_name: String,
    is_unsafe: bool,
    /// 是否校验属性名与静态取值。`@apply` 展开出来的声明是机器生成的
    /// （含 `--tw-*` 与厂商前缀），不走这套判据。
    validate: bool,
    /// 整个宏调用的源码，用于恢复 token 之间的空白（见 [`crate::css::spacing`]）
    region: Option<Rc<str>>,
}

#[derive(Clone)]
struct DynamicContext<'a> {
    class_name: &'a str,
    is_unsafe: bool,
    region: Option<Rc<str>>,
}

/// `compile_block_internal` 的入参。
///
/// 这些开关组合起来决定「谁在编译、编译给谁用」，散成一长串位置参数极易接错。
struct CompileOptions<'a> {
    span: Span,
    /// 是否把产物包进 `.class { }`（`global!` 不包）
    wrap_in_class: bool,
    is_unsafe: bool,
    prefix: &'a str,
    /// 宏调用点的源码，用于恢复 token 之间的空白
    region: Option<Rc<str>>,
    /// 是否校验属性名与静态取值（机器生成的 CSS 不校验）
    validate: bool,
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
        // 这条入口只服务 `tw!` —— 传进来的 `CssBlock` 是 resolver 生成的，
        // 里头有 `--tw-*` 与 `clip` 这类注册表之外的属性名，不该拿用户书写
        // 的那套判据去卡
        Self::compile_block_internal(
            block,
            CompileOptions {
                span,
                wrap_in_class: true,
                is_unsafe,
                prefix,
                region: macro_region(),
                validate: false,
            },
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
            CompileOptions {
                span,
                wrap_in_class: true,
                is_unsafe,
                prefix: "slx-tw-",
                region: Some(Rc::from(source)),
                validate: true,
            },
        )
    }

    /// 同上，但可以指定前缀（前缀决定落进哪个 layer）。
    #[cfg(test)]
    pub fn compile_with_source_and_prefix(
        source: &str,
        prefix: &str,
        span: Span,
    ) -> Result<CssCompileResult> {
        let ts: TokenStream = source.parse().map_err(|e| syn::Error::new(span, e))?;
        let block: CssBlock = syn::parse2(ts.clone())?;
        Self::compile_block_internal(
            &block,
            CompileOptions {
                span,
                wrap_in_class: true,
                is_unsafe: false,
                prefix,
                region: Some(Rc::from(source)),
                validate: true,
            },
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
            CompileOptions {
                span,
                wrap_in_class: false,
                is_unsafe,
                prefix: "slx-",
                region: Some(Rc::from(source)),
                validate: true,
            },
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
            CompileOptions {
                span,
                wrap_in_class: true,
                is_unsafe,
                prefix,
                region: macro_region(),
                validate: true,
            },
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
            CompileOptions {
                span,
                wrap_in_class: false,
                is_unsafe,
                prefix: "slx-",
                region: macro_region(),
                validate: true,
            },
        )
    }

    /// 类名从**产物**取哈希，不从宏输入的源码文本取。
    ///
    /// 从前哈希的是 `TokenStream` 的文本，于是 `css!{ color: red }` 与
    /// `css!{ color: red; }` 落到两个类名、注入两份一模一样的规则。同一个 crate 里
    /// `static_id` 那一侧一直是对的（`format!("static-{}", hash_one(&final_static_css))`
    /// 哈希的就是产物），组件 CSS 只是没跟上。
    ///
    /// 挡在中间的是个环：类名要写进产物（`.slx-xxx { … }`、`var(--slx-xxx-0)`），
    /// 产物又要用来算类名。解法是先拿 [`PLACEHOLDER_PENDING_CLASS`] 跑完整个生成
    /// 过程，对产物取哈希算出类名，再把占位符换回真名——`Style::render` 那一侧
    /// 早就是这么解的（`runtime/template.rs`：哈希模板结构而不是渲染结果）。
    ///
    /// 哈希的是**最小化之前**的中间产物而不是最终 CSS：最小化必须先有类名，
    /// 而且中间产物相等一定蕴含最终 CSS 相等，所以这个口径只会少合、不会错合。
    fn compile_block_internal(
        block: &CssBlock,
        opts: CompileOptions<'_>,
    ) -> Result<CssCompileResult> {
        let CompileOptions {
            span,
            wrap_in_class,
            is_unsafe,
            prefix,
            region,
            validate,
        } = opts;

        let mut state = ParserState {
            static_css: String::new(),
            lifted_css: String::new(),
            expressions: Vec::new(),
            dynamic_rules: Vec::new(),
            warnings: Vec::new(),
            assertions: Vec::new(),
            class_name: if wrap_in_class {
                PLACEHOLDER_PENDING_CLASS.to_string()
            } else {
                // `global!` 不包 `.class { }`，产物里根本没有类名，也就没有环
                "".to_string()
            },
            is_unsafe,
            validate,
            region,
        };

        process_css_block(block, &mut state)?;

        let class_name = format!("{}{}", prefix, fingerprint(prefix, &state));
        let style_id = format!("style-{}", class_name);
        if wrap_in_class {
            state.static_css = state
                .static_css
                .replace(PLACEHOLDER_PENDING_CLASS, &class_name);
            state.lifted_css = state
                .lifted_css
                .replace(PLACEHOLDER_PENDING_CLASS, &class_name);
            for rule in &mut state.dynamic_rules {
                rule.template = rule
                    .template
                    .replace(PLACEHOLDER_PENDING_CLASS, &class_name);
            }
        }

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
                "slx-twv-" | "slx-st-" => LAYER_COMPONENTS,
                _ => LAYER_UTILITIES,
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
            //
            // `global!` 的产出此前**不带 layer**。规范里无层规则的优先级高于所有
            // 具名层，于是全局重置无条件压过每一个组件样式——恰好和「重置垫在
            // 最底下」的直觉相反。`base` 这一层从层序声明出现起就一直空着，
            // 它本来就是留给这里的。
            let wrapped = format!("@layer {} {{ {} }}", LAYER_BASE, state.static_css);
            match StyleSheet::parse(&wrapped, ParserOptions::default()) {
                Ok(stylesheet) => stylesheet
                    .to_css(PrinterOptions {
                        minify: true,
                        targets: get_compiler_targets(),
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
            assertions: state.assertions,
        })
    }
}

/// 产物的指纹，Base36 编码后就是类名的后缀。
///
/// 喂进去的是「这次编译会产出什么」的全部：静态 CSS、提升出去的 CSS、每条动态
/// 规则的模板。三者里的类名此时都还是占位符，所以指纹与类名之间没有循环。
///
/// **不喂**插值表达式的文本。`css!{ color: $(a) }` 与 `css!{ color: $(b) }` 的产物
/// 都是 `.slx-x { color: var(--slx-x-0) }`——差别由各自元素上的行内自定义属性承担，
/// 共用一个类名是对的，也正是这条口径能省下最多的重复注入。
///
/// `prefix` 参与哈希：它决定产物落进哪个 `@layer`（见 `compile_block_internal`
/// 里的 `layer_name`），同样的声明在 components 层和 utilities 层不是一回事。
fn fingerprint(prefix: &str, state: &ParserState) -> String {
    use core::hash::{Hash, Hasher};

    let mut hasher = silex_hash::css::CssHasher::new();
    prefix.hash(&mut hasher);
    state.static_css.hash(&mut hasher);
    state.lifted_css.hash(&mut hasher);
    for rule in &state.dynamic_rules {
        rule.template.hash(&mut hasher);
    }
    let mut buf = [0u8; 13];
    silex_hash::css::encode_base36(hasher.finish(), &mut buf).to_string()
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
                // 属性名与静态取值都要过一遍校验：此前静态声明完全绕开类型系统，
                // `colr: red`、`color: 10px` 都是编译通过、无警告、产物错误
                let validate = state.validate && !state.is_unsafe;
                if validate {
                    crate::css::table::resolve_property_type(&decl.property, decl.span)?;
                }

                state.static_css.push_str(&decl.property);
                state.static_css.push_str(": ");

                let prop_for_expr = if state.is_unsafe {
                    "any"
                } else {
                    &decl.property
                };
                let expr_count_before = state.expressions.len();
                let val = extract_dynamic_value(
                    &decl.values,
                    &mut state.expressions,
                    &mut state.warnings,
                    prop_for_expr,
                    &ctx,
                )?;

                // 取值里没有插值时才校验：有插值的取值文本里只剩
                // `var(--cls-0)` 占位符，没什么可查的，插值本身的类型由
                // `ValidFor` 在展开产物里管
                if validate && state.expressions.len() == expr_count_before {
                    // 裸关键字 / 函数式取值 / 分量个数三层判据。放在定型断言
                    // **之前**：`width: 1 0px` 的分量个数不对，但下面那一步会
                    // 先把空白折掉、再把它认成一个合法的 `10px`
                    crate::css::value_check::check_static_value(
                        &decl.property,
                        &val,
                        // 取值的错误要指到取值上。`decl.span` 是属性名的位置，
                        // 拿它报「`centre` 不是合法取值」会把箭头画在
                        // `align-items` 底下，读者第一反应是属性名写错了
                        value_span(&decl.values).unwrap_or(decl.span),
                        &mut state.warnings,
                    )?;

                    // 整条取值就是一个能定型的字面量时，生成一条编译期断言，
                    // 交给 `ValidFor` 回答「这个值类型对这个属性合法吗」
                    if let Some(value_type) = classify_static_value(&val) {
                        state.assertions.push(StaticAssertion {
                            property: decl.property.clone(),
                            value_type,
                            span: decl.span,
                        });
                    }
                }

                state.static_css.push_str(&val);
                // 分号无条件补上，不看源码里写没写。块内最后一条声明的分号
                // 在 CSS 里可有可无，产物经 lightningcss 最小化后完全一样；
                // 但它此前会留在中间产物里，让 `color: red` 与 `color: red;`
                // 落到两个类名。这是「按产物去重」的另一半。
                state.static_css.push_str("; ");
            }
            CssRule::Apply(ap) => {
                #[cfg(feature = "tw")]
                {
                    let raw_str = ap.classes.trim().trim_matches('"');
                    let anchor = crate::css::tw::parser::TokenAnchor::whole(raw_str, ap.span);
                    let rules = crate::css::tw::parser::parse_class_list(&anchor, &mut Vec::new())?;
                    let apply_block = crate::css::tw::codegen::build_css_block_from_rules(rules)?;
                    // `@apply` 展开出来的声明是机器生成的（含 `--tw-*` 与厂商前缀），
                    // 不该拿用户书写的那套判据去卡
                    let old = state.validate;
                    state.validate = false;
                    let result = process_css_block(&apply_block, state);
                    state.validate = old;
                    result?;
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
                    assertions: Vec::new(),
                    class_name: state.class_name.clone(),
                    is_unsafe: state.is_unsafe,
                    // 这几条 at-rule 的块里装的是**描述符**（`src`、`system`、
                    // `inherits`），不是 CSS 属性，拿属性注册表去卡它们只会
                    // 把合法写法判成拼写错误
                    validate: state.validate && !is_descriptor_at_rule(&at.name),
                    region: state.region.clone(),
                };

                process_css_block(at_block, &mut inner_state)?;

                // Sync back state
                state.expressions = inner_state.expressions;
                state.warnings = inner_state.warnings;
                state.assertions.extend(inner_state.assertions);
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
                // 与静态那一侧同理，见 `process_css_block`
                template.push_str("; ");
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
            // 其余控制字符也一律转义。除了本来就该这么写，这还保证了
            // `PLACEHOLDER_CLASS` / `PLACEHOLDER_VALUE` /
            // `PLACEHOLDER_PENDING_CLASS` 这几个占位符不可能从用户的
            // 字符串字面量里冒出来
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\{:X} ", c as u32))
            }
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
                        out.push(PLACEHOLDER_VALUE);
                        exprs.push(("any".to_string(), g.stream()));
                        iter.next();
                        return Ok(true);
                    }
                    if let Some(path) = handle_dollar_path(iter)? {
                        check_unexpected_complex_tokens(iter)?;
                        if space_before {
                            out.push(' ');
                        }
                        out.push(PLACEHOLDER_VALUE);
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
                    // 类名留成占位符：运行时那一轮用的是带哈希后缀的动态类名，
                    // 此前是先写基类名、再 `res.replace(基类名, 动态类名)`——
                    // 规则里同时存在 `.foo` 与 `.foo-bar` 时后者会被一起改掉
                    out.push('.');
                    out.push(PLACEHOLDER_CLASS);
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
    let first_expr = exprs.len();
    let value = process_tokens(
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
    )?;

    // 只有当插值**就是整条取值**时，它才能按该属性的类型来校验。
    // `grid-template-columns: repeat($(columns), minmax(0, 1fr))` 里的
    // `$(columns)` 是取值里的一个片段，它的类型跟属性本身的取值类型没有关系，
    // 拿属性去卡它只会报出无从下手的错误。片段一律按 `props::Any` 处理。
    let sole_value =
        exprs.len() == first_expr + 1 && value.trim() == placeholder(first_expr).as_str();
    if !sole_value {
        for (prop, _) in exprs.iter_mut().skip(first_expr) {
            "any".clone_into(prop);
        }
    }

    Ok(value)
}

/// 取值的第一个 token 的位置。
///
/// `Span::join` 只在 nightly 可用，拿不到「整条取值」的范围，所以取第一个
/// token——箭头落在取值的开头，比落在属性名上准得多。
fn value_span(values: &TokenStream) -> Option<Span> {
    values.clone().into_iter().next().map(|tt| tt.span())
}

/// 块内装的是描述符而不是 CSS 属性的 at-rule。
///
/// `@font-face { src: … }` 里的 `src` 不是属性，`@property { inherits: … }`
/// 的 `inherits` 也不是；属性注册表对它们一无所知。
fn is_descriptor_at_rule(name: &str) -> bool {
    matches!(
        name,
        "font-face"
            | "font-palette-values"
            | "font-feature-values"
            | "counter-style"
            | "property"
            | "page"
            | "viewport"
            | "position-try"
    )
}

/// 判断一条静态取值是否是「一眼能定型」的字面量，是则给出对应的 CSS 值类型名。
///
/// 只认三类：带单位的数值、百分比、十六进制颜色——这三类能直接对上
/// `silex_css::types` 里的一个类型，交给 `ValidFor` 判定即可。
///
/// 关键字（`red`、`auto`）、函数（`rgb(…)`）、多分量取值（`1px solid red`）
/// 返回 `None`：它们在 Rust 侧没有单一的对应类型，改由 `css::value_check` 拿
/// MDN 语法表直接判（见那里的三层判据）。
///
/// 特意不认裸数字：`0` 在 CSS 里是合法长度，但 `i32` 并不是 `ValidFor<Width>`，
/// 认了就会把 `width: 0` 这种正常写法判成错误。
fn classify_static_value(value: &str) -> Option<&'static str> {
    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return None;
    }

    if let Some(digits) = compact.strip_prefix('#') {
        return if matches!(digits.len(), 3 | 4 | 6 | 8)
            && digits.chars().all(|c| c.is_ascii_hexdigit())
        {
            Some("Hex")
        } else {
            None
        };
    }

    // 数值前缀 + 单位后缀
    let split = compact
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+'))
        .map(|(i, _)| i)?;
    let (num, unit) = compact.split_at(split);
    if num.is_empty() || num.parse::<f64>().is_err() {
        return None;
    }
    // 与 `silex_css::types::units` 里的单位一一对应。少一个不会出错，只是
    // 那种写法退回「不定型、不校验」——所以加新单位时记得同步这里。
    match unit {
        // 长度
        "px" => Some("Px"),
        "rem" => Some("Rem"),
        "em" => Some("Em"),
        "ch" => Some("Ch"),
        "ex" => Some("Ex"),
        "vw" => Some("Vw"),
        "vh" => Some("Vh"),
        "vmin" => Some("Vmin"),
        "vmax" => Some("Vmax"),
        "dvw" => Some("Dvw"),
        "dvh" => Some("Dvh"),
        "svw" => Some("Svw"),
        "svh" => Some("Svh"),
        "lvw" => Some("Lvw"),
        "lvh" => Some("Lvh"),
        "pt" => Some("Pt"),
        "pc" => Some("Pc"),
        "cm" => Some("Cm"),
        "mm" => Some("Mm"),
        "in" => Some("In"),
        "Q" => Some("Qmm"),
        "%" => Some("Percent"),
        // 网格轨道
        "fr" => Some("Fr"),
        // 角度
        "deg" => Some("Deg"),
        "rad" => Some("Rad"),
        "turn" => Some("Turn"),
        // 时间
        "s" => Some("Sec"),
        "ms" => Some("Ms"),
        _ => None,
    }
}

/// 默认浏览器基线。
///
/// 此前硬编码的是 chrome 80 / safari 13 / firefox 75，而运行时实际要求高得多：
///
/// | 依赖 | 最低版本 |
/// | --- | --- |
/// | `document.adoptedStyleSheets` + `new CSSStyleSheet()`（主注入路径） | Chrome 73 / Safari 16.4 / Firefox 101 |
/// | `@layer`（层序声明无条件输出） | Chrome 99 / Safari 15.4 / Firefox 97 |
/// | `color-mix()`（`CssVar::alpha`） | Chrome 111 / Safari 16.2 / Firefox 113 |
///
/// 声明的 Safari 13 目标根本跑不起来，lightningcss 为此做的降级（`::before`
/// → `:before` 之类）全是无用功。默认值现在取上表的上界。
const DEFAULT_TARGETS: &[(&str, u32)] = &[
    ("chrome", 111 << 16),
    ("safari", (16 << 16) | (4 << 8)),
    ("firefox", 113 << 16),
];

fn get_compiler_targets() -> Targets {
    static CACHE: std::sync::OnceLock<Targets> = std::sync::OnceLock::new();
    *CACHE.get_or_init(|| {
        let configured = crate::css::config::get_config()
            .map(|c| &c.css.targets)
            .filter(|t| !t.is_empty());

        let browsers = match configured {
            Some(t) => match parse_browsers(t) {
                Ok(b) => b,
                // 配置写错了不能静默退回默认值——那等于把用户写的基线当没看见
                Err(msg) => panic!("silex.toml `[css.targets]`：{msg}"),
            },
            None => {
                let mut b = lightningcss::targets::Browsers::default();
                for (name, version) in DEFAULT_TARGETS {
                    set_browser(&mut b, name, *version).expect("默认基线的浏览器名是合法的");
                }
                b
            }
        };

        Targets {
            browsers: Some(browsers),
            // 基线抬到 Chrome 111 / Safari 16.4 之后，媒体查询的区间语法
            // （`(width >= 768px)`）就在支持范围内了，lightningcss 会按那种
            // 形式打印。它不比 `(min-width: 768px)` 做得更多，却让产物与
            // Tailwind 的写法对不上——tw 的差分测试正是靠这个对齐的。
            // `include` = 无论目标是否支持都降级成 `min-`/`max-` 形式。
            include: lightningcss::targets::Features::MediaRangeSyntax
                | lightningcss::targets::Features::MediaIntervalSyntax,
            ..Targets::default()
        }
    })
}

/// 把 `[css.targets]` 解析成 lightningcss 的 `Browsers`。
fn parse_browsers(
    table: &std::collections::HashMap<String, String>,
) -> std::result::Result<lightningcss::targets::Browsers, String> {
    let mut browsers = lightningcss::targets::Browsers::default();
    let mut names: Vec<&String> = table.keys().collect();
    names.sort();
    for name in names {
        let raw = &table[name];
        let version = parse_version(raw).ok_or_else(|| {
            format!("`{name} = \"{raw}\"` 不是合法的版本号（形如 `16` 或 `16.4`）")
        })?;
        set_browser(&mut browsers, name, version)?;
    }
    Ok(browsers)
}

/// `"16.4"` → `16 << 16 | 4 << 8`，这是 lightningcss 的版本编码。
fn parse_version(raw: &str) -> Option<u32> {
    let mut parts = raw.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    let patch: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() || major > 0xffff || minor > 0xff || patch > 0xff {
        return None;
    }
    Some((major << 16) | (minor << 8) | patch)
}

fn set_browser(
    browsers: &mut lightningcss::targets::Browsers,
    name: &str,
    version: u32,
) -> std::result::Result<(), String> {
    let slot = match name {
        "android" => &mut browsers.android,
        "chrome" => &mut browsers.chrome,
        "edge" => &mut browsers.edge,
        "firefox" => &mut browsers.firefox,
        "ie" => &mut browsers.ie,
        "ios_saf" | "ios_safari" => &mut browsers.ios_saf,
        "opera" => &mut browsers.opera,
        "safari" => &mut browsers.safari,
        "samsung" => &mut browsers.samsung,
        other => {
            return Err(format!(
                "`{other}` 不是可识别的浏览器名（可用：android、chrome、edge、\
                 firefox、ie、ios_saf、opera、safari、samsung）"
            ));
        }
    };
    *slot = Some(version);
    Ok(())
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

    // --- P2-3：层级归属 ---
    //
    // 这条优先级链此前既没写进文档，也没有任何断言保护：`base` 一条规则都没有，
    // 而 `sty()`（在 `silex_css::builder`）与 `global!` 完全不带 layer——按规范
    // 无层规则压过所有具名层，于是全局重置反而盖过每一个组件样式。

    #[test]
    fn css_lands_in_the_utilities_layer() {
        let css = compile_all("color: red;");
        assert!(css.contains("@layer utilities{"), "{css}");
    }

    #[test]
    fn styled_lands_in_the_components_layer() {
        let res = CssCompiler::compile_with_source_and_prefix(
            "color: red;",
            "slx-st-",
            Span::call_site(),
        )
        .unwrap();
        assert!(res.component_css.contains("@layer components{"), "{res:?}");
    }

    /// 变体（`declare_variants!`）与 `styled!` 同层
    #[test]
    fn variant_classes_land_in_the_components_layer() {
        let res = CssCompiler::compile_with_source_and_prefix(
            "color: red;",
            "slx-twv-",
            Span::call_site(),
        )
        .unwrap();
        assert!(res.component_css.contains("@layer components{"), "{res:?}");
    }

    #[test]
    fn global_lands_in_the_base_layer() {
        let res = CssCompiler::compile_global_with_source(
            "body { color: red; }",
            Span::call_site(),
            false,
        )
        .unwrap();
        assert!(res.component_css.contains("@layer base{"), "{res:?}");
        assert!(res.component_css.contains("body"), "{res:?}");
    }

    /// 从组件里提升出来的 `@font-face` / `@keyframes` 不套 layer——它们本来就
    /// 不属于那个组件，套进 `components` 只会让同名字体/动画的解析多一层层序
    #[test]
    fn lifted_at_rules_stay_outside_any_layer() {
        let res = CssCompiler::compile_with_source(
            "@font-face { font-family: \"X\"; } color: red;",
            Span::call_site(),
            false,
        )
        .unwrap();
        assert!(res.static_css.contains("@font-face"), "{res:?}");
        assert!(!res.static_css.contains("@layer"), "{res:?}");
        assert!(res.component_css.contains("@layer utilities{"), "{res:?}");
    }

    // --- P2-4：浏览器基线 ---

    #[test]
    fn version_strings_parse_into_lightningcss_encoding() {
        assert_eq!(parse_version("111"), Some(111 << 16));
        assert_eq!(parse_version("16.4"), Some((16 << 16) | (4 << 8)));
        assert_eq!(parse_version("1.2.3"), Some((1 << 16) | (2 << 8) | 3));
        assert_eq!(parse_version(" 16.4 "), Some((16 << 16) | (4 << 8)));
        assert_eq!(parse_version("16.4.5.6"), None);
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn unknown_browser_names_are_rejected_instead_of_ignored() {
        let mut table = std::collections::HashMap::new();
        table.insert("chorme".to_string(), "111".to_string());
        let err = parse_browsers(&table).unwrap_err();
        assert!(err.contains("chorme"), "{err}");
    }

    #[test]
    fn configured_targets_land_in_the_right_slots() {
        let mut table = std::collections::HashMap::new();
        table.insert("safari".to_string(), "16.4".to_string());
        table.insert("ios_safari".to_string(), "16.4".to_string());
        let b = parse_browsers(&table).unwrap();
        assert_eq!(b.safari, Some((16 << 16) | (4 << 8)));
        assert_eq!(b.ios_saf, Some((16 << 16) | (4 << 8)));
        assert_eq!(b.chrome, None);
    }

    /// 默认基线必须真的能跑起来：`adoptedStyleSheets` 是唯一注入路径，
    /// Safari 要到 16.4 才有；此前声明的是 Safari 13。
    #[test]
    fn the_default_baseline_can_actually_run_the_runtime() {
        let browsers = get_compiler_targets().browsers.unwrap();
        assert!(browsers.safari.unwrap() >= (16 << 16) | (4 << 8));
        assert!(browsers.chrome.unwrap() >= 111 << 16);
        assert!(browsers.firefox.unwrap() >= 113 << 16);
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
        let parts = template_parts(&res.dynamic_rules[0].template);
        assert_eq!(
            parts[..2],
            [TemplatePart::Lit(".x ".into()), TemplatePart::Val(0)],
            "{parts:?}"
        );
    }

    /// `$sel .x` 是后代选择器，不是字段访问
    #[test]
    fn dynamic_selector_can_be_followed_by_a_descendant() {
        let res =
            CssCompiler::compile_with_source("$sel .x { color: red; }", Span::call_site(), false)
                .unwrap();
        assert_eq!(res.dynamic_rules.len(), 1);
        let parts = template_parts(&res.dynamic_rules[0].template);
        assert_eq!(parts[0], TemplatePart::Val(0), "{parts:?}");
        assert!(
            matches!(&parts[1], TemplatePart::Lit(s) if s.starts_with(" .x")),
            "{parts:?}"
        );
    }

    /// 模板里的类名是**占位符**，不是基类名文本。
    ///
    /// 报告 P2-8：此前 `&` 展开成 `.slx-st-xxx`，运行时再
    /// `res.replace(".slx-st-xxx", ".slx-st-xxx-dyn-h")`——规则里同时存在
    /// `.foo` 与 `.foo-bar` 时，后者会被改成 `.foo-dyn-h-bar`。
    #[test]
    fn the_component_class_is_a_placeholder_not_literal_text() {
        let res = CssCompiler::compile_with_source(
            "& $sel .foo-bar { color: red; }",
            Span::call_site(),
            false,
        )
        .unwrap();
        assert_eq!(res.dynamic_rules.len(), 1);
        let template = &res.dynamic_rules[0].template;
        assert!(
            !template.contains(&res.class_name),
            "模板里不该出现基类名文本：{template:?}"
        );
        let parts = template_parts(template);
        assert_eq!(
            parts[..3],
            [
                TemplatePart::Lit(".".into()),
                TemplatePart::Class,
                TemplatePart::Lit(" ".into())
            ],
            "{parts:?}"
        );
    }

    /// 用户字符串里的控制字符会被转义，占位符不可能被伪造出来
    #[test]
    fn control_characters_in_string_literals_cannot_forge_a_placeholder() {
        let css = compile_all("content: \"a\\u{1}b\\u{2}c\";");
        assert!(!css.contains(PLACEHOLDER_CLASS), "{css:?}");
        assert!(!css.contains(PLACEHOLDER_VALUE), "{css:?}");
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
        let parts = template_parts(&res.dynamic_rules[0].template);
        assert_eq!(
            parts[..2],
            [TemplatePart::Lit(".x ".into()), TemplatePart::Val(0)],
            "{parts:?}"
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

    // --- 缺口 A：静态取值的三层校验（判据本身在 `css::value_check`）---
    //
    // 这里测的是「判据接进了编译流程」，而不是判据本身对不对：`value_check`
    // 的单测直接调 `check_static_value`，证明不了 `compile` 真的会因此失败。

    #[test]
    fn a_misspelled_keyword_fails_compilation() {
        assert!(compile_err("align-items: centre;").contains("`center`"));
    }

    #[test]
    fn a_color_function_on_a_keyword_property_fails_compilation() {
        assert!(compile_err("align-items: rgb(0 0 0);").contains("`rgb()`"));
    }

    #[test]
    fn a_multi_component_value_on_a_single_value_property_fails_compilation() {
        assert!(compile_err("color: 1px solid red;").contains("只接受单个取值"));
    }

    /// **逃生口**：`unsafe { … }` 块必须绕过全部三层。
    ///
    /// 这是 MDN 数据滞后时用户唯一不需要改配置就能用的出口，一旦失效，
    /// 收紧校验就变成了「有些合法 CSS 再也写不出来」
    #[test]
    fn an_unsafe_block_bypasses_all_three_layers() {
        let css = compile_all(
            "unsafe { align-items: centre; color: 1px solid red; z-index: rgb(0 0 0); }",
        );
        assert!(css.contains("centre"), "{css}");
        assert!(css.contains("1px solid red"), "{css}");
    }

    /// 插值取值不参与静态校验：取值文本里只剩 `var(--…)` 占位符。
    ///
    /// `$(…)` 本身的类型由展开产物里的 `ValidFor` 管——这里要确认的是三层
    /// 判据不会先一步把它判成「不认识的函数 `var()`」或「多分量」
    #[test]
    fn interpolated_values_skip_the_static_layers() {
        let ts = syn::parse_str("align-items: $(v);").unwrap();
        assert!(CssCompiler::compile(ts, Span::call_site(), false).is_ok());
        let ts = syn::parse_str("color: $(a) $(b);").unwrap();
        assert!(CssCompiler::compile(ts, Span::call_site(), false).is_ok());
    }

    /// 描述符 at-rule 的块里装的不是 CSS 属性，整块不校验
    #[test]
    fn descriptor_at_rules_are_not_value_checked() {
        let css = compile_all("@font-face { font-family: MyFont; src: url(a.woff2); }");
        assert!(css.contains("@font-face"), "{css}");
    }

    /// `!important` 是优先级标记，不能被数成一个取值分量
    #[test]
    fn important_survives_the_arity_check() {
        let css = compile_all("color: red !important;");
        assert!(css.contains("important"), "{css}");
    }

    // --- 缺口 E：类名按产物去重，不按源码文本 ---

    fn class_of(src: &str) -> String {
        CssCompiler::compile_with_source(src, Span::call_site(), false)
            .unwrap()
            .class_name
    }

    /// 写法不同、产物相同 → 同一个类名。
    ///
    /// 否则产物里会有两条一模一样的规则，各占一个类名各注入一次。
    #[test]
    fn writing_style_does_not_change_the_class_name() {
        let canonical = class_of("color: red;");
        for src in [
            "color:red;",
            "color: red",
            "  color : red ; ",
            "color:red",
            "color:\n    red;\n",
        ] {
            assert_eq!(
                canonical,
                class_of(src),
                "{src:?} 应当与 `color: red;` 同名"
            );
        }
    }

    /// 但声明顺序仍然区分——CSS 里后写的赢，那是两段不同的样式
    #[test]
    fn declaration_order_still_changes_the_class_name() {
        assert_ne!(
            class_of("color: red; width: 1px;"),
            class_of("width: 1px; color: red;")
        );
    }

    /// 字符串字面量逐字参与身份：大小写、内部空白都不能被折掉。
    ///
    /// 这正是「哈希产物」而不是「哈希规范化后的源码」的理由——按空白折叠去哈希
    /// 会把 `"a  b"` 和 `"a b"` 判成同一段，于是两段不同的 CSS 抢同一个类名，
    /// 后注入的那份被 `inject_style` 按 id 丢掉，其中一处直接显示错的内容。
    #[test]
    fn string_literals_participate_in_the_identity_verbatim() {
        assert_ne!(class_of("content: \"A\";"), class_of("content: \"a\";"));
        assert_ne!(
            class_of("content: \"a  b\";"),
            class_of("content: \"a b\";")
        );
    }

    /// 嵌套块与 at-rule 一样按产物去重
    #[test]
    fn nested_rules_and_at_rules_dedupe_by_product_too() {
        assert_eq!(
            class_of("&:hover { color: red; }"),
            class_of("&:hover{color:red}")
        );
        assert_eq!(
            class_of("@media (min-width: 600px) { color: red; }"),
            class_of("@media (min-width: 600px){color:red}")
        );
    }

    /// 插值表达式不参与身份：产物都是 `var(--<cls>-0)`，差别由元素上的
    /// 行内自定义属性承担，共用一个类名是对的
    #[test]
    fn interpolated_expressions_do_not_change_the_class_name() {
        assert_eq!(class_of("color: $(a);"), class_of("color: $(b);"));
        // 但插值的**位置**变了就是另一段 CSS
        assert_ne!(class_of("color: $(a);"), class_of("width: $(a);"));
    }

    /// 层不同就是两段不同的样式：同样的声明落进 components 与 utilities
    /// 的优先级不一样，不能共用类名
    #[test]
    fn the_layer_is_part_of_the_identity() {
        let utilities = CssCompiler::compile_with_source("color: red;", Span::call_site(), false)
            .unwrap()
            .class_name;
        let components = CssCompiler::compile_with_source_and_prefix(
            "color: red;",
            "slx-st-",
            Span::call_site(),
        )
        .unwrap()
        .class_name;
        assert_ne!(
            utilities.trim_start_matches("slx-tw-"),
            components.trim_start_matches("slx-st-")
        );
    }

    /// 类名占位符必须被逐字换回真名，一个都不能漏进产物
    #[test]
    fn the_pending_class_placeholder_never_reaches_the_product() {
        let res = CssCompiler::compile_with_source(
            "color: $v; &:hover { color: red; } @keyframes k { 0% { top: $t; } } $sel & { left: 0; }",
            Span::call_site(),
            false,
        )
        .unwrap();
        for css in [&res.static_css, &res.component_css] {
            assert!(!css.contains(PLACEHOLDER_PENDING_CLASS), "{css:?}");
        }
        for rule in &res.dynamic_rules {
            assert!(
                !rule.template.contains(PLACEHOLDER_PENDING_CLASS),
                "{:?}",
                rule.template
            );
        }
        // 换回去的确实是这次算出来的类名
        assert!(
            res.component_css.contains(&format!(".{}", res.class_name)),
            "{:?}",
            res.component_css
        );
        assert!(
            res.static_css
                .contains(&format!("var(--{}-", res.class_name)),
            "{:?}",
            res.static_css
        );
    }

    /// 同一段产物 → 同一个注入 id。`inject_style` 按 id 去重，这是
    /// 「少注入一次」真正生效的地方
    #[test]
    fn the_same_product_lands_on_the_same_style_id() {
        let a = CssCompiler::compile_with_source("color: red", Span::call_site(), false).unwrap();
        let b = CssCompiler::compile_with_source("color:red;", Span::call_site(), false).unwrap();
        assert_eq!(a.style_id, b.style_id);
        assert_eq!(a.component_css, b.component_css);
    }
}
