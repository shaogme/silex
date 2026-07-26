use crate::css::{
    config::get_config,
    tw::{
        ast::{Modifier, SpannedModifier, TwInput, TwSegment, UtilityRule},
        functional::parse_functional_modifier,
        resolver::{
            codegen::modifiers::parse_modifier_fast, is_marker_class, resolve_utility,
            suggest::find_best_modifier_suggestion,
        },
    },
};
use proc_macro2::{Literal, Span};
use syn::{
    Error, Expr, LitStr, Result, Token, parenthesized,
    parse::{Parse, ParseStream},
    token::Paren,
};

/// 一串 tw 词条在源码中的定位锚点。
///
/// 报告 §4.1：此前每个词条与每个变体都共用整个字符串字面量的 span，
/// `tw!("flex items-center p-44x rounded-lg")` 里的一个笔误会把**整行**标红，
/// `SpannedModifier.span` / `UtilityRule.span` 这套基础设施等于空转。
///
/// 这里按字节偏移给每个词条（以及每个变体前缀）单独算 span：
///
/// * nightly 下 `Literal::subspan` 能给出真正的子区间，rustc 的箭头直接指到笔误上；
/// * stable 下 `subspan` 恒返回 `None`（proc_macro 侧尚未稳定），退回整体 span，
///   并在错误信息里附一段插入符上下文，指出是哪个词条出的问题。
pub(crate) struct TokenAnchor<'a> {
    /// 字符串字面量的**内容**（回退时打印上下文用）
    raw: &'a str,
    /// 字面量 token；`subspan` 的偏移是相对它的源码文本（含引号）
    lit: Option<Literal>,
    /// 整体 span，作为回退
    span: Span,
}

impl<'a> TokenAnchor<'a> {
    /// 没有字面量可定位时（`@apply`、`styled!` 的内联字符串等）的退化锚点
    pub(crate) fn whole(raw: &'a str, span: Span) -> Self {
        Self {
            raw,
            lit: None,
            span,
        }
    }

    pub(crate) fn from_lit_str(raw: &'a str, lit: &LitStr) -> Self {
        Self {
            raw,
            lit: Some(lit.token()),
            span: lit.span(),
        }
    }

    /// 内容偏移 `[offset, offset + len)` 对应的精确 span
    fn sub_opt(&self, offset: usize, len: usize) -> Option<Span> {
        let lit = self.lit.as_ref()?;
        let repr = lit.to_string();
        // 引号（以及 `r#"` 前缀）之后才是内容
        let content_start = repr.find('"')? + 1;
        // 内容里有转义时，字面量文本与内容的偏移不再一一对应，宁可退回整体 span
        if repr.len() != content_start + self.raw.len() + 1 {
            return None;
        }
        lit.subspan(content_start + offset..content_start + offset + len)
    }

    fn sub(&self, offset: usize, len: usize) -> Span {
        self.sub_opt(offset, len).unwrap_or(self.span)
    }

    /// span 无法细化时，给错误信息补一段插入符上下文
    fn decorate(&self, err: Error, offset: usize, len: usize) -> Error {
        if self.sub_opt(offset, len).is_some() {
            // 箭头已经指到位，再画一遍插入符只是噪音
            return err;
        }
        let msg = err.to_string();
        // 内层（变体前缀）已经画过更精确的插入符，外层不要再叠一遍整词条的
        if msg.contains(CARET_MARKER) {
            return err;
        }
        Error::new(
            self.sub(offset, len),
            format!("{}\n{}", msg, caret_context(self.raw, offset, len)),
        )
    }
}

const CARET_MARKER: &str = "  in `tw!` string:";

/// 画出 `词条上下文 + 插入符` 两行，长字符串两侧用 `…` 截断
fn caret_context(raw: &str, offset: usize, len: usize) -> String {
    const WINDOW: usize = 36;

    let chars: Vec<char> = raw.chars().collect();
    // 偏移是字节量，转成字符位置才能对齐插入符
    let start_char = raw[..offset.min(raw.len())].chars().count();
    let len_char = raw[offset.min(raw.len())..(offset + len).min(raw.len())]
        .chars()
        .count()
        .max(1);

    let from = start_char.saturating_sub(WINDOW);
    let to = (start_char + len_char + WINDOW).min(chars.len());
    let head = if from > 0 { "…" } else { "" };
    let tail = if to < chars.len() { "…" } else { "" };
    let snippet: String = chars[from..to].iter().collect();

    format!(
        "{CARET_MARKER}\n    {}{}{}\n    {}{}",
        head,
        snippet,
        tail,
        " ".repeat(head.chars().count() + (start_char - from)),
        "^".repeat(len_char),
    )
}

/// 解析一串以空白分隔的 tw 词条。
///
/// `tw!` / `@apply` / `styled!` 三条入口共用此函数——它们此前各自抄了一遍
/// "按空白切分 → 剥变体 → resolve" 的循环，于是 `!important` 之类的词条级语法
/// 只在其中一处生效，细粒度 span 也只有一处能享受到。
pub(crate) fn parse_class_list(
    anchor: &TokenAnchor<'_>,
    extra_classes: &mut Vec<String>,
) -> Result<Vec<UtilityRule>> {
    let mut rules = Vec::new();
    for (offset, token) in split_whitespace_indices(anchor.raw) {
        let span = anchor.sub(offset, token.len());
        let (mut resolved, marker) = parse_utility_token(token, span, anchor, offset)
            .map_err(|e| anchor.decorate(e, offset, token.len()))?;
        if let Some(marker) = marker
            && !extra_classes.contains(&marker.to_string())
        {
            extra_classes.push(marker.to_string());
        }
        rules.append(&mut resolved);
    }
    Ok(rules)
}

/// `split_whitespace` 的带字节偏移版本
fn split_whitespace_indices(s: &str) -> impl Iterator<Item = (usize, &str)> {
    s.split_whitespace().map(move |tok| {
        // 词条来自 `s` 本身，指针差就是字节偏移
        let offset = tok.as_ptr() as usize - s.as_ptr() as usize;
        (offset, tok)
    })
}

/// 解析单个词条（变体前缀 + `!important` 标记 + 工具类本体）为规则集。
///
/// 第二个返回值是该词条本身需要原样进 class 属性的 marker 类名（`group` / `peer` /
/// `container`），没有则为 `None`。
pub(crate) fn parse_utility_token<'t>(
    token: &'t str,
    span: Span,
    anchor: &TokenAnchor<'_>,
    token_offset: usize,
) -> Result<(Vec<UtilityRule>, Option<&'t str>)> {
    let (token, mut important) = match token.strip_suffix('!') {
        Some(rest) => (rest, true),
        None => (token, false),
    };

    let (modifiers, body_token) = parse_modifiers_and_body(token, anchor, token_offset)?;

    // 兼容 v3 的前置写法 `!p-4`；剥变体之后才轮到它，`hover:!p-4` 才能正确识别
    let body_token = match body_token.strip_prefix('!') {
        Some(rest) => {
            important = true;
            rest
        }
        None => body_token,
    };

    let marker = (modifiers.is_empty() && is_marker_class(body_token)).then_some(body_token);

    let mut rules = resolve_utility(modifiers, body_token, span)?;
    if important {
        for rule in &mut rules {
            rule.important = true;
        }
    }
    Ok((rules, marker))
}

impl Parse for TwInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut segments = Vec::new();
        let mut extra_classes = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                let lit: LitStr = input.parse()?;
                let raw = lit.value();
                let rules =
                    parse_class_list(&TokenAnchor::from_lit_str(&raw, &lit), &mut extra_classes)?;
                segments.push(TwSegment::Static(rules));
            } else if input.peek(Paren) {
                let content;
                parenthesized!(content in input);

                let (cond_expr, lit, else_lit) = if content.peek(LitStr) {
                    let lit: LitStr = content.parse()?;
                    content.parse::<Token![,]>()?;
                    let cond_expr: Expr = content.parse()?;
                    let else_lit: Option<LitStr> = if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        if content.peek(LitStr) {
                            Some(content.parse()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    (cond_expr, lit, else_lit)
                } else {
                    let cond_expr: Expr = content.parse()?;
                    content.parse::<Token![,]>()?;
                    let lit: LitStr = content.parse()?;
                    let else_lit: Option<LitStr> = if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        if content.peek(LitStr) {
                            Some(content.parse()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    (cond_expr, lit, else_lit)
                };

                let then_raw = lit.value();
                let then_rules = parse_class_list(
                    &TokenAnchor::from_lit_str(&then_raw, &lit),
                    &mut extra_classes,
                )?;
                let else_rules = if let Some(else_l) = else_lit {
                    let else_raw = else_l.value();
                    parse_class_list(
                        &TokenAnchor::from_lit_str(&else_raw, &else_l),
                        &mut extra_classes,
                    )?
                } else {
                    Vec::new()
                };

                segments.push(TwSegment::Conditional {
                    condition: cond_expr,
                    then_rules,
                    else_rules,
                });
            } else {
                return Err(
                    input.error("Expected string literal or conditional tuple in tw! macro")
                );
            }

            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(TwInput {
            segments,
            extra_classes,
        })
    }
}

/// 解析单个修饰符前缀字符串为 `Modifier`
///
/// 未知前缀会**报编译错误**并给出 Levenshtein 建议，绝不静默降级为伪类：
/// LightningCSS 不会拒绝未知伪类，`mdd:flex` 之类的拼写错误会一路生成
/// 永远不匹配任何元素的 `:mdd` 规则。若确需透传自定义伪类，请写作 `[&:my-pseudo]:`。
pub(crate) fn parse_single_modifier(prefix: &str, span: Span) -> Result<Modifier> {
    // 1. 调用 Codegen 生成的 0-alloc Match DFA / 状态机快速解析
    if let Some(m) = parse_modifier_fast(prefix) {
        return Ok(m);
    }

    // 2. 自定义断点响应式匹配
    let is_custom_bp = get_config()
        .map(|cfg| cfg.theme.breakpoints.contains_key(prefix))
        .unwrap_or(false);
    if is_custom_bp {
        return Ok(Modifier::MediaBreakpoint(prefix.to_string()));
    }

    // 3. 带参数的函数式变体（`not-*` / `in-*` / `nth-*` / `min-*` / `max-*` /
    //    `supports-[…]` / `starting`）——无法枚举进 `MODIFIER_TABLE`，手写解析
    if let Some(m) = parse_functional_modifier(prefix, span)? {
        return Ok(m);
    }

    // 4. 未知前缀：报错并给出建议
    let msg = match find_best_modifier_suggestion(prefix) {
        Some(s) => format!(
            "Unknown variant prefix '{}:'. Did you mean '{}:'? \
             (use `[&:{}]:` to pass an arbitrary pseudo-class through)",
            prefix, s, prefix
        ),
        None => format!(
            "Unknown variant prefix '{}:'. \
             Use `[&:{}]:` if you intend to emit an arbitrary pseudo-class.",
            prefix, prefix
        ),
    };
    Err(Error::new(span, msg))
}

/// 剥离修饰符前缀（如 `hover:`, `md:`, `dark:`）与基础 Utility Token，并附加细粒度 Span
///
/// 每个变体前缀都按它在字面量里的字节区间单独取 span，`md:hoveer:flex` 的箭头
/// 只指向 `hoveer`，而不是整个词条。
pub(crate) fn parse_modifiers_and_body<'t>(
    token: &'t str,
    anchor: &TokenAnchor<'_>,
    token_offset: usize,
) -> Result<(Vec<SpannedModifier>, &'t str)> {
    let mut modifiers = Vec::new();
    let mut current = token;
    let mut offset = token_offset;

    while let Some((prefix, rest)) = split_modifier(current) {
        let prefix_span = anchor.sub(offset, prefix.len());
        let modifier = parse_single_modifier(prefix, prefix_span)
            .map_err(|e| anchor.decorate(e, offset, prefix.len()))?;
        modifiers.push(SpannedModifier::new(modifier, prefix_span));
        // `+ 1` 跳过分隔用的 ':'
        offset += prefix.len() + 1;
        current = rest;
    }

    Ok((modifiers, current))
}

fn split_modifier(s: &str) -> Option<(&str, &str)> {
    let colon_idx = s.find(':')?;
    let prefix = &s[..colon_idx];
    if prefix.contains('[') && !prefix.contains(']') {
        let close_idx = s.find(']')?;
        let next_colon = s[close_idx..].find(':')?;
        let real_colon = close_idx + next_colon;
        return Some((&s[..real_colon], &s[real_colon + 1..]));
    }
    Some((prefix, &s[colon_idx + 1..]))
}
