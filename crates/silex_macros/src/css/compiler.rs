//! # CSS 编译器模块 (`silex_macros::css::compiler`)
//!
//! 本模块负责将 `css!`, `styled!`, `tw!`, `global!` 等宏解析出的 CSS AST（[`CssBlock`](crate::css::ast::CssBlock)）
//! 编译为最终可在运行时注入的 CSS 字符串、动态模板及 Rust 表达式。
//!
//! 为了降低单文件复杂性，该模块按职责划分为以下子模块：
//!
//! - [`types`]: **类型与常量定义**。包含 [`CssCompileResult`](types::CssCompileResult)、[`DynamicRule`](types::DynamicRule)、
//!   [`ParserState`](types::ParserState) 等编译中间/结果数据结构，以及 `@layer` 层级常量与占位符字符。
//! - [`tokens`]: **Token 处理与空白恢复**。包含 Token 游标 [`CssTokens`](tokens::CssTokens)、源码精确空白恢复、
//!   字面量转义（[`escape_css_string`](tokens::escape_css_string)）及表达式解析辅助。
//! - [`targets`]: **浏览器基线配置**。基于 LightningCSS 的浏览器兼容目标配置与版本号解析逻辑。
//! - [`parser`]: **AST 转换与编译核心**。包含 [`process_css_block`](parser::process_css_block)、动态选择器提取
//!   与静态属性校验的核心递归解析流程。
//! - [`tests`][]: 单元测试套件。
//!
//! ## 主入口 [`CssCompiler`]
//!
//! [`CssCompiler`] 结构体提供了各种 compile 入口方法（如 [`compile`](CssCompiler::compile),
//! [`compile_block`](CssCompiler::compile_block), [`compile_global`](CssCompiler::compile_global) 等）。
//! 编译主逻辑采用两阶段解环：先使用占位符完成结构转换并对中间产物取哈希计算类名指纹，
//! 再将真实类名替换回最终 CSS 产物中。

use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use proc_macro2::{Span, TokenStream};
use syn::Result;

use crate::css::ast::CssBlock;

pub mod parser;
pub mod targets;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod tests;

pub(crate) use parser::*;
pub(crate) use targets::*;
pub(crate) use tokens::*;
pub use types::*;

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
        use std::rc::Rc;
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
        use std::rc::Rc;
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
        use std::rc::Rc;
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
                // `global!` receives a whole CSS token stream rather than a single
                // declaration/value span. The call-site source text also contains the
                // macro invocation itself, so using it for whitespace recovery can make
                // the reconstructed global rule differ from the parsed token stream.
                // Global selectors still use the conservative token-shape fallback.
                region: None,
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
