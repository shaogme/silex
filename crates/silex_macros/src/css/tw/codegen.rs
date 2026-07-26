use crate::css::{
    ast::{CssAtRule, CssBlock, CssDeclaration, CssNested, CssRule},
    config::get_config,
    tw::{
        ast::{
            Modifier, ModifierList, SpannedModifier, TwInput, TwSegment, UtilityRule, UtilityValue,
        },
        resolver::codegen::{
            keyframes::lookup_keyframe_meta, modifiers::lookup_modifier_meta,
            property_id::CssPropertyId,
        },
    },
};
use proc_macro2::{Delimiter, Group, Literal, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::{BTreeSet, HashMap};
use syn::{Result, token::Semi};

/// 将解析后的 `Vec<UtilityRule>` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_rules(rules: Vec<UtilityRule>) -> Result<CssBlock> {
    let mut root_raw_rules = Vec::new();
    let mut modifier_groups: Vec<(ModifierList, Vec<UtilityRule>)> = Vec::new();
    // BTreeSet 而非 HashSet：注入顺序直接进入产物文本与类名哈希，
    // HashSet 的迭代顺序每次构造都不同，同一份输入会编译出不同的 CSS 与类名。
    let mut detected_keyframes: BTreeSet<&'static str> = BTreeSet::new();

    for rule in rules {
        if rule.modifiers.is_empty() {
            root_raw_rules.push(rule);
        } else {
            if let Some((_, group_rules)) = modifier_groups
                .iter_mut()
                .find(|(m, _)| m == &rule.modifiers)
            {
                group_rules.push(rule);
            } else {
                modifier_groups.push((rule.modifiers.clone(), vec![rule]));
            }
        }
    }

    // 按修饰符分类与响应式断点 (min-width) 升序排序，保证样式层叠覆盖顺序符合 CSS Specificity 规范
    modifier_groups.sort_by_key(|(m, _)| modifier_group_sort_key(m));

    // 组合型属性要跨修饰符组叠加，必须在消解**之前**做（报告 §2.9）
    let mut all_groups = Vec::with_capacity(modifier_groups.len() + 1);
    all_groups.push((ModifierList::new(), root_raw_rules));
    all_groups.extend(modifier_groups);
    inherit_composable_components(&mut all_groups);

    let mut root_rules = Vec::new();
    for (modifiers, rules) in all_groups {
        let deduped = deduplicate_utility_rules(rules);
        // keyframes 只按**去重之后**真正留下的声明收集。放在去重之前收集时，
        // `animate-spin animate-[pulsar_1s]` 里那条已经被覆盖掉的 `animate-spin`
        // 照样会把 `@keyframes spin` 注入产物——而 DCE 的起点就是这个集合，
        // 它永远剪不掉自己带进来的东西。
        for rule in &deduped {
            if rule.css_property == CssPropertyId::Animation {
                collect_keyframes(&rule.value, &mut detected_keyframes);
            }
        }
        if modifiers.is_empty() {
            root_rules.extend(deduped.iter().map(convert_rule_to_declaration));
        } else {
            root_rules.push(build_modifier_rule(modifiers, deduped)?);
        }
    }

    // 按需注入 @keyframes：只有实际留在产物里的动画才有对应规则
    inject_keyframes_rules(&mut root_rules, &detected_keyframes);

    Ok(CssBlock { rules: root_rules })
}

/// 将解析后的 `TwInput` 归一化转换构建为 `silex_macros::css::ast::CssBlock`
pub fn build_css_block_from_tw(input: TwInput) -> Result<CssBlock> {
    let mut rules = Vec::new();
    for seg in input.segments {
        match seg {
            TwSegment::Static(r) => rules.extend(r),
            TwSegment::Conditional {
                then_rules,
                else_rules,
                ..
            } => {
                rules.extend(then_rules);
                rules.extend(else_rules);
            }
        }
    }
    build_css_block_from_rules(rules)
}

/// 计算单修饰符的分类权重
fn modifier_priority(m: &SpannedModifier) -> u32 {
    match &m.modifier {
        Modifier::Child | Modifier::Descendant => 10,
        Modifier::PseudoClass(_) | Modifier::PseudoElement(_) => 20,
        Modifier::SelectorVariant(_) => 25,
        Modifier::DataAttribute { .. } | Modifier::AriaAttribute { .. } => 30,
        Modifier::Group { .. } | Modifier::Peer { .. } => 40,
        Modifier::Has(_) => 50,
        Modifier::Dark => 60,
        Modifier::MediaQuery(_) => 65,
        // 条件块变体自带权重：`max-md` 与 `max-lg` 的作用区间重叠，
        // 谁覆盖谁只能由宽度决定，不能是同一个常量（见 `functional.rs`）
        Modifier::AtRuleCondition { priority, .. } => *priority,
        Modifier::ContainerQuery { .. } => 70,
        Modifier::MediaBreakpoint(bp) => {
            if let Some(meta) = lookup_modifier_meta(bp.as_str()) {
                meta.priority
            } else {
                let px = get_config()
                    .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()))
                    .and_then(|s| s.strip_suffix("px").and_then(|v| v.parse::<u32>().ok()))
                    .unwrap_or(640);
                1000 + px
            }
        }
        Modifier::CustomSelector(_) => 5000,
    }
}

/// 计算修饰符组的综合排序 Key
fn modifier_group_sort_key(modifiers: &[SpannedModifier]) -> (u32, u32, usize) {
    let max_p = modifiers.iter().map(modifier_priority).max().unwrap_or(0);
    let total_p: u32 = modifiers.iter().map(modifier_priority).sum();
    (max_p, total_p, modifiers.len())
}

/// 值可空格拼接叠加的组合型属性：同一修饰符组内的多条声明应合并而非互相覆盖
///
/// 例：`translate-x-2 translate-y-2` → `transform: translateX(.5rem) translateY(.5rem)`；
/// `blur-4 brightness-50` → `filter: blur(4px) brightness(.5)`。
#[inline]
fn is_composable_property(prop: CssPropertyId) -> bool {
    matches!(
        prop,
        CssPropertyId::Transform | CssPropertyId::Filter | CssPropertyId::BackdropFilter
    )
}

/// 让组合型属性跨修饰符组叠加（报告 §2.9 / 第四阶段第 19 项）
///
/// `hover:translate-x-2 translate-y-2` 此前产出
/// `.x { transform: translateY(.5rem) }` + `.x:hover { transform: translateX(.5rem) }`——
/// CSS 里 `transform` 是单一属性，hover 那条**整条**盖掉基础那条，Y 位移在 hover 时凭空消失。
///
/// Tailwind 用 `--tw-translate-x/y` 变量组合规避；这里选择编译期解决：把所有
/// **修饰符集合是自己真子集**的组的分量补进来（那些组在本组生效时必然也生效），
/// 补的顺序按层叠顺序，本组自己的分量最后写入，`deduplicate_utility_rules` 里
/// 按函数名 last-wins 的逻辑因此天然给出正确的覆盖结果。
///
/// 相比变量方案：产出不含运行时变量、不需要 preflight 兜底 `--tw-*` 初始值，
/// 也不会因为漏声明某个变量而出现 `translate(var(--tw-translate-x))` 解析失败。
/// 代价是补进来的分量会让声明变长——但同一属性仍只有一条声明。
///
/// 只补**本组已经用到该属性**的情形：否则 `hover:flex` 会凭空长出一条 transform。
fn inherit_composable_components(groups: &mut [(ModifierList, Vec<UtilityRule>)]) {
    // 每组自己的组合型分量快照（按属性），补入的分量不参与再次传递——
    // 真子集关系已经把所有该继承的祖先组都算进去了
    let own: Vec<Vec<UtilityRule>> = groups
        .iter()
        .map(|(_, rules)| {
            rules
                .iter()
                .filter(|r| is_composable_property(r.css_property))
                .cloned()
                .collect()
        })
        .collect();

    for i in 0..groups.len() {
        if own[i].is_empty() {
            continue;
        }
        let mut inherited: Vec<UtilityRule> = Vec::new();
        for j in 0..i {
            if !is_proper_subset(&groups[j].0, &groups[i].0) {
                continue;
            }
            for rule in &own[j] {
                // 只补本组确实用到的那个属性
                if !own[i].iter().any(|r| r.css_property == rule.css_property) {
                    continue;
                }
                let mut cloned = rule.clone();
                cloned.modifiers = groups[i].0.clone();
                inherited.push(cloned);
            }
        }
        if inherited.is_empty() {
            continue;
        }
        // 继承来的分量排在最前：本组自己的写在后面才能覆盖同名函数
        let rules = &mut groups[i].1;
        let existing = std::mem::take(rules);
        rules.extend(inherited);
        rules.extend(existing);
    }
}

/// 修饰符集合 `a` 是 `b` 的真子集（`b` 生效时 `a` 必然也生效）
fn is_proper_subset(a: &ModifierList, b: &ModifierList) -> bool {
    a.len() < b.len() && a.iter().all(|m| b.contains(m))
}

/// 编译期 Tailwind Merge: 相同修饰符组下的实用类属性消解 (基于 Bitmask 的高速覆盖计算，支持简写属性与长写属性关联覆盖，Last-wins 覆盖先出者)
pub(crate) fn deduplicate_utility_rules(rules: Vec<UtilityRule>) -> Vec<UtilityRule> {
    let mut covered_masks: HashMap<(ModifierList, u16), u64> = HashMap::new();
    let mut deduped_rev = Vec::new();
    // 用 Vec 而非 HashMap 保存：迭代顺序直接决定产出 CSS 的声明顺序与类名哈希，
    // HashMap 的随机迭代顺序会让同一份输入产生不确定的输出。
    let mut composable_groups: Vec<((ModifierList, CssPropertyId), Vec<UtilityRule>)> = Vec::new();

    for rule in rules.into_iter().rev() {
        let prop = rule.css_property;

        if is_composable_property(prop) {
            let key = (rule.modifiers.clone(), prop);
            match composable_groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, group)) => group.push(rule),
                None => composable_groups.push((key, vec![rule])),
            }
            continue;
        }

        let bitmask = prop.bitmask();
        let key = (rule.modifiers.clone(), bitmask.group_id);
        let current_covered = covered_masks.entry(key).or_insert(0);

        let all_covered = (*current_covered & bitmask.mask) == bitmask.mask;

        if !all_covered {
            *current_covered |= bitmask.mask;
            deduped_rev.push(rule);
        }
    }

    // 按 (修饰符组, 属性) 合并组合型规则
    for ((modifiers, prop), mut group) in composable_groups {
        let Some(first_span) = group.last().map(|r| r.span) else {
            continue;
        };
        group.reverse(); // 恢复原始顺序
        let important = group.iter().any(|r| r.important);

        // 组合型属性内部同样遵守 last-wins：`blur-sm blur-lg` 必须只留 `blur(16px)`，
        // 拼成 `blur(4px) blur(16px)` 会叠加两次模糊，是错误结果。
        // 覆盖以**函数名**为单位——`blur` 与 `brightness` 互不影响。
        //
        // 保留的是 `UtilityValue` 本身而不是渲染后的字符串：函数名只用来判断"谁覆盖谁"，
        // 真正的值等到 token 层再展开，否则 `$(signal)` 会被压成裸标识符。
        let mut combined: Vec<UtilityValue> = Vec::new();
        for value in group.into_iter().map(|r| r.value) {
            let rendered = utility_value_to_css_string(&value);
            if rendered.is_empty() {
                continue;
            }
            // `none` 是整个属性的关键字取值，不能与函数并列
            // （`filter: blur(2px) none` 是非法 CSS），它会清空此前累积的全部分量。
            if rendered == "none" {
                combined.clear();
                combined.push(value);
                continue;
            }
            if combined
                .first()
                .is_some_and(|v| utility_value_to_css_string(v) == "none")
            {
                combined.clear();
            }

            let name = composable_function_name(&rendered).to_string();
            match combined.iter().position(|prev| {
                composable_function_name(&utility_value_to_css_string(prev)) == name
            }) {
                Some(idx) => combined[idx] = value,
                None => combined.push(value),
            }
        }

        let value = if combined.len() == 1 {
            combined.pop().unwrap()
        } else {
            UtilityValue::Composed(combined)
        };

        deduped_rev.insert(
            0,
            UtilityRule {
                modifiers,
                css_property: prop,
                value,
                // 合并后只剩一条声明，任一分量带 `!` 整条就得带
                important,
                span: first_span,
            },
        );
    }

    deduped_rev.into_iter().rev().collect()
}

/// 取组合型属性中单个分量的函数名（`blur(4px)` → `blur`）。
/// 不是函数调用形式时（`none`）以整串作为标识。
fn composable_function_name(rendered: &str) -> &str {
    match rendered.find('(') {
        Some(idx) => &rendered[..idx],
        None => rendered,
    }
}

fn utility_value_to_css_string(val: &UtilityValue) -> String {
    match val {
        UtilityValue::Keyword(k) => k.to_string(),
        UtilityValue::Numeric(v, unit) => {
            if unit.is_empty() {
                v.to_string()
            } else {
                format!("{}{}", v, unit)
            }
        }
        UtilityValue::HexColor(hex) => hex.clone(),
        UtilityValue::ThemeVar(var, opacity) => match opacity {
            Some(op) => format!(
                "color-mix(in srgb, var(--slx-theme-{}) {}%, transparent)",
                var, op
            ),
            None => format!("var(--slx-theme-{})", var),
        },
        UtilityValue::ArbitraryLiteral(s) => s.clone(),
        UtilityValue::Composed(parts) => parts
            .iter()
            .map(utility_value_to_css_string)
            .collect::<Vec<_>>()
            .join(" "),
        UtilityValue::DynamicExpr { expr, wrapper, .. } => {
            let rendered = quote::quote!(#expr).to_string();
            match wrapper {
                Some(w) => w.replace("{}", &rendered),
                None => rendered,
            }
        }
    }
}

/// 从一条 `animation` 声明里挑出需要随产物一起注入的 `@keyframes` 名字。
///
/// 按**空白/逗号切出的完整 token** 与 `KEYFRAME_TABLE` 精确比对，
/// 而不是在整条值上 `contains("spin")`：后者会让 `animate-[spinner_2s_linear_infinite]`
/// 注入一份用不上的 `@keyframes spin`、`animate-[my-ping_1s]` 注入 `@keyframes ping`。
/// 候选名字也来自 `KEYFRAME_TABLE` 本身，不再硬编码那四个动画——
/// 表里新增动画时这里自动跟上。
fn collect_keyframes(value: &UtilityValue, keyframes: &mut BTreeSet<&'static str>) {
    let anim_str = match value {
        UtilityValue::Keyword(kw) => *kw,
        UtilityValue::ArbitraryLiteral(s) => s.as_str(),
        _ => return,
    };

    for token in anim_str.split([' ', ',', '\t']).filter(|t| !t.is_empty()) {
        if let Some(meta) = lookup_keyframe_meta(token) {
            keyframes.insert(meta.name);
        }
    }
}

fn inject_keyframes_rules(root_rules: &mut Vec<CssRule>, keyframes: &BTreeSet<&'static str>) {
    for name in keyframes {
        if let Some(at_rule) = build_keyframe_at_rule(name) {
            root_rules.push(CssRule::AtRule(at_rule));
        }
    }
}

fn build_keyframe_at_rule(name: &str) -> Option<CssAtRule> {
    let meta = lookup_keyframe_meta(name)?;

    let params: TokenStream = name.parse().ok()?;

    let mut keyframe_rules = Vec::new();
    for step in meta.steps {
        let decls = step
            .declarations
            .iter()
            .map(|&(prop, val)| {
                let ts: TokenStream = val.parse().unwrap_or_else(|_| quote!(#val));
                (prop, ts)
            })
            .collect();
        keyframe_rules.push(make_nested_rule(step.selector, decls));
    }

    Some(CssAtRule {
        name: "keyframes".to_string(),
        params,
        block: Some(CssBlock {
            rules: keyframe_rules,
        }),
        span: Span::call_site(),
    })
}

fn make_nested_rule(selector: &str, declarations: Vec<(&str, TokenStream)>) -> CssRule {
    let selectors: TokenStream = selector.parse().unwrap();
    let mut decl_rules = Vec::new();

    for (prop, vals) in declarations {
        decl_rules.push(CssRule::Declaration(CssDeclaration {
            property: prop.to_string(),
            values: vals,
            semi_token: Some(Semi(Span::call_site())),
            span: Span::call_site(),
        }));
    }

    CssRule::Nested(CssNested {
        selectors,
        block: CssBlock { rules: decl_rules },
    })
}

fn convert_rule_to_declaration(rule: &UtilityRule) -> CssRule {
    let prop = rule.css_property.as_str().to_string();
    let values = utility_value_to_tokens(&rule.value);

    let values = if rule.important {
        quote!(#values !important)
    } else {
        values
    };

    CssRule::Declaration(CssDeclaration {
        property: prop,
        values,
        semi_token: Some(Semi(rule.span)),
        span: rule.span,
    })
}

/// 把一个 `UtilityValue` 渲染为声明值的 token 流
fn utility_value_to_tokens(value: &UtilityValue) -> TokenStream {
    match value {
        // 表里的关键字是成品 CSS 文本（`linear-gradient(to right, var(--x))`），
        // 走 `TokenStream::parse` 会把空白交给重建逻辑去猜，这里直接逐字送出
        UtilityValue::Keyword(kw) => {
            let lit = crate::css::ast::verbatim_literal(kw);
            quote!(#lit)
        }
        UtilityValue::Numeric(val, unit) => {
            if unit.is_empty() {
                let lit = Literal::f64_unsuffixed(*val);
                quote!(#lit)
            } else {
                let lit = crate::css::ast::verbatim_literal(&format!("{}{}", val, unit));
                quote!(#lit)
            }
        }
        UtilityValue::HexColor(hex) => {
            let lit = crate::css::ast::verbatim_literal(hex);
            quote!(#lit)
        }
        UtilityValue::ThemeVar(var, opacity) => {
            let val_str = match opacity {
                Some(op) => format!(
                    "color-mix(in srgb, var(--slx-theme-{}) {}%, transparent)",
                    var, op
                ),
                None => format!("var(--slx-theme-{})", var),
            };
            let lit = crate::css::ast::verbatim_literal(&val_str);
            quote!(#lit)
        }
        UtilityValue::ArbitraryLiteral(lit) => {
            let lit_node = crate::css::ast::verbatim_literal(lit);
            quote!(#lit_node)
        }
        // 分量之间靠 token 类型自然分隔：两个相邻字面量会被序列化成 `a b`
        UtilityValue::Composed(parts) => parts.iter().map(utility_value_to_tokens).collect(),
        UtilityValue::DynamicExpr {
            expr,
            span: expr_span,
            wrapper,
        } => {
            // 包装为 Silex 动态表达式节点 `$ ( expr )`
            let mut ts = TokenStream::new();
            let mut dollar = Punct::new('$', Spacing::Joint);
            dollar.set_span(*expr_span);
            ts.extend(std::iter::once(TokenTree::Punct(dollar)));
            let mut group = Group::new(Delimiter::Parenthesis, quote!(#expr));
            // 表达式本身报错时（类型不匹配等）让 rustc 指向写它的那个词条
            group.set_span(*expr_span);
            ts.extend(std::iter::once(TokenTree::Group(group)));
            match wrapper {
                Some(w) => apply_wrapper_tokens(w, ts),
                None => ts,
            }
        }
    }
}

/// 把 `inner` 按包裹模板套进 token 流：`calc({} * -1)` + `$(x)` → `calc($(x) * -1)`
///
/// 不能对模板字符串直接 `parse::<TokenStream>()` 再拼接——`calc(` 单独一段是不平衡的
/// 分隔符，`proc_macro2` 直接拒绝。做法是先把 `{}` 换成一个合法标识符占位，
/// 让整个模板成为可解析的 token 树，再递归替换掉那个占位。
fn apply_wrapper_tokens(wrapper: &str, inner: TokenStream) -> TokenStream {
    const PLACEHOLDER: &str = "__slx_wrapped_value__";
    let Ok(template) = wrapper.replace("{}", PLACEHOLDER).parse::<TokenStream>() else {
        return inner;
    };
    substitute_placeholder(template, PLACEHOLDER, &inner)
}

fn substitute_placeholder(ts: TokenStream, placeholder: &str, inner: &TokenStream) -> TokenStream {
    ts.into_iter()
        .flat_map(|tt| match tt {
            TokenTree::Ident(ref id) if id == placeholder => inner.clone(),
            TokenTree::Group(g) => {
                let replaced = substitute_placeholder(g.stream(), placeholder, inner);
                TokenStream::from(TokenTree::Group(Group::new(g.delimiter(), replaced)))
            }
            other => TokenStream::from(other),
        })
        .collect()
}

/// 把当前块包进一个 at-rule（`@media` / `@supports` / `@container` / `@starting-style`）
///
/// 条件一律以**字符串字面量**传递：`(width >= 600px)` 走 `TokenStream::parse` 会把
/// `>=` 与数值单位重新排布，`not (min-width: 768px)` 的空格也会被吃掉。
/// 这与 `SelectorVariant` 的处理原则一致（报告 §10.5 note ②）。
fn wrap_in_at_rule(name: &str, condition: &str, block: CssBlock, span: Span) -> CssBlock {
    let selector_ts: TokenStream = "&".parse().unwrap();
    let nested_block = CssBlock {
        rules: vec![CssRule::Nested(CssNested {
            selectors: selector_ts,
            block,
        })],
    };

    let params = if condition.is_empty() {
        TokenStream::new()
    } else {
        let mut lit = Literal::string(condition);
        // 让产出的 token 携带该变体的 span：条件写错时 rustc 的箭头才会指向
        // 出问题的那个变体，而不是整个 `tw!` 字面量
        lit.set_span(span);
        quote!(#lit)
    };

    CssBlock {
        rules: vec![CssRule::AtRule(CssAtRule {
            name: name.to_string(),
            params,
            block: Some(nested_block),
            span,
        })],
    }
}

/// 把当前块包进一个嵌套选择器；`selectors` 以字符串字面量传递以保住空白
fn wrap_in_selector(selector: &str, block: CssBlock, span: Span) -> CssBlock {
    let mut lit = Literal::string(selector);
    lit.set_span(span);
    CssBlock {
        rules: vec![CssRule::Nested(CssNested {
            selectors: quote!(#lit),
            block,
        })],
    }
}

fn build_modifier_rule(modifiers: ModifierList, rules: Vec<UtilityRule>) -> Result<CssRule> {
    let mut inner_declarations = Vec::new();
    for rule in rules {
        inner_declarations.push(convert_rule_to_declaration(&rule));
    }
    let inner_block = CssBlock {
        rules: inner_declarations,
    };

    // 从右往左递归组装修饰符块
    let mut current_block = inner_block;

    for spanned in modifiers.into_iter().rev() {
        let span = spanned.span();
        current_block = match spanned.modifier {
            // 变体 key 与伪类名不总是一致（`first` → `first-child`、`even` → `nth-child(even)`）。
            // 真值只有 `MODIFIER_TABLE.css_selector` 一份——此处曾另有一张硬编码映射表，
            // 漏掉的条目（如 `file` → `::file-selector-button`）会静默产出非法伪类。
            Modifier::PseudoClass(pc) => {
                let sel = lookup_modifier_meta(pc.as_str())
                    .map(|meta| meta.css_selector.to_string())
                    .unwrap_or_else(|| format!("&:{pc}"));
                wrap_in_selector(&sel, current_block, span)
            }
            // 同 PseudoClass：`file` 的伪元素是 `::file-selector-button`，不是 `::file`
            Modifier::PseudoElement(pe) => {
                let sel = lookup_modifier_meta(pe.as_str())
                    .map(|meta| meta.css_selector.to_string())
                    .unwrap_or_else(|| format!("&::{pe}"));
                wrap_in_selector(&sel, current_block, span)
            }
            Modifier::Dark => {
                let dark_mode = get_config()
                    .and_then(|cfg| cfg.theme.dark_mode.as_deref())
                    .unwrap_or("class");
                if dark_mode == "media" {
                    wrap_in_at_rule("media", "(prefers-color-scheme: dark)", current_block, span)
                } else {
                    wrap_in_selector(".dark &, &.dark", current_block, span)
                }
            }
            Modifier::Group { state, name } => wrap_in_selector(
                &format_group_peer_selector(true, &state, name.as_deref()),
                current_block,
                span,
            ),
            Modifier::Peer { state, name } => wrap_in_selector(
                &format_group_peer_selector(false, &state, name.as_deref()),
                current_block,
                span,
            ),
            Modifier::Child => wrap_in_selector("& > *", current_block, span),
            // `& *` 的后代组合符是一个空格，走 TokenStream 会被吃掉，
            // 退化成 `&*`（作用在元素自身的复合选择器）——`wrap_in_selector` 走字符串字面量
            Modifier::Descendant => wrap_in_selector("& *", current_block, span),
            Modifier::DataAttribute { key, value } => {
                let sel = match value {
                    Some(v) => format!("&[data-{}=\"{}\"]", key, v),
                    None => format!("&[data-{}]", key),
                };
                wrap_in_selector(&sel, current_block, span)
            }
            Modifier::AriaAttribute { key, value } => {
                let sel = match value {
                    Some(v) => format!("&[aria-{}=\"{}\"]", key, v),
                    None => format!("&[aria-{}]", key),
                };
                wrap_in_selector(&sel, current_block, span)
            }
            Modifier::Has(target) => wrap_in_selector(
                &format!("&:has({})", has_target(&target)),
                current_block,
                span,
            ),
            Modifier::CustomSelector(cs) => {
                let ts: TokenStream = cs.parse().unwrap_or_else(|_| quote!(#cs));
                CssBlock {
                    rules: vec![CssRule::Nested(CssNested {
                        selectors: ts,
                        block: current_block,
                    })],
                }
            }
            // 这些选择器含有空格（后代组合符）与嵌套括号，必须走字符串字面量，
            // 否则 `&:where(..., [dir="rtl"] *)` 会退化成 `[dir=rtl]*`
            Modifier::SelectorVariant(cs) => wrap_in_selector(&cs, current_block, span),
            Modifier::MediaBreakpoint(bp) => {
                let query = if let Some(meta) = lookup_modifier_meta(bp.as_str()) {
                    meta.css_selector.to_string()
                } else {
                    let min_width = get_config()
                        .and_then(|cfg| cfg.theme.breakpoints.get(bp.as_str()).cloned())
                        .unwrap_or_else(|| "640px".to_string());
                    format!("(min-width: {})", min_width)
                };
                wrap_in_at_rule("media", &query, current_block, span)
            }
            Modifier::MediaQuery(query) => wrap_in_at_rule("media", &query, current_block, span),
            Modifier::AtRuleCondition {
                at_rule, condition, ..
            } => wrap_in_at_rule(at_rule, &condition, current_block, span),
            Modifier::ContainerQuery { name, min_width } => {
                let query = match name {
                    Some(n) => format!("{} (min-width: {})", n, min_width),
                    None => format!("(min-width: {})", min_width),
                };
                wrap_in_at_rule("container", &query, current_block, span)
            }
        };
    }

    if current_block.rules.len() == 1 {
        Ok(current_block.rules.into_iter().next().unwrap())
    } else {
        let ts: TokenStream = "&".parse().unwrap();
        Ok(CssRule::Nested(CssNested {
            selectors: ts,
            block: current_block,
        }))
    }
}

/// 将 `data-[k=v]` / `data-[k]` 形式的内容转换为属性选择器片段
fn bracket_attr_selector(kind: &str, inner: &str) -> String {
    match inner.split_once('=') {
        Some((k, v)) => format!("[{}-{}=\"{}\"]", kind, k, v.trim_matches('"')),
        None => format!("[{}-{}]", kind, inner),
    }
}

/// 把 `has-*` 变体的参数转换为 `:has(...)` 的内容
///
/// `has-[.x]` / `has-data-[k=v]` / `has-not-[.x]` 三种形态。缺了 `has-not-` 会退化成
/// `:has(has-not-[.x])`——语法上 LightningCSS 也许放行，但永远不匹配任何元素。
pub(crate) fn has_target(target: &str) -> String {
    let inner = target.strip_prefix("has-").unwrap_or(target);

    if let Some(rest) = inner
        .strip_prefix("data-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        return bracket_attr_selector("data", rest);
    }
    if let Some(rest) = inner
        .strip_prefix("aria-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        return bracket_attr_selector("aria", rest);
    }
    if let Some(rest) = inner.strip_prefix("not-") {
        return format!(":not({})", has_target(rest));
    }
    if let Some(rest) = inner.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return rest.to_string();
    }
    // 无括号的状态简写（`has-checked` → `:has(:checked)`）：`&` 去掉后剩下的伪类
    // 本身就表示"任意后代处于该状态"，不需要再补 `*`
    if let Some(meta) = lookup_modifier_meta(inner) {
        return meta
            .css_selector
            .strip_prefix('&')
            .unwrap_or(meta.css_selector)
            .to_string();
    }
    inner.to_string()
}

/// 计算 group/peer 状态在 marker 元素自身上的复合选择器后缀（不含组合符）
fn group_peer_state_compound(state: &str) -> String {
    if let Some(inner) = state
        .strip_prefix("data-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        bracket_attr_selector("data", inner)
    } else if let Some(inner) = state
        .strip_prefix("aria-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        bracket_attr_selector("aria", inner)
    } else if let Some(inner) = state
        .strip_prefix("has-data-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        format!(":has({})", bracket_attr_selector("data", inner))
    } else if let Some(inner) = state
        .strip_prefix("has-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        format!(":has({})", inner)
    } else if let Some(inner) = state
        .strip_prefix("not-[")
        .and_then(|s| s.strip_suffix(']'))
    {
        format!(":not({})", inner.strip_prefix('&').unwrap_or(inner))
    } else if state.starts_with('[') && state.ends_with(']') {
        // 任意选择器形式 `[&:hover]` / `[&.foo]` / `[.foo]`
        let inner = &state[1..state.len() - 1];
        inner.strip_prefix('&').unwrap_or(inner).to_string()
    } else {
        // 普通状态：伪类（含 aria-xxx / data-xxx 的无括号简写由上游转成 state 原样透传）
        format!(":{}", state)
    }
}

/// 拼接 group/peer 变体选择器。
///
/// 复合部分（`:hover`、`[data-x="y"]`、`:has(...)`）永远作用在 marker 元素自身上，
/// 随后**必须**跟一个组合符才能指向被修饰元素：group 为后代组合符 `" &"`，
/// peer 为兄弟组合符 `" ~ &"`。各分支一律不得自行拼接 connector。
fn format_group_peer_selector(is_group: bool, state: &str, name: Option<&str>) -> String {
    let prefix = if is_group { ".group" } else { ".peer" };
    let base = match name {
        Some(n) => format!("{}\\/{}", prefix, n),
        None => prefix.to_string(),
    };
    let connector = if is_group { "&" } else { "~ &" };

    format!("{}{} {}", base, group_peer_state_compound(state), connector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use smallvec::smallvec;

    #[test]
    fn test_format_group_peer_selector_exact() {
        // group：复合部分作用在 .group 自身，其后必须是后代组合符
        assert_eq!(
            format_group_peer_selector(true, "hover", None),
            ".group:hover &"
        );
        assert_eq!(
            format_group_peer_selector(true, "data-[state=open]", None),
            ".group[data-state=\"open\"] &"
        );
        assert_eq!(
            format_group_peer_selector(true, "data-[disabled]", None),
            ".group[data-disabled] &"
        );
        assert_eq!(
            format_group_peer_selector(true, "aria-[expanded=true]", None),
            ".group[aria-expanded=\"true\"] &"
        );
        assert_eq!(
            format_group_peer_selector(true, "has-[.x]", None),
            ".group:has(.x) &"
        );
        assert_eq!(
            format_group_peer_selector(true, "has-data-[size=lg]", None),
            ".group:has([data-size=\"lg\"]) &"
        );
        assert_eq!(
            format_group_peer_selector(true, "[&:focus-visible]", None),
            ".group:focus-visible &"
        );
        assert_eq!(
            format_group_peer_selector(true, "[.foo]", None),
            ".group.foo &"
        );
        assert_eq!(
            format_group_peer_selector(true, "data-[size=sm]", Some("avatar")),
            ".group\\/avatar[data-size=\"sm\"] &"
        );

        // peer：兄弟组合符
        assert_eq!(
            format_group_peer_selector(false, "focus", None),
            ".peer:focus ~ &"
        );
        assert_eq!(
            format_group_peer_selector(false, "data-[state=open]", Some("sidebar")),
            ".peer\\/sidebar[data-state=\"open\"] ~ &"
        );
    }

    #[test]
    fn test_transform_rules_merging() {
        let rules = vec![
            UtilityRule {
                modifiers: smallvec![],
                css_property: CssPropertyId::Transform,
                value: UtilityValue::ArbitraryLiteral("translateX(-50%)".to_string()),
                important: false,
                span: Span::call_site(),
            },
            UtilityRule {
                modifiers: smallvec![],
                css_property: CssPropertyId::Transform,
                value: UtilityValue::ArbitraryLiteral("translateY(-50%)".to_string()),
                important: false,
                span: Span::call_site(),
            },
        ];

        let deduped = deduplicate_utility_rules(rules);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].css_property, CssPropertyId::Transform);
        // 合并后是 Composed（分量原样保留），渲染出来才是拼接结果
        assert_eq!(
            utility_value_to_css_string(&deduped[0].value),
            "translateX(-50%) translateY(-50%)"
        );
        assert!(
            matches!(deduped[0].value, UtilityValue::Composed(ref p) if p.len() == 2),
            "组合型属性合并必须保留分量结构，否则动态表达式会被提前压成字符串"
        );
    }

    #[test]
    fn test_responsive_breakpoint_sorting() {
        let lg_rule = UtilityRule {
            modifiers: smallvec![Modifier::MediaBreakpoint("lg".to_string()).into()],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(2.0, "rem"),
            important: false,
            span: Span::call_site(),
        };
        let sm_rule = UtilityRule {
            modifiers: smallvec![Modifier::MediaBreakpoint("sm".to_string()).into()],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(0.5, "rem"),
            important: false,
            span: Span::call_site(),
        };

        // 假定输入顺序为 lg 在前, sm 在后
        let block = build_css_block_from_rules(vec![lg_rule, sm_rule]).unwrap();
        assert_eq!(block.rules.len(), 2);

        // 验证转换后的 AtRule 中，sm (640px) 应该排在 lg (1024px) 之前
        if let CssRule::AtRule(ref at1) = block.rules[0] {
            assert!(at1.params.to_string().contains("640px"));
        } else {
            panic!("Expected AtRule for sm breakpoint first");
        }

        if let CssRule::AtRule(ref at2) = block.rules[1] {
            assert!(at2.params.to_string().contains("1024px"));
        } else {
            panic!("Expected AtRule for lg breakpoint second");
        }
    }

    #[test]
    fn test_extended_atomic_deduplication() {
        let inset_x = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::InsetX,
            value: UtilityValue::Numeric(0.0, "px"),
            important: false,
            span: Span::call_site(),
        };
        let left_override = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Left,
            value: UtilityValue::Numeric(1.0, "rem"),
            important: false,
            span: Span::call_site(),
        };

        let deduped = deduplicate_utility_rules(vec![inset_x, left_override]);
        // inset-x 生成了 left 和 right 的覆盖，后续的 left 将正确覆盖 inset-x 中的 left
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].css_property, CssPropertyId::InsetX);
        assert_eq!(deduped[1].css_property, CssPropertyId::Left);
    }

    #[test]
    fn test_transform_rules_merging_respects_modifiers() {
        let base_transform = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Transform,
            value: UtilityValue::ArbitraryLiteral("translate-x-0".to_string()),
            important: false,
            span: Span::call_site(),
        };
        let dark_transform = UtilityRule {
            modifiers: smallvec![Modifier::Dark.into()],
            css_property: CssPropertyId::Transform,
            value: UtilityValue::ArbitraryLiteral("translate-x-full".to_string()),
            important: false,
            span: Span::call_site(),
        };

        let deduped =
            deduplicate_utility_rules(vec![base_transform.clone(), dark_transform.clone()]);
        assert_eq!(
            deduped.len(),
            2,
            "Base transform and dark transform must NOT be merged together into one rule"
        );

        let has_base = deduped
            .iter()
            .any(|r| r.modifiers.is_empty() && r.css_property == CssPropertyId::Transform);
        let has_dark = deduped.iter().any(|r| {
            r.modifiers.len() == 1
                && r.modifiers[0] == Modifier::Dark
                && r.css_property == CssPropertyId::Transform
        });
        assert!(has_base && has_dark);
    }

    #[test]
    fn test_deduplicate_respects_modifiers_for_general_properties() {
        let base_padding = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(1.0, "rem"),
            important: false,
            span: Span::call_site(),
        };
        let hover_padding = UtilityRule {
            modifiers: smallvec![Modifier::PseudoClass("hover".to_string()).into()],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(2.0, "rem"),
            important: false,
            span: Span::call_site(),
        };
        let override_padding = UtilityRule {
            modifiers: smallvec![],
            css_property: CssPropertyId::Padding,
            value: UtilityValue::Numeric(1.5, "rem"),
            important: false,
            span: Span::call_site(),
        };

        // 输入顺序：p-4 (base), hover:p-8 (hover), p-6 (override)
        let deduped = deduplicate_utility_rules(vec![
            base_padding,
            hover_padding.clone(),
            override_padding.clone(),
        ]);

        // 应该保留 2 条：hover:p-8 和最后覆盖的 p-6 (override)，而最早的 base p-4 被 override 覆盖
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].modifiers, hover_padding.modifiers);
        assert_eq!(deduped[1].modifiers, override_padding.modifiers);
    }
}
