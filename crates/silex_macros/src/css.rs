pub mod ast;
pub mod classes;
pub mod compiler;
pub mod config;
pub mod error;
pub(crate) mod html_tag;
pub mod property_caps;
pub mod property_keywords;
pub mod property_names;
pub mod spacing;
pub mod styled;
pub mod table;
pub mod theme;
#[cfg(feature = "tw")]
pub mod tw;
pub mod value_check;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote, quote_spanned};
use syn::{Expr, Result, Token, parse::Parse, parse::ParseStream};

use ast::{CssBlock, CssRule};
use compiler::CssCompiler;
use table::PropertyResolveResult;

pub(crate) fn get_prop_type(prop: &str, span: Span) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    match table::resolve_property_type(prop, span)? {
        PropertyResolveResult::Builtin(type_name) => {
            let ident = syn::Ident::new(&type_name, Span::call_site());
            Ok(quote_spanned! { span => #__silex::css::types::props::#ident })
        }
        PropertyResolveResult::Untyped => {
            Ok(quote_spanned! { span => #__silex::css::types::props::Any })
        }
    }
}

/// 把静态声明的类型断言展开成代码。
///
/// 每条断言就是一次「这个值类型对这个属性合法吗」的实例化，不合法时由
/// `ValidFor` 的 `#[diagnostic::on_unimplemented]` 给出可读报错，位置指向
/// 源码里那条声明。
pub(crate) fn generate_static_assertions(
    assertions: &[compiler::StaticAssertion],
) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let mut out = TokenStream::new();
    for a in assertions {
        let prop_ty = get_prop_type(&a.property, a.span)?;
        let value_ty = syn::Ident::new(a.value_type, a.span);
        let span = a.span;
        out.extend(quote_spanned! { span =>
            const _: () = {
                fn __silex_css_assert_valid<
                    __P,
                    __T: #__silex::css::types::ValidFor<__P>,
                >() {}
                let _ = __silex_css_assert_valid::<
                    #prop_ty,
                    #__silex::css::types::#value_ty,
                >;
            };
        });
    }
    Ok(out)
}

/// 生成静态插值的值绑定。
///
/// 静态插值必须同时满足三条边界：表达式在 const 上下文中可求值、结果实现
/// `StaticCssValue`，并且结果类型满足当前 CSS 属性的 `ValidFor`。绑定只求值一次，
/// 后续模板渲染和 global replacement 都复用同一份字符串。
pub(crate) fn generate_static_value_bindings(
    expressions: &[(String, TokenStream)],
    span: Span,
    prefix: &str,
) -> Result<(TokenStream, Vec<syn::Ident>)> {
    let __silex = crate::crate_path::silex();
    let mut declarations = TokenStream::new();
    let mut values = Vec::with_capacity(expressions.len());

    for (index, (property, expression)) in expressions.iter().enumerate() {
        let value_ident = quote::format_ident!("{prefix}_{index}");
        let prop_type = get_prop_type(property, span)?;
        let expression_span = expression
            .clone()
            .into_iter()
            .next()
            .map(|token| token.span())
            .unwrap_or(span);

        declarations.extend(quote_spanned! { expression_span =>
            const _: () = {
                let _ = #expression;
            };
            let #value_ident = #__silex::css::static_css_value::<#prop_type, _>(#expression);
        });
        values.push(value_ident);
    }

    Ok((declarations, values))
}

pub(crate) fn static_values_tokens(values: &[syn::Ident]) -> TokenStream {
    let rendered = values.iter().map(|value| quote! { #value.to_string() });
    quote! { ::std::vec![ #(#rendered),* ] }
}

/// 生成静态 stylesheet 的注入代码。
pub(crate) fn generate_static_style_inits(
    result: &compiler::CssCompileResult,
    static_values: Option<&syn::Ident>,
) -> TokenStream {
    let __silex = crate::crate_path::silex();
    let static_id = &result.static_id;
    let static_css = &result.static_css;
    let style_id = &result.style_id;
    let component_css = &result.component_css;

    match static_values {
        Some(values) => quote! {
            if !#static_css.is_empty() {
                let __slx_rendered_static_css =
                    #__silex::css::render_static_template(#static_css, &#values);
                #__silex::css::inject_style(#static_id, &__slx_rendered_static_css);
            }
            if !#component_css.is_empty() {
                let __slx_rendered_component_css =
                    #__silex::css::render_static_template(#component_css, &#values);
                #__silex::css::inject_style(#style_id, &__slx_rendered_component_css);
            }
        },
        None => quote! {
            if !#static_css.is_empty() {
                #__silex::css::inject_style(#static_id, #static_css);
            }
            if !#component_css.is_empty() {
                #__silex::css::inject_style(#style_id, #component_css);
            }
        },
    }
}

pub fn inject_css_impl(ts: TokenStream) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let span = Span::call_site();

    // 先在 CSS AST 上拒绝动态输入。全局编译器需要先经过 lightningcss，而
    // declaration 的动态占位符不一定是 lightningcss 可接受的最终 CSS；若等编译
    // 完成后再检查，错误可能先被解析器吞成一个无关的 CSS 语法错误。
    let block: CssBlock = syn::parse2(ts.clone())?;
    reject_dynamic_global(
        &block,
        span,
        "inject_css! 只接受纯静态 CSS；动态插值不能在文档级样式中使用。",
        "inject_css! 只接受纯静态 CSS；动态选择器不能在文档级样式中使用。",
    )?;

    let compile_result = CssCompiler::compile_global(ts, span, false)?;

    if let Some((_, expression)) = compile_result.expressions.first() {
        let expression_span = expression
            .clone()
            .into_iter()
            .next()
            .map(|token| token.span())
            .unwrap_or(span);
        return Err(syn::Error::new(
            expression_span,
            "inject_css! 只接受纯静态 CSS；动态插值不能在文档级样式中使用。",
        ));
    }
    if let Some(rule) = compile_result.dynamic_rules.first() {
        let rule_span = rule
            .expressions
            .first()
            .and_then(|(_, expression)| expression.clone().into_iter().next())
            .map(|token| token.span())
            .unwrap_or(span);
        return Err(syn::Error::new(
            rule_span,
            "inject_css! 只接受纯静态 CSS；动态选择器不能在文档级样式中使用。",
        ));
    }

    let assertions = generate_static_assertions(&compile_result.assertions)?;
    let static_id = &compile_result.static_id;
    let static_css = &compile_result.static_css;
    let style_id = &compile_result.style_id;
    let component_css = &compile_result.component_css;
    let (static_value_decls, static_values) = generate_static_value_bindings(
        &compile_result.static_expressions,
        span,
        "__slx_inject_static",
    )?;
    let static_value_tokens = static_values_tokens(&static_values);

    Ok(quote! {
        {
            #assertions
            const __STATIC_CSS: &str = #static_css;
            const __COMPONENT_CSS: &str = #component_css;
            #static_value_decls
            let __STATIC_VALUES: ::std::vec::Vec<::std::string::String> = #static_value_tokens;

            if !__STATIC_CSS.is_empty() {
                let __rendered = #__silex::css::render_static_template(
                    __STATIC_CSS,
                    &__STATIC_VALUES,
                );
                #__silex::css::inject_style(#static_id, &__rendered);
            }
            if !__COMPONENT_CSS.is_empty() {
                let __rendered = #__silex::css::render_static_template(
                    __COMPONENT_CSS,
                    &__STATIC_VALUES,
                );
                #__silex::css::inject_style(#style_id, &__rendered);
            }
        }
    })
}

pub(crate) fn reject_dynamic_global(
    block: &CssBlock,
    fallback_span: Span,
    value_message: &str,
    selector_message: &str,
) -> Result<()> {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                if let Some(span) = first_reactive_token_span(&decl.values) {
                    return Err(syn::Error::new(span, value_message));
                }
            }
            CssRule::Nested(nested) => {
                if compiler::contains_dynamic_selector(&nested.selectors) {
                    let span = first_dynamic_token_span(&nested.selectors).unwrap_or(fallback_span);
                    return Err(syn::Error::new(span, selector_message));
                }
                reject_dynamic_global(
                    &nested.block,
                    fallback_span,
                    value_message,
                    selector_message,
                )?;
            }
            CssRule::AtRule(at) => {
                if let Some(span) = first_dynamic_token_span(&at.params) {
                    return Err(syn::Error::new(span, value_message));
                }
                if let Some(block) = &at.block {
                    reject_dynamic_global(block, fallback_span, value_message, selector_message)?;
                }
            }
            CssRule::Unsafe(unsafe_rule) => {
                reject_dynamic_global(
                    &unsafe_rule.block,
                    fallback_span,
                    value_message,
                    selector_message,
                )?;
            }
            CssRule::Apply(_) => {}
        }
    }
    Ok(())
}

fn first_dynamic_token_span(ts: &TokenStream) -> Option<Span> {
    let mut iter = ts.clone().into_iter().peekable();
    while let Some(token) = iter.next() {
        if let TokenTree::Punct(punct) = &token
            && punct.as_char() == '$'
        {
            let dynamic = match iter.peek() {
                Some(TokenTree::Ident(_)) => true,
                Some(TokenTree::Group(group)) => group.delimiter() == Delimiter::Parenthesis,
                _ => false,
            };
            if dynamic {
                return Some(punct.span());
            }
        }
        if let TokenTree::Group(group) = token
            && let Some(span) = first_dynamic_token_span(&group.stream())
        {
            return Some(span);
        }
    }
    None
}

fn first_reactive_token_span(ts: &TokenStream) -> Option<Span> {
    let mut iter = ts.clone().into_iter().peekable();
    while let Some(token) = iter.next() {
        if let TokenTree::Punct(punct) = &token
            && punct.as_char() == '$'
        {
            match iter.peek() {
                Some(TokenTree::Group(group))
                    if group.delimiter() == Delimiter::Parenthesis
                        && matches!(
                            crate::css::ast::parse_interpolation(group),
                            Ok(crate::css::ast::CssInterpolation::Static(_))
                        ) => {}
                Some(TokenTree::Ident(_))
                | Some(TokenTree::Group(_))
                | Some(TokenTree::Literal(_))
                | Some(TokenTree::Punct(_)) => return Some(punct.span()),
                None => return Some(punct.span()),
            }
        }
        if let TokenTree::Group(group) = token
            && let Some(span) = first_reactive_token_span(&group.stream())
        {
            return Some(span);
        }
    }
    None
}

pub fn css_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site(); // Use call site for better error reporting in blocks
    let (error_handler, css_tokens) = parse_css_input(ts);
    let compile_result = CssCompiler::compile(css_tokens, span, false)?;
    generate_css_output(compile_result, span, error_handler)
}

struct CssInputPrefix {
    error_handler: Expr,
    _semi: Token![;],
    body: TokenStream,
}

impl Parse for CssInputPrefix {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            error_handler: input.parse()?,
            _semi: input.parse()?,
            body: input.parse()?,
        })
    }
}

fn parse_css_input(ts: TokenStream) -> (Option<TokenStream>, TokenStream) {
    match syn::parse2::<CssInputPrefix>(ts.clone()) {
        Ok(prefix) => {
            let body = unwrap_css_body_group(prefix.body);
            (Some(prefix.error_handler.into_token_stream()), body)
        }
        Err(_) => (None, ts),
    }
}

fn unwrap_css_body_group(body: TokenStream) -> TokenStream {
    let tokens: Vec<_> = body.into_iter().collect();
    if let [TokenTree::Group(group)] = tokens.as_slice()
        && group.delimiter() == Delimiter::Brace
    {
        return group.stream();
    }
    tokens.into_iter().collect()
}

pub(crate) fn generate_css_output(
    compile_result: compiler::CssCompileResult,
    span: Span,
    error_handler: Option<TokenStream>,
) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let class_name = &compile_result.class_name;
    let expressions = &compile_result.expressions;
    let dynamic_rules = &compile_result.dynamic_rules;
    let warnings = &compile_result.warnings;
    let static_id = &compile_result.static_id;
    let static_css = &compile_result.static_css;
    let style_id = &compile_result.style_id;
    let component_css = &compile_result.component_css;
    let layer = compile_result.layer;
    let has_static_values = !compile_result.static_expressions.is_empty();
    let static_values_ident = quote::format_ident!("__slx_static_values");
    let (static_value_decls, static_value_ids) = generate_static_value_bindings(
        &compile_result.static_expressions,
        span,
        "__slx_css_static",
    )?;
    let static_value_tokens = static_values_tokens(&static_value_ids);

    let warning_tokens = warnings.iter().map(|w| {
        let msg = &w.message;
        let warning_span = w.span;
        quote_spanned! { warning_span =>
            #[allow(non_upper_case_globals, dead_code)]
            #[deprecated(note = #msg)]
            const _SILEX_CSS_WARNING: () = ();
            let _ = _SILEX_CSS_WARNING;
        }
    });

    let assertions = generate_static_assertions(&compile_result.assertions)?;

    let common_inits = quote! {
        #(#warning_tokens)*
        #assertions
    };
    let static_value_inits = if has_static_values {
        quote! {
            #static_value_decls
            let #static_values_ident: ::std::vec::Vec<::std::string::String> =
                #static_value_tokens;
        }
    } else {
        quote! {}
    };
    let static_inits = if has_static_values {
        generate_static_style_inits(&compile_result, Some(&static_values_ident))
    } else {
        generate_static_style_inits(&compile_result, None)
    };
    let dynamic_static_values = if has_static_values {
        quote! { .with_static_values(#static_values_ident) }
    } else {
        quote! {}
    };

    // Generate Rust Code
    if expressions.is_empty() && dynamic_rules.is_empty() {
        Ok(quote! {
            {
                #common_inits
                #static_value_inits
                #static_inits
                #class_name
            }
        })
    } else {
        let Some(error_handler) = error_handler else {
            return Err(syn::Error::new(
                span,
                "动态 `css!` 必须显式提供 ErrorReporter 或 ErrorHandlerToken；请使用 `css!(error_handler; { ... })`",
            ));
        };
        // Generate DynamicCss struct
        let mut var_calls = Vec::new();
        for (i, (prop, expr)) in expressions.iter().enumerate() {
            let var_name = format!("--{}-{}", class_name, i);
            let prop_type = get_prop_type(prop, span)?;
            let needs_unwrap = !dynamic_rules.is_empty() || i + 1 < expressions.len();
            let question_mark = if needs_unwrap {
                quote! { ? }
            } else {
                quote! {}
            };
            var_calls.push(quote! {
                .with_var::<#prop_type, _>(#var_name, #expr, __slx_css_error_handler)
                #question_mark
            });
        }

        let mut rule_calls = Vec::new();
        for rule in dynamic_rules {
            let parts = compiler::template_parts_tokens(&rule.template);
            let mut exprs = Vec::new();
            for (prop, expr) in &rule.expressions {
                let prop_type = get_prop_type(prop, span)?;
                exprs.push(quote! {
                    #__silex::css::make_property_val::<#prop_type, _>(
                        #expr,
                        __slx_css_error_handler,
                    )?
                });
            }
            rule_calls.push(quote! {
                .with_rule(#parts, ::std::vec![ #(#exprs),* ])
            });
        }

        let dynamic_css = quote! {
            #__silex::css::DynamicCss::new(#class_name)
                .with_layer(#layer)
                #dynamic_static_values
                .with_static_style(#static_id, #static_css)
                .with_static_style(#style_id, #component_css)
                #(#var_calls)*
                #(#rule_calls)*
        };
        let dynamic_result = if dynamic_rules.is_empty() {
            quote! { #dynamic_css }
        } else {
            quote! { Ok(#dynamic_css) }
        };

        Ok(quote! {
            {
                #common_inits
                #static_value_inits
                let __slx_css_error_handler_input = #error_handler;
                let __slx_css_error_handler = #__silex::core::ErrorHandlerInput::handler_ref(
                    &__slx_css_error_handler_input,
                );
                #dynamic_result
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prop_type_custom_variable_and_builtin() {
        let span = Span::call_site();
        let builtin = get_prop_type("p", span).unwrap().to_string();
        assert!(builtin.contains("Padding"));

        let standard = get_prop_type("grid-template-columns", span)
            .unwrap()
            .to_string();
        assert!(standard.contains("GridTemplateColumns"));

        let custom_var = get_prop_type("--my-custom-color", span)
            .unwrap()
            .to_string();
        assert!(custom_var.contains("Any"));

        // 注册表之外的属性名不再被静默放行（此前会生成一个不存在的类型名，
        // 把问题推迟成宏展开产物里的 E0412），而是在宏里就报错并给出建议
        let unsupported = get_prop_type("unsupported-custom-property", span).unwrap_err();
        assert!(unsupported.to_string().contains("不存在"));

        // 厂商前缀属性 MDN 没有语法数据，仍按 `props::Any` 放行
        let vendor = get_prop_type("-webkit-font-smoothing", span)
            .unwrap()
            .to_string();
        assert!(vendor.contains("Any"));
    }

    /// 报告 P0-8：`css!{ colr: red }` 此前编译通过、无警告、浏览器丢弃
    #[test]
    fn misspelled_static_property_is_a_compile_error() {
        let err = css_impl(quote! { colr: red; }).unwrap_err().to_string();
        assert!(err.contains("`colr` 不存在"), "{err}");
        assert!(err.contains("`color`"), "{err}");
    }

    #[test]
    fn misspelled_dynamic_selector_property_is_a_compile_error() {
        let err = css_impl(quote! { $selector { colr: red; } })
            .unwrap_err()
            .to_string();
        assert!(err.contains("`colr` 不存在"), "{err}");
        assert!(err.contains("`color`"), "{err}");
    }

    /// `unsafe { … }` 仍然原样透传，不做属性名校验
    #[test]
    fn unsafe_blocks_bypass_property_validation() {
        let out = css_impl(quote! { unsafe { colr: red; } });
        assert!(
            out.is_ok(),
            "{:?}",
            out.map(|_| ()).unwrap_err().to_string()
        );
    }

    #[test]
    fn inject_css_rejects_dynamic_value() {
        let input = "color: $(color);".parse().expect("valid CSS token stream");
        let err = inject_css_impl(input).unwrap_err().to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态插值"), "{err}");
    }

    #[test]
    fn inject_css_rejects_dynamic_identifier() {
        let err = inject_css_impl(quote! { color: $color; })
            .unwrap_err()
            .to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态插值"), "{err}");
    }

    #[test]
    fn inject_css_rejects_dynamic_selector() {
        let err = inject_css_impl(quote! { $selector { color: red; } })
            .unwrap_err()
            .to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态选择器"), "{err}");
    }

    #[test]
    fn inject_css_rejects_dynamic_selector_expression() {
        let err = inject_css_impl(quote! { $(selector) { color: red; } })
            .unwrap_err()
            .to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态选择器"), "{err}");
    }

    #[test]
    fn inject_css_rejects_dynamic_at_rule_parameter() {
        let err = inject_css_impl(quote! {
            @media (min-width: $width) { color: red; }
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态插值"), "{err}");
    }

    #[test]
    fn inject_css_static_output_has_no_dynamic_runtime_payload() {
        let output = inject_css_impl(quote! { :root { color: red; } })
            .unwrap()
            .to_string();
        assert!(output.contains("inject_style"), "{output}");
        assert!(!output.contains("DynamicCss"), "{output}");
        assert!(!output.contains("RuntimeInputs"), "{output}");
        assert!(!output.contains("MountOwner"), "{output}");
    }

    #[test]
    fn dynamic_css_carries_static_styles_into_its_owner_bound_payload() {
        let output = css_impl(quote! { error_handler; color: $(color); })
            .unwrap()
            .to_string();
        assert!(output.contains("with_static_style"), "{output}");
        assert!(!output.contains("inject_style"), "{output}");
    }

    #[test]
    fn static_interpolation_generates_a_rendered_template_and_type_bound() {
        let output = css_impl(quote! { color: $(static AppTheme::PRIMARY); })
            .unwrap()
            .to_string();
        assert!(output.contains("render_static_template"), "{output}");
        assert!(output.contains("static_css_value"), "{output}");
        assert!(!output.contains("DynamicCss"), "{output}");
    }

    #[test]
    fn dynamic_css_carries_static_values_with_its_template() {
        let output = css_impl(quote! {
            error_handler;
            color: $(static AppTheme::PRIMARY);
            width: $(color);
        })
        .unwrap()
        .to_string();
        assert!(output.contains("with_static_values"), "{output}");
        assert!(output.contains("with_static_style"), "{output}");
    }

    #[test]
    fn production_global_compiler_accepts_static_nested_rules() {
        let input: TokenStream = "body { color: red; }".parse().unwrap();
        let result = CssCompiler::compile_global(input, Span::call_site(), false);
        assert!(result.is_ok(), "{result:?}");
    }

    /// `color: 10px` 此前也是静默通过：`ValidFor` 只在 `$expr` 分支起作用
    #[test]
    fn typed_static_values_are_checked_against_the_property() {
        let out = css_impl(quote! { color: 10px; }).unwrap().to_string();
        // 断言本身是编译期的，这里只验证它确实被生成了出来
        assert!(out.contains("__silex_css_assert_valid"), "{out}");
        assert!(out.contains("Px"), "{out}");
        assert!(out.contains("props :: Color"), "{out}");
    }

    /// 定不了型的取值不生成断言——宁可漏报也不能把合法 CSS 拒之门外
    #[test]
    fn untypable_static_values_produce_no_assertion() {
        for src in [
            quote! { color: red; },
            quote! { width: calc(100% - 10px); },
            quote! { margin: 1px 2px; },
            quote! { width: 0; },
        ] {
            let out = css_impl(src.clone()).unwrap().to_string();
            assert!(
                !out.contains("__silex_css_assert_valid"),
                "{src} 不该生成断言：{out}"
            );
        }
    }
}
