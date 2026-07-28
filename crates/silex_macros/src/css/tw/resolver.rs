pub mod arbitrary;
pub mod codegen;
pub mod core_bridge;
pub mod numeric;
pub mod palette;
pub mod suggest;

use std::{fmt::Display, result::Result as StdResult};

use proc_macro2::Span;
use smallvec::SmallVec;
use syn::{Error, Result};

use crate::css::tw::{
    ast::{ModifierList, SpannedModifier, UtilityRule, UtilityValue},
    resolver::{
        arbitrary::{parse_arbitrary_syntax, resolve_arbitrary},
        codegen::{property_id::CssPropertyId, table::resolve_static_rule},
        core_bridge::{MacroCtx, at_rule_utility_to_rules, rule_sets_to_rules, with_selector},
        numeric::resolve_numeric_utility,
        palette::resolve_color_rules,
        suggest::find_best_suggestion,
    },
};

use silex_tw_core::{
    lookup_at_rule_utility,
    prefix::{ColorPrefixRule, lookup_color_prefix},
};

// ring 的 box-shadow 载体由 `silex_tw_core` 定义，这里只是转出常量，
// 避免宏侧再抄一份字面量。
pub use silex_tw_core::RING_BOX_SHADOW;

#[inline]
pub(super) fn kw(s: &'static str) -> UtilityValue {
    UtilityValue::Keyword(s)
}

#[inline]
pub(super) fn num(v: f64, u: &'static str) -> UtilityValue {
    UtilityValue::Numeric(v, u)
}

#[inline]
pub(super) fn num_unitless(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "")
}

#[inline]
pub(super) fn rem(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "rem")
}

#[inline]
pub(super) fn px(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "px")
}

pub trait IntoCssPropertyId: Sized {
    fn into_css_property_id(self) -> StdResult<CssPropertyId, Self>;
}

impl IntoCssPropertyId for CssPropertyId {
    #[inline]
    fn into_css_property_id(self) -> StdResult<CssPropertyId, Self> {
        Ok(self)
    }
}

impl IntoCssPropertyId for &str {
    #[inline]
    fn into_css_property_id(self) -> StdResult<CssPropertyId, Self> {
        CssPropertyId::parse(self).ok_or(self)
    }
}

impl<'a> IntoCssPropertyId for &'a &'a str {
    #[inline]
    fn into_css_property_id(self) -> StdResult<CssPropertyId, Self> {
        CssPropertyId::parse(self).ok_or(self)
    }
}

pub trait IntoModifierList {
    fn into_modifier_list(self) -> ModifierList;
}

impl IntoModifierList for ModifierList {
    #[inline]
    fn into_modifier_list(self) -> ModifierList {
        self
    }
}

impl IntoModifierList for &[SpannedModifier] {
    #[inline]
    fn into_modifier_list(self) -> ModifierList {
        self.iter().cloned().collect()
    }
}

impl IntoModifierList for Vec<SpannedModifier> {
    #[inline]
    fn into_modifier_list(self) -> ModifierList {
        SmallVec::from_vec(self)
    }
}

pub fn make_rule<P>(
    modifiers: impl IntoModifierList,
    prop: P,
    value: UtilityValue,
    span: Span,
) -> Result<UtilityRule>
where
    P: IntoCssPropertyId + Display,
{
    let css_property = prop.into_css_property_id().map_err(|unsupported| {
        Error::new(
            span,
            format!(
                "CSS Property '{}' is not registered in CssPropertyId table",
                unsupported
            ),
        )
    })?;

    Ok(UtilityRule {
        modifiers: modifiers.into_modifier_list(),
        css_property,
        value,
        // `!important` 是词条级别的标记，由 `parser.rs` 在解析完整个词条后统一打上
        important: false,
        is_default_line_height: false,
        span,
    })
}

/// 判断是否为 Marker Class（`group` / `peer`，含 `group/name` 这类命名形式）。
///
/// Marker class 的定义是"必须以**字面类名**出现在 DOM 上，好让别的选择器引用它"——
/// `group-hover:` 生成的选择器里写死了 `.group`，那个类名不进 `class` 属性就永远匹配不上。
///
/// 容器查询不属于这一类：`@container` / `@container/card` 的 `container-type` 与
/// `container-name` 都是**声明**，落在 `tw!` 生成的哈希类上就够了。此前它们也被塞进
/// `extra_classes`，于是 `class` 属性里会出现 `@container`、`container/side` 这种
/// 连合法 CSS 类名都不是的字符串（`@` 与 `/` 在选择器里必须转义）。
#[inline]
pub fn is_marker_class(token: &str) -> bool {
    let base = match token.split_once('/') {
        Some((prefix, _)) => prefix,
        None => token,
    };
    matches!(base, "group" | "peer")
}

/// 将基础的 Utility 词条（如 `p-4`, `hover:bg-primary`, `w-[12px]`）解析为标准的 `UtilityRule`
pub fn resolve_utility(
    modifiers: Vec<SpannedModifier>,
    utility_token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let base = match utility_token.split_once('/') {
        Some((prefix, _)) => prefix,
        None => utility_token,
    };
    if matches!(base, "group" | "peer") {
        return Ok(vec![]);
    }

    // 1. at-rule 分组型工具类（`container` / `outline-hidden`）。
    //    必须排在静态表之前：静态表一行挂不了 `@media`，真让它先命中就等于
    //    永远拿不到条件分组——那正是报告 §3.1 说的"另一份实现是死代码"。
    let mut rules = if let Some(meta) = lookup_at_rule_utility(utility_token) {
        at_rule_utility_to_rules(&modifiers, meta, span)?
    } else if let Some(rules) = resolve_static_rule(&modifiers, utility_token, span) {
        rules
    } else {
        resolve_pattern_utility(modifiers, utility_token, span)?
    };

    // 若当前词条同时展开出了 FontSize 与 LineHeight（如 `text-sm`），
    // 且不是显式指定行高的斜杠简写（如 `text-sm/6`），
    // 则该 LineHeight 是字号档位自带的默认行高，标注 is_default_line_height = true。
    let is_slash_shorthand = utility_token.contains('/') && utility_token.starts_with("text-");
    if !is_slash_shorthand {
        let has_font_size = rules
            .iter()
            .any(|r| r.css_property == CssPropertyId::FontSize);
        let has_line_height = rules
            .iter()
            .any(|r| r.css_property == CssPropertyId::LineHeight);
        if has_font_size && has_line_height {
            for rule in rules.iter_mut() {
                if rule.css_property == CssPropertyId::LineHeight {
                    rule.is_default_line_height = true;
                }
            }
        }
    }

    Ok(rules)
}

/// 用共享的颜色前缀规则展开一个「值已经算好」的颜色声明。
///
/// 供 `theme()` 与任意值两条路径复用：它们的**值**是 Silex 特有的
/// （`ThemeVar` / `DynamicExpr`），但"前缀映射到哪些属性、要不要补 `box-shadow`、
/// 声明落在哪个选择器上"这三件事必须与普通颜色路径完全一致，所以统一读 core 的表。
pub(crate) fn expand_color_prefix_rule(
    modifiers: &[SpannedModifier],
    rule: &ColorPrefixRule,
    value: UtilityValue,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let mods = with_selector(modifiers, rule.selector, span);

    let mut rules = Vec::with_capacity(rule.props.len() + 2);
    for &prop in rule.props {
        rules.push(make_rule(mods.clone(), prop, value.clone(), span)?);
    }
    if let Some(companion) = rule.companion {
        for &(prop, val) in companion.decls() {
            rules.push(make_rule(mods.clone(), prop, kw(val), span)?);
        }
    }
    Ok(rules)
}

/// 解析前缀规律型 Utility (如 `p-4`, `mt-2`, `w-16`, `bg-theme(primary)`, `text-slate-900`, `bg-indigo-600/50`, `w-[12px]`)
fn resolve_pattern_utility(
    modifiers: Vec<SpannedModifier>,
    token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    // 0. 处理命名容器 marker class (例: @container/card-header, container/sidebar)
    if let Some(c_name) = token
        .strip_prefix("@container/")
        .or_else(|| token.strip_prefix("container/"))
    {
        return Ok(vec![
            make_rule(
                modifiers.clone(),
                "container-name",
                UtilityValue::ArbitraryLiteral(c_name.to_string()),
                span,
            )?,
            make_rule(modifiers, "container-type", kw("inline-size"), span)?,
        ]);
    }

    // 1. Theme 变量, 如 `bg-theme(primary)` / `text-theme(border)` / `bg-theme(primary/50)`
    if let Some((prefix, theme_var, opacity)) = parse_theme_var(token) {
        let Some(rule) = lookup_color_prefix(prefix) else {
            return Err(Error::new(
                span,
                format!("Unsupported theme prefix: '{}'", prefix),
            ));
        };
        return expand_color_prefix_rule(
            &modifiers,
            rule,
            UtilityValue::ThemeVar(theme_var.to_string(), opacity),
            span,
        );
    }

    // 2. 字号 / 行高简写 `text-sm/6`、`text-[14px]/[1.5]`
    //    必须排在颜色路径之前：`text-red-500/50` 与 `text-sm/6` 形状完全相同，
    //    唯一的区别是斜杠前那段解析出来是颜色还是字号。
    if let Some(rules) = resolve_font_size_with_leading(&modifiers, token, span) {
        return rules;
    }

    // 3. 颜色型 Utility：色板色阶、`/透明度`、`[#hex]`、语义 token，
    //    以及 ring / 渐变色标 / divide / placeholder 的伴生声明与伴生选择器。
    //    前缀表与展开规则全在 `silex_tw_core`，与静态表同源。
    if let Some(rules) = resolve_color_rules(&modifiers, token, span) {
        return rules;
    }

    // 4. 任意值与动态表达式语法, 如 `w-[100px]`、`-mt-[10px]` 或 `p-[$(pad_val)]`
    if let Some((prefix, raw_val)) = parse_arbitrary_syntax(token) {
        // 负号必须在查前缀**之前**剥掉：此前 `-mt-[10px]` 会拿 `-mt` 去查
        // `CssPropertyId`，报的是"属性 '-mt' 未注册"这种内部错误（报告 §2.7）
        let (prefix, negate) = match prefix.strip_prefix('-') {
            Some(rest) if !rest.is_empty() => (rest, true),
            _ => (prefix, false),
        };
        return resolve_arbitrary(modifiers, prefix, raw_val, negate, span);
    }

    // 5. 交给 `silex_tw_core` 的完整解析器。
    //
    //    静态表只预计算了 `classes.json` 里那 22 879 个类名；`rounded-t-7`、`p-97`
    //    这类同样有规律、只是没被 Tailwind 列举出来的词条不在表里，但 core 解析得了。
    //    过去它们落到下面的 `resolve_numeric_utility`——那是一份与 core 平行的实现，
    //    `rounded-*-sm` 在 core 侧早已修正为 v4 的 0.25rem，宏这份却还留着 v3 的
    //    0.125rem，只因静态表优先命中才没暴露。走 core 之后这类分叉不可能再出现。
    if let Some(sets) = silex_tw_core::resolve_class(token, &MacroCtx) {
        return rule_sets_to_rules(&modifiers, sets, span);
    }

    // 6. 数值、分数 (1/2, 1/3) 与方向边距/定位 Utility 解析（core 未覆盖的规律）
    if let Some(rules) = resolve_numeric_utility(&modifiers, token, span) {
        return Ok(rules);
    }

    // 7. Levenshtein 智能纠错与建议
    let suggestion = find_best_suggestion(token);
    let msg = match suggestion {
        Some(s) => format!(
            "Unknown or unsupported Utility class '{}'. Did you mean '{}'?",
            token, s
        ),
        None => format!("Unknown or unsupported Utility class '{}'.", token),
    };

    Err(Error::new(span, msg))
}

/// `text-<字号>/<行高>`：Tailwind 的字号与行高简写（`text-sm/6`、`text-[14px]/[1.5]`）
///
/// 与 `text-red-500/50`（颜色 + 不透明度）形状完全相同，只能靠**斜杠前那段解析出来是不是
/// 字号**来区分，所以这里直接复用 `resolve_utility` 求值再看属性，而不是另写一套
/// "什么样的后缀算字号"的判别——后者一定会与静态表和 core 漂移。
/// 行高同理：`leading-6` / `leading-[1.5]` 已经有唯一实现，拼出词条交给它即可。
fn resolve_font_size_with_leading(
    modifiers: &[SpannedModifier],
    token: &str,
    span: Span,
) -> Option<Result<Vec<UtilityRule>>> {
    let (head, leading) = token.rsplit_once('/')?;
    if !head.starts_with("text-") {
        return None;
    }
    // 斜杠落在任意值内部（`text-[calc(10px/2)]`）时括号不成对，不是简写语法
    if head.matches('[').count() != head.matches(']').count() {
        return None;
    }
    if leading.is_empty() {
        return None;
    }

    let head_rules = resolve_utility(modifiers.to_vec(), head, span).ok()?;
    if !head_rules
        .iter()
        .any(|r| r.css_property == CssPropertyId::FontSize)
    {
        return None;
    }

    let leading_token = format!("leading-{}", leading);
    let leading_rules = match resolve_utility(modifiers.to_vec(), &leading_token, span) {
        Ok(rules) => rules,
        Err(_) => {
            return Some(Err(Error::new(
                span,
                format!(
                    "Unknown line-height '{}' in '{}'. The part after `/` must be a valid `leading-*` value, e.g. `text-sm/6` or `text-[14px]/[1.5]`.",
                    leading, token
                ),
            )));
        }
    };

    // 字号档位自带的行高（`text-sm` → `line-height: 1.25rem`）被显式写出的那个替换掉
    let mut rules: Vec<UtilityRule> = head_rules
        .into_iter()
        .filter(|r| r.css_property != CssPropertyId::LineHeight)
        .collect();
    rules.extend(
        leading_rules
            .into_iter()
            .filter(|r| r.css_property == CssPropertyId::LineHeight),
    );
    Some(Ok(rules))
}

fn parse_theme_var(token: &str) -> Option<(&str, &str, Option<f64>)> {
    if let Some((prefix, rest)) = token.split_once("-theme(")
        && let Some(inner) = rest.strip_suffix(')')
    {
        if let Some((var_name, op_str)) = inner.split_once('/')
            && let Ok(op) = op_str.parse::<f64>()
        {
            return Some((prefix, var_name, Some(op)));
        }
        return Some((prefix, inner, None));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn test_resolve_pattern_numeric_rules() {
        let span = Span::call_site();

        // 1. 单属性规则 (rem 缩放)
        let rules = resolve_utility(vec![], "p-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "padding");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.0, "rem"));

        let rules = resolve_utility(vec![], "-mt-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "margin-top");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-0.5, "rem"));

        // 2. 双属性规则 (对称方向)
        let rules = resolve_utility(vec![], "px-6", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "padding-left");
        assert_eq!(rules[1].css_property, "padding-right");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.5, "rem"));
        assert_eq!(rules[1].value, UtilityValue::Numeric(1.5, "rem"));

        let rules = resolve_utility(vec![], "size-8", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(rules[1].css_property, "height");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, "rem"));

        // 3. 自定义数值计算与转换规则
        let rules = resolve_utility(vec![], "grid-cols-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "grid-template-columns");
        assert_eq!(
            rules[0].value,
            UtilityValue::Keyword("repeat(4, minmax(0, 1fr))")
        );

        let rules = resolve_utility(vec![], "opacity-50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "opacity");
        assert_eq!(rules[0].value, UtilityValue::Numeric(0.5, ""));

        let rules = resolve_utility(vec![], "rotate-45", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("rotate(45deg)"));

        let rules = resolve_utility(vec![], "-rotate-90", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("rotate(-90deg)"));

        let rules = resolve_utility(vec![], "bg-theme(primary)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), None)
        );

        let rules = resolve_utility(vec![], "bg-theme(primary/50)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), Some(50.0))
        );

        // 4. Hex 颜色解析规则
        let rules = resolve_utility(vec![], "bg-[#1e293b]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#1e293b".into()));

        let rules = resolve_utility(vec![], "text-[#818cf8]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#818cf8".into()));

        // 5. 通用任意值语法解析规则
        let rules = resolve_utility(vec![], "w-[100px]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("100px".into())
        );

        // 6. Levenshtein 拼写纠错测试
        let err = resolve_utility(vec![], "flexx", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'flex'?"));

        let err = resolve_utility(vec![], "items-centerr", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'items-center'?"));

        // 7. Phase 4: Container Query Utilities
        let rules = resolve_utility(vec![], "@container", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-type");
        assert_eq!(rules[0].value, UtilityValue::Keyword("inline-size"));

        let rules = resolve_utility(vec![], "container-[sidebar]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-name");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("sidebar".into())
        );

        // 8. Phase 5: Standard Color Palette & Opacity Suffix Rules
        let rules = resolve_utility(vec![], "text-slate-900", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("oklch(20.8% 0.042 265.755)".into())
        );

        let rules = resolve_utility(vec![], "bg-indigo-600/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral(
                "color-mix(in oklab, oklch(51.1% 0.262 276.966) 50%, transparent)".into()
            )
        );

        let rules = resolve_utility(vec![], "border-emerald-500/25", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral(
                "color-mix(in oklab, oklch(69.6% 0.17 162.48) 25%, transparent)".into()
            )
        );

        let rules = resolve_utility(vec![], "border-t-rose-500", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("oklch(64.5% 0.246 16.439)".into())
        );

        let rules = resolve_utility(vec![], "bg-white/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral(
                "color-mix(in oklab, oklch(100% 0 0) 50%, transparent)".into()
            )
        );
    }

    #[test]
    fn test_new_fractional_and_directional_features() {
        let span = Span::call_site();

        // 分数宽度测试
        let rules = resolve_utility(vec![], "w-1/2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(50.0, "%"));

        let rules = resolve_utility(vec![], "h-1/3", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "height");
        if let UtilityValue::Numeric(val, unit) = &rules[0].value {
            assert_eq!(*unit, "%");
            assert!((val - 33.333333333333336).abs() < 1e-6);
        } else {
            panic!("Expected Numeric");
        }

        // 分数 translate
        let rules = resolve_utility(vec![], "-translate-x-1/2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("translateX(-50%)"));

        // 定位与负 inset
        let rules = resolve_utility(vec![], "-top-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "top");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-1.0, "rem"));

        let rules = resolve_utility(vec![], "inset-x-0", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "left");
        assert_eq!(rules[1].css_property, "right");

        // 方向 Border 宽度
        let rules = resolve_utility(vec![], "border-t-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, "px"));

        let rules = resolve_utility(vec![], "border-x-4", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "border-left-width");
        assert_eq!(rules[1].css_property, "border-right-width");

        // 完全透明度 /0 测试
        let rules = resolve_utility(vec![], "bg-black/0", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral(
                "color-mix(in oklab, oklch(0% 0 0) 0%, transparent)".into()
            )
        );

        // 新扩充静态词条测试
        let rules = resolve_utility(vec![], "max-w-xs", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "max-width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(20.0, "rem"));

        let rules = resolve_utility(vec![], "text-5xl", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "font-size");
        assert_eq!(rules[0].value, UtilityValue::Numeric(3.0, "rem"));

        let rules = resolve_utility(vec![], "italic", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "font-style");
        assert_eq!(rules[0].value, UtilityValue::Keyword("italic"));

        // 多列与分栏 Break 规则测试
        // 列数与列宽都走 `columns` 简写，与 Tailwind 一致
        let rules = resolve_utility(vec![], "columns-4xl", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "columns");
        assert_eq!(rules[0].value, UtilityValue::Numeric(56.0, "rem"));

        let rules = resolve_utility(vec![], "columns-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "columns");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, ""));

        let rules = resolve_utility(vec![], "break-inside-avoid-flex", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "break-inside");
        assert_eq!(rules[0].value, UtilityValue::Keyword("avoid-flex"));

        // z-index, box-decoration & isolation 规则测试
        let rules = resolve_utility(vec![], "z-50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Numeric(50.0, ""));

        let rules = resolve_utility(vec![], "-z-10", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-10.0, ""));

        let rules = resolve_utility(vec![], "z-auto", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Keyword("auto"));

        let rules = resolve_utility(vec![], "box-decoration-slice", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "box-decoration-break");
        assert_eq!(rules[0].value, UtilityValue::Keyword("slice"));

        let rules = resolve_utility(vec![], "isolate", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "isolation");
        assert_eq!(rules[0].value, UtilityValue::Keyword("isolate"));

        // Outline 规则测试 (outline-1, outline-ring)
        let rules = resolve_utility(vec![], "outline-1", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "outline-style");
        assert_eq!(rules[0].value, UtilityValue::Keyword("solid"));
        assert_eq!(rules[1].css_property, "outline-width");
        assert_eq!(rules[1].value, UtilityValue::Numeric(1.0, "px"));

        let rules = resolve_utility(vec![], "outline-ring", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "outline-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("var(--ring)".into())
        );
    }
}
