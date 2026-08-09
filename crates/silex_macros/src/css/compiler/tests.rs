use super::*;
use proc_macro2::Span;

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
    let res =
        CssCompiler::compile_with_source_and_prefix("color: red;", "slx-st-", Span::call_site())
            .unwrap();
    assert!(res.component_css.contains("@layer components{"), "{res:?}");
}

/// 变体（`declare_variants!`）与 `styled!` 同层
#[test]
fn variant_classes_land_in_the_components_layer() {
    let res =
        CssCompiler::compile_with_source_and_prefix("color: red;", "slx-twv-", Span::call_site())
            .unwrap();
    assert!(res.component_css.contains("@layer components{"), "{res:?}");
}

#[test]
fn global_lands_in_the_base_layer() {
    let res =
        CssCompiler::compile_global_with_source("body { color: red; }", Span::call_site(), false)
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
    let res = CssCompiler::compile_with_source(".x $sel { color: red; }", Span::call_site(), false)
        .unwrap();
    assert_eq!(res.dynamic_rules.len(), 1);
    let parts = template_parts(&res.dynamic_rules[0].template);
    assert_eq!(
        parts[..2],
        [
            TemplatePart::Lit(".x ".into()),
            TemplatePart::SelectorVal(0)
        ],
        "{parts:?}"
    );
}

/// `$sel .x` 是后代选择器，不是字段访问
#[test]
fn dynamic_selector_can_be_followed_by_a_descendant() {
    let res = CssCompiler::compile_with_source("$sel .x { color: red; }", Span::call_site(), false)
        .unwrap();
    assert_eq!(res.dynamic_rules.len(), 1);
    let parts = template_parts(&res.dynamic_rules[0].template);
    assert_eq!(parts[0], TemplatePart::SelectorVal(0), "{parts:?}");
    assert!(
        matches!(&parts[1], TemplatePart::Lit(s) if s.starts_with(" .x")),
        "{parts:?}"
    );
}

#[test]
fn template_tokens_preserve_selector_and_declaration_contexts() {
    let template = format!(
        "color: {} ; .scope {} {{ color: {} }}",
        PLACEHOLDER_VALUE, PLACEHOLDER_SELECTOR_VALUE, PLACEHOLDER_VALUE
    );
    let tokens = template_parts_tokens(&template).to_string();

    assert!(tokens.contains("CssPart :: Val (0usize)"), "{tokens}");
    assert!(
        tokens.contains("CssPart :: SelectorVal (1usize)"),
        "{tokens}"
    );
    assert!(tokens.contains("CssPart :: Val (2usize)"), "{tokens}");
}

#[test]
fn dynamic_selector_validates_property_names() {
    let err = compile_err("$selector { colr: red; }");
    assert!(err.contains("`colr` 不存在"), "{err}");
}

#[test]
fn dynamic_selector_validates_static_values() {
    assert!(compile_err("$selector { align-items: centre; }").contains("`center`"));
    assert!(compile_err("$selector { align-items: rgb(0 0 0); }").contains("`rgb()`"));
    assert!(compile_err("$selector { color: 1px solid red; }").contains("只接受单个取值"));
}

#[test]
fn dynamic_selector_collects_static_type_assertions() {
    let res =
        CssCompiler::compile_with_source("$selector { color: 10px; }", Span::call_site(), false)
            .unwrap();
    assert_eq!(res.assertions.len(), 1);
    assert_eq!(res.assertions[0].property, "color");
    assert_eq!(res.assertions[0].value_type, "Px");
}

#[test]
fn dynamic_selector_preserves_unsafe_and_generated_css_exceptions() {
    assert!(
        CssCompiler::compile_with_source(
            "unsafe { $selector { colr: red; } }",
            Span::call_site(),
            false,
        )
        .is_ok()
    );

    #[cfg(feature = "tw")]
    assert!(
        CssCompiler::compile_with_source(
            "$selector { @apply flex items-center; }",
            Span::call_site(),
            false,
        )
        .is_ok()
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
        parts[..4],
        [
            TemplatePart::Lit(".".into()),
            TemplatePart::Class,
            TemplatePart::Lit(" ".into()),
            TemplatePart::SelectorVal(0),
        ],
        "{parts:?}"
    );
}

#[test]
fn a_class_placeholder_does_not_consume_selector_indices() {
    let template = format!(
        ".{}{} {}{}",
        PLACEHOLDER_CLASS,
        PLACEHOLDER_SELECTOR_VALUE,
        PLACEHOLDER_SELECTOR_VALUE,
        PLACEHOLDER_CLASS
    );
    assert_eq!(
        template_parts(&template),
        [
            TemplatePart::Lit(".".into()),
            TemplatePart::Class,
            TemplatePart::SelectorVal(0),
            TemplatePart::Lit(" ".into()),
            TemplatePart::SelectorVal(1),
            TemplatePart::Class,
        ]
    );
}

/// 用户字符串里的控制字符会被转义，占位符不可能被伪造出来
#[test]
fn control_characters_in_string_literals_cannot_forge_a_placeholder() {
    let css = compile_all("content: \"a\\u{1}b\\u{2}c\";");
    assert!(!css.contains(PLACEHOLDER_CLASS), "{css:?}");
    assert!(!css.contains(PLACEHOLDER_VALUE), "{css:?}");
    assert!(!css.contains(PLACEHOLDER_SELECTOR_VALUE), "{css:?}");
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
        let res = CssCompiler::compile_global_with_source(src, Span::call_site(), false).unwrap();
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
        [
            TemplatePart::Lit(".x ".into()),
            TemplatePart::SelectorVal(0)
        ],
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
    let css =
        compile_all("unsafe { align-items: centre; color: 1px solid red; z-index: rgb(0 0 0); }");
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
    let components =
        CssCompiler::compile_with_source_and_prefix("color: red;", "slx-st-", Span::call_site())
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
