use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::rc::Rc;
use syn::Result;

use super::tokens::*;
use super::types::*;
use crate::css::ast::{CssBlock, CssDeclaration, CssRule};

fn validate_declaration_property(
    decl: &CssDeclaration,
    validate: bool,
    is_unsafe: bool,
) -> Result<bool> {
    let validate = validate && !is_unsafe;
    if validate {
        crate::css::table::resolve_property_type(&decl.property, decl.span)?;
    }
    Ok(validate)
}

fn validate_static_declaration_value(
    decl: &CssDeclaration,
    val: &str,
    expr_count_before: usize,
    expr_count_after: usize,
    validate: bool,
    warnings: &mut Vec<CssWarning>,
    assertions: &mut Vec<StaticAssertion>,
) -> Result<()> {
    if !validate || expr_count_after != expr_count_before {
        return Ok(());
    }

    // 裸关键字 / 函数式取值 / 分量个数三层判据。放在定型断言之前：
    // `width: 1 0px` 的分量个数不对，但下面那一步会先把空白折掉、再把它
    // 认成一个合法的 `10px`。
    crate::css::value_check::check_static_value(
        &decl.property,
        val,
        value_span(&decl.values).unwrap_or(decl.span),
        warnings,
    )?;

    // 整条取值就是一个能定型的字面量时，生成一条编译期断言，交给
    // `ValidFor` 回答「这个值类型对这个属性合法吗」。
    if let Some(value_type) = classify_static_value(val) {
        assertions.push(StaticAssertion {
            property: decl.property.clone(),
            value_type,
            span: decl.span,
        });
    }

    Ok(())
}

pub(crate) fn process_css_block(block: &CssBlock, state: &mut ParserState) -> Result<()> {
    for rule in &block.rules {
        let ctx = DynamicContext {
            class_name: &state.class_name,
            is_unsafe: state.is_unsafe,
            validate: state.validate,
            region: state.region.clone(),
        };
        match rule {
            CssRule::Declaration(decl) => {
                // 属性名与静态取值都要过一遍校验：此前静态声明完全绕开类型系统，
                // `colr: red`、`color: 10px` 都是编译通过、无警告、产物错误
                let validate =
                    validate_declaration_property(decl, state.validate, state.is_unsafe)?;

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

                validate_static_declaration_value(
                    decl,
                    &val,
                    expr_count_before,
                    state.expressions.len(),
                    validate,
                    &mut state.warnings,
                    &mut state.assertions,
                )?;

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
                        &ctx,
                        &mut state.assertions,
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

pub(crate) fn build_dynamic_template(
    nested: &crate::css::ast::CssNested,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    warnings: &mut Vec<CssWarning>,
    ctx: &DynamicContext,
    assertions: &mut Vec<StaticAssertion>,
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
        assertions,
    )?;
    template.push_str(" }");
    Ok(template)
}

pub(crate) fn build_dynamic_block_recursive(
    block: &CssBlock,
    template: &mut String,
    selector_exprs: &mut Vec<(String, TokenStream)>,
    global_expressions: &mut Vec<(String, TokenStream)>,
    warnings: &mut Vec<CssWarning>,
    ctx: &DynamicContext,
    assertions: &mut Vec<StaticAssertion>,
) -> Result<()> {
    for rule in &block.rules {
        match rule {
            CssRule::Declaration(decl) => {
                let validate = validate_declaration_property(decl, ctx.validate, ctx.is_unsafe)?;
                template.push_str(&decl.property);
                template.push_str(": ");
                let prop_for_expr = if ctx.is_unsafe { "any" } else { &decl.property };
                let expr_count_before = global_expressions.len();
                let val = extract_dynamic_value(
                    &decl.values,
                    global_expressions,
                    warnings,
                    prop_for_expr,
                    ctx,
                )?;
                validate_static_declaration_value(
                    decl,
                    &val,
                    expr_count_before,
                    global_expressions.len(),
                    validate,
                    warnings,
                    assertions,
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
                    assertions,
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
                    &DynamicContext {
                        validate: ctx.validate && !is_descriptor_at_rule(&at.name),
                        ..ctx.clone()
                    },
                    assertions,
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
                        &DynamicContext {
                            validate: false,
                            ..ctx.clone()
                        },
                        assertions,
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
                    assertions,
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
pub(crate) fn contains_dynamic_selector(ts: &TokenStream) -> bool {
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

/// at-rule 的参数（`@media (…)`、`@keyframes name`、`@supports (…)`）。
///
/// 这里**不接受**运行时值：媒体查询的条件在 CSS 里不允许出现 `var()`，
/// 之前把 `$w` 替换成 `var(--cls-0)` 的实现无论如何都产不出可用结果，
/// 只会以 `Invalid media query` 的形式炸在 lightningcss 里。直接给出可读报错。
pub(crate) fn extract_at_rule_params(
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

/// 动态选择器。选择器里的运行时片段使用独立的位置占位符，
/// 由 `styled!` / `global!` 侧按顺序填回（见 `expand_dynamic_rule`）。
pub(crate) fn extract_dynamic_selector(
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
                        out.push(PLACEHOLDER_SELECTOR_VALUE);
                        exprs.push(("any".to_string(), g.stream()));
                        iter.next();
                        return Ok(true);
                    }
                    if let Some(path) = handle_dollar_path(iter)? {
                        check_unexpected_complex_tokens(iter)?;
                        if space_before {
                            out.push(' ');
                        }
                        out.push(PLACEHOLDER_SELECTOR_VALUE);
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
pub(crate) fn extract_dynamic_value(
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
pub(crate) fn value_span(values: &TokenStream) -> Option<Span> {
    values.clone().into_iter().next().map(|tt| tt.span())
}

/// 块内装的是描述符而不是 CSS 属性的 at-rule。
///
/// `@font-face { src: … }` 里的 `src` 不是属性，`@property { inherits: … }`
/// 的 `inherits` 也不是；属性注册表对它们一无所知。
pub(crate) fn is_descriptor_at_rule(name: &str) -> bool {
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
pub(crate) fn classify_static_value(value: &str) -> Option<&'static str> {
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
