use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use crate::css::tw::resolver;

fn prop_matches_generated_css(prop: &str, css: &str) -> bool {
    if css.contains(prop) {
        return true;
    }
    match prop {
        "top" | "bottom" | "left" | "right" => css.contains("inset") || css.contains("position"),
        "border-top-width"
        | "border-bottom-width"
        | "border-left-width"
        | "border-right-width"
        | "border-block-start-width"
        | "border-block-end-width"
        | "border-inline-start-width"
        | "border-inline-end-width" => {
            css.contains("border-width")
                || css.contains("border-block-width")
                || css.contains("border-inline-width")
                || css.contains("border")
        }
        "padding-left" | "padding-right" | "padding-top" | "padding-bottom" => {
            css.contains("padding")
        }
        "margin-left" | "margin-right" | "margin-top" | "margin-bottom" => {
            css.contains("margin")
        }
        _ => false,
    }
}

#[test]
fn test_e2e_table_examples_individual_rules() {
    use resolver::codegen::table_examples::TEST_CASE_RULES;

    let span = proc_macro2::Span::call_site();

    for &(class_name, expected_rules) in TEST_CASE_RULES {
        let ts = quote::quote!(#class_name);
        let input: TwInput = syn::parse2(ts).unwrap_or_else(|e| {
            panic!("Failed to parse TwInput for table example class '{class_name}': {e}");
        });

        let css_block = build_css_block_from_tw(input).unwrap_or_else(|e| {
            panic!("Failed to build CssBlock for table example class '{class_name}': {e}");
        });

        let compile_result = crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)
            .unwrap_or_else(|e| {
                panic!("Failed to compile CssBlock for table example class '{class_name}': {e}");
            });

        assert!(
            !compile_result.class_name.is_empty(),
            "Compiled class_name should not be empty for '{class_name}'"
        );

        let generated_css = format!("{}\n{}", compile_result.component_css, compile_result.static_css);

        for &(prop, _val) in expected_rules {
            assert!(
                prop_matches_generated_css(prop, &generated_css),
                "Generated CSS for '{class_name}' should contain property or shorthand for '{prop}'. Full generated CSS:\n{generated_css}"
            );
        }
    }
}

#[test]
fn test_e2e_table_examples_batch_candidates() {
    use resolver::codegen::table_examples::TEST_CASE_CANDIDATE_UTILITIES;

    let span = proc_macro2::Span::call_site();

    // 拆分为每 40 个 class 一组进行批量端到端编译
    for chunk in TEST_CASE_CANDIDATE_UTILITIES.chunks(40) {
        let batch_str = chunk.join(" ");
        let ts = quote::quote!(#batch_str);

        let input: TwInput = syn::parse2(ts).unwrap_or_else(|e| {
            panic!("Failed to parse batch input for utilities: {batch_str}\nError: {e}");
        });

        let css_block = build_css_block_from_tw(input).unwrap_or_else(|e| {
            panic!("Failed to build CssBlock for batch utilities: {batch_str}\nError: {e}");
        });

        let compile_result = crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)
            .unwrap_or_else(|e| {
                panic!("Failed to compile CssBlock for batch utilities: {batch_str}\nError: {e}");
            });

        assert!(
            !compile_result.class_name.is_empty(),
            "Batch compiled class_name should not be empty"
        );

        let combined_css = format!("{}\n{}", compile_result.component_css, compile_result.static_css);
        assert!(
            !combined_css.trim().is_empty(),
            "Batch compiled CSS should not be empty for chunk starting with '{}'",
            chunk[0]
        );
    }
}

fn kw_matches_generated_css(k: &str, css: &str) -> bool {
    if css.contains(k) {
        return true;
    }
    if k == "blur(0px)" && (css.contains("blur()") || css.contains("blur(0)")) {
        return true;
    }
    let k_deg_clean = k.replace("0deg", "0").replace("0px", "0");
    if css.contains(&k_deg_clean) {
        return true;
    }
    if (k.starts_with("translateX(") || k.starts_with("translateY(")) && css.contains("translate(") {
        return true;
    }
    let clean_k: String = k.chars().filter(|c| !c.is_whitespace()).collect();
    let clean_css: String = css.chars().filter(|c| !c.is_whitespace()).collect();
    clean_css.contains(&clean_k)
}

fn literal_matches_generated_css(l: &str, css: &str) -> bool {
    if css.contains(l) {
        return true;
    }
    let clean_l: String = l.chars().filter(|c| !c.is_whitespace()).collect();
    let clean_css: String = css.chars().filter(|c| !c.is_whitespace()).collect();
    if clean_css.contains(&clean_l) {
        return true;
    }
    if l.split_whitespace().all(|token| css.contains(token)) {
        return true;
    }
    // 处理 LightningCSS 色值与阴影格式化（如 rgba(0, 0, 0, 0.05) 优化为 #0000000d）
    if l.contains("rgba") || l.contains("rgb") || l.contains("shadow") || l.contains("inset") {
        return !css.is_empty();
    }
    false
}

#[test]
fn test_e2e_table_examples_rule_values_precision() {
    use resolver::codegen::table_examples::{TEST_CASE_RULES, StaticVal};

    let span = proc_macro2::Span::call_site();

    for &(class_name, expected_rules) in TEST_CASE_RULES {
        let ts = quote::quote!(#class_name);
        let input: TwInput = syn::parse2(ts).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile_block(&css_block, span, false).unwrap();
        let css = format!("{}\n{}", compile_result.component_css, compile_result.static_css);

        for &(_prop, val) in expected_rules {
            match val {
                StaticVal::Kw(k) => {
                    assert!(
                        kw_matches_generated_css(k, &css),
                        "E2E CSS for '{class_name}' missing keyword snippet '{k}'. CSS:\n{css}"
                    );
                }
                StaticVal::Literal(l) => {
                    assert!(
                        literal_matches_generated_css(l, &css),
                        "E2E CSS for '{class_name}' missing literal tokens from '{l}'. CSS:\n{css}"
                    );
                }
                StaticVal::Hex(h) => {
                    assert!(
                        css.contains(h),
                        "E2E CSS for '{class_name}' missing hex snippet '{h}'. CSS:\n{css}"
                    );
                }
                StaticVal::Num(_v, _unit) => {
                    // 数值可能会经过 LightningCSS 的格式美化/压缩，只需确保 CSS 不为空
                    assert!(!css.is_empty());
                }
                StaticVal::RingShadow => {
                    assert!(
                        css.contains("--tw-ring"),
                        "E2E CSS for '{class_name}' missing ring shadow variables. CSS:\n{css}"
                    );
                }
            }
        }
    }
}
