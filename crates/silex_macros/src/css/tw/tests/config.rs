use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

#[test]
fn test_silex_config_deserialization() {
    let toml_content = r##"
    [theme]
    dark_mode = "media"

    [theme.colors]
    brand-primary = "#6366f1"

    [theme.dark_colors]
    brand-primary = "#818cf8"

    [theme.breakpoints]
    "3xl" = "1920px"
    "##;

    let cfg: crate::css::config::SilexConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(cfg.theme.dark_mode.as_deref(), Some("media"));
    assert_eq!(cfg.theme.colors.get("brand-primary").unwrap(), "#6366f1");
    assert_eq!(
        cfg.theme.dark_colors.get("brand-primary").unwrap(),
        "#818cf8"
    );
    assert_eq!(cfg.theme.breakpoints.get("3xl").unwrap(), "1920px");
}

#[test]
fn test_custom_design_tokens_from_silex_toml() {
    let input: TwInput = syn::parse2(quote!("bg-brand-primary 3xl:p-12")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let compile_result = crate::css::compiler::CssCompiler::compile(
        quote! { #css_block },
        proc_macro2::Span::call_site(),
        false,
    )
    .unwrap();

    assert!(
        compile_result.component_css.contains("#6366f1")
            || compile_result.component_css.contains("background-color:")
    );
    assert!(
        compile_result.component_css.contains("1920px")
            || compile_result.component_css.contains("media")
    );
}
