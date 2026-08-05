pub mod ast;
pub mod classes;
pub mod compiler;
pub mod config;
pub mod error;
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
use quote::{quote, quote_spanned};
use syn::Result;

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

    Ok(quote! {
        {
            #assertions
            const __STATIC_CSS: &str = #static_css;
            const __COMPONENT_CSS: &str = #component_css;

            if !__STATIC_CSS.is_empty() {
                #__silex::css::inject_style(#static_id, __STATIC_CSS);
            }
            if !__COMPONENT_CSS.is_empty() {
                #__silex::css::inject_style(#style_id, __COMPONENT_CSS);
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
                if let Some(span) = first_dynamic_token_span(&decl.values) {
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
                if first_dynamic_token_span(&at.params).is_some() {
                    return Err(syn::Error::new(at.span, value_message));
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

pub fn css_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site(); // Use call site for better error reporting in blocks
    let compile_result = CssCompiler::compile(ts, span, false)?;
    generate_css_output(compile_result, span)
}

pub(crate) fn generate_css_output(
    compile_result: compiler::CssCompileResult,
    span: Span,
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
    let static_inits = quote! {
        if !#static_css.is_empty() {
            #__silex::css::inject_style(#static_id, #static_css);
        }
        if !#component_css.is_empty() {
            #__silex::css::inject_style(#style_id, #component_css);
        }
    };

    // Generate Rust Code
    if expressions.is_empty() && dynamic_rules.is_empty() {
        Ok(quote! {
            {
                #common_inits
                #static_inits
                #class_name
            }
        })
    } else {
        // Generate DynamicCss struct
        let mut var_calls = Vec::new();
        for (i, (prop, expr)) in expressions.iter().enumerate() {
            let var_name = format!("--{}-{}", class_name, i);
            let prop_type = get_prop_type(prop, span)?;
            var_calls.push(quote! {
                .with_var::<#prop_type, _>(#var_name, #expr)
            });
        }

        let mut rule_calls = Vec::new();
        for rule in dynamic_rules {
            let parts = compiler::template_parts_tokens(&rule.template);
            let mut exprs = Vec::new();
            for (prop, expr) in &rule.expressions {
                let prop_type = get_prop_type(prop, span)?;
                exprs.push(quote! { #__silex::css::make_property_val::<#prop_type, _>(#expr) });
            }
            rule_calls.push(quote! {
                .with_rule(#parts, ::std::vec![ #(#exprs),* ])
            });
        }

        Ok(quote! {
            {
                #common_inits
                #__silex::css::DynamicCss::new(#class_name)
                    .with_layer(#layer)
                    .with_static_style(#static_id, #static_css)
                    .with_static_style(#style_id, #component_css)
                    #(#var_calls)*
                    #(#rule_calls)*
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
    fn inject_css_rejects_dynamic_selector() {
        let err = inject_css_impl(quote! { $selector { color: red; } })
            .unwrap_err()
            .to_string();
        assert!(err.contains("只接受纯静态 CSS"), "{err}");
        assert!(err.contains("动态选择器"), "{err}");
    }

    #[test]
    fn dynamic_css_carries_static_styles_into_its_owner_bound_payload() {
        let output = css_impl(quote! { color: $(color); }).unwrap().to_string();
        assert!(output.contains("with_static_style"), "{output}");
        assert!(!output.contains("inject_style"), "{output}");
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
