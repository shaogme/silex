use proc_macro2::token_stream::IntoIter;
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;
use std::ops::Range;
use std::rc::Rc;
use syn::Result;

use super::types::CssWarning;

/// 一层 token 的游标，同时携带从原文恢复出来的空白信息。
///
/// 直接迭代 `TokenStream` 会丢掉 token 之间的空白，而 CSS 里空白有语义
/// （见 [`crate::css::spacing`]）。这个游标把「下一个 token 前原文里是否有空白」
/// 和迭代绑在一起，`handler` 自行消费 token 时下标也能跟着走。
#[derive(Clone)]
pub(crate) struct CssTokens {
    iter: Peekable<IntoIter>,
    /// `info[i]`：第 i 个 token 在原文中的位置信息；`None` 表示原文不可得
    info: Option<Vec<crate::css::spacing::TokenSpacing>>,
    idx: usize,
    /// 本层 token 所处的原文片段
    region: Option<Rc<str>>,
}

impl CssTokens {
    pub(crate) fn new(ts: &TokenStream, region: Option<Rc<str>>) -> Self {
        let info = region.as_deref().and_then(|src| {
            let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
            crate::css::spacing::recover(&tokens, src)
        });
        Self {
            iter: ts.clone().into_iter().peekable(),
            info,
            idx: 0,
            region,
        }
    }

    /// 进入一个 `Group`：用匹配时算出的组内范围切出子片段。
    /// 定位失败时子层也没有原文可依据，退回启发式。
    pub(crate) fn descend(&self, g: &proc_macro2::Group, inner: Option<Range<usize>>) -> Self {
        let region = match (&self.region, inner) {
            (Some(src), Some(range)) => src.get(range).map(Rc::<str>::from),
            _ => None,
        };
        Self::new(&g.stream(), region)
    }

    pub(crate) fn next(&mut self) -> Option<TokenTree> {
        let tt = self.iter.next();
        if tt.is_some() {
            self.idx += 1;
        }
        tt
    }

    pub(crate) fn peek(&mut self) -> Option<&TokenTree> {
        self.iter.peek()
    }

    /// 下一个 token 在原文中的位置信息；`None` = 无法确定。
    pub(crate) fn info_of_next(&self) -> Option<crate::css::spacing::TokenSpacing> {
        self.info.as_ref().and_then(|v| v.get(self.idx).cloned())
    }

    /// 下一个 token 前原文里是否有空白；`None` = 无法确定。
    pub(crate) fn space_before_next(&self) -> Option<bool> {
        self.info_of_next().map(|i| i.space_before)
    }
}

/// 整个宏调用的源码。stable 工具链上这是恢复空白的唯一依据。
pub(crate) fn macro_region() -> Option<Rc<str>> {
    Span::call_site().source_text().map(Rc::<str>::from)
}

pub(crate) fn process_tokens<F>(
    ts: &TokenStream,
    region: Option<Rc<str>>,
    warnings: &mut Vec<CssWarning>,
    handler: &mut F,
) -> Result<String>
where
    F: FnMut(&TokenTree, &mut CssTokens, &mut String, bool) -> Result<bool>,
{
    let mut cursor = CssTokens::new(ts, region);
    process_tokens_iter(&mut cursor, warnings, handler)
}

/// 原文不可得时的保守回退：只按 token 类型猜。
///
/// 这条路只在 token 不来自用户源码时才会走到（例如 `@apply` 由 tw 展开出来的
/// 规则），那些 token 的形状是我们自己生成的、可控的。
pub(crate) fn guess_space_between(prev: &TokenTree, cur: &TokenTree) -> bool {
    /// 媒体/特性查询里的逻辑关键字，后面跟括号时必须留空格，
    /// 否则 `screen and (…)` 会被拼成函数调用 `and(…)`。
    ///
    /// 只收那些**不可能是 CSS 函数名**的词：`not` / `selector` 同时也是伪类函数
    /// （`:not(.a)`、`@supports selector(…)`），给它们补空格会把 `:not ([hidden])`
    /// 写成非法选择器。用户手写的媒体查询能从原文恢复空白，走不到这条回退。
    const QUERY_KEYWORDS: [&str; 3] = ["and", "or", "only"];

    match (prev, cur) {
        (TokenTree::Ident(_), TokenTree::Ident(_))
        | (TokenTree::Ident(_), TokenTree::Literal(_))
        | (TokenTree::Literal(_), TokenTree::Ident(_))
        | (TokenTree::Literal(_), TokenTree::Literal(_))
        | (TokenTree::Group(_), TokenTree::Ident(_))
        | (TokenTree::Group(_), TokenTree::Literal(_))
        | (TokenTree::Group(_), TokenTree::Group(_)) => true,
        // `and (min-width: 1px)`：关键字与括号之间必须有空格
        (TokenTree::Ident(id), TokenTree::Group(g))
            if g.delimiter() == Delimiter::Parenthesis
                && QUERY_KEYWORDS.contains(&id.to_string().as_str()) =>
        {
            true
        }
        (TokenTree::Ident(_), TokenTree::Punct(p)) if p.as_char() == '&' => true,
        // `& span`：`&` 后紧跟元素名时按后代选择器处理。复合形式（`&span`，
        // 即「自身同时是该元素」）远比后代少见，需要时用字符串字面量选择器书写。
        (TokenTree::Punct(p), TokenTree::Ident(_)) if p.as_char() == '&' => true,
        (TokenTree::Punct(p), TokenTree::Ident(_))
        | (TokenTree::Punct(p), TokenTree::Literal(_))
            if p.as_char() == '$' =>
        {
            true
        }
        (TokenTree::Punct(p1), TokenTree::Punct(p2))
            if p2.as_char() == '&'
                && (p1.as_char() == '~' || p1.as_char() == '>' || p1.as_char() == '+') =>
        {
            true
        }
        (TokenTree::Punct(p1), _)
            if p1.as_char() == '~' || p1.as_char() == '>' || p1.as_char() == '+' =>
        {
            true
        }
        _ => false,
    }
}

pub(crate) fn process_tokens_iter<F>(
    cursor: &mut CssTokens,
    warnings: &mut Vec<CssWarning>,
    handler: &mut F,
) -> Result<String>
where
    F: FnMut(&TokenTree, &mut CssTokens, &mut String, bool) -> Result<bool>,
{
    let mut out = String::new();
    let mut prev_tt: Option<TokenTree> = None;

    loop {
        let info = cursor.info_of_next();
        let Some(tt) = cursor.next() else { break };

        let space_before = match (&prev_tt, &info) {
            (None, _) => false,
            (Some(_), Some(known)) => known.space_before,
            (Some(prev), None) => guess_space_between(prev, &tt),
        };

        if handler(&tt, cursor, &mut out, space_before)? {
            prev_tt = Some(tt);
            continue;
        }

        if space_before {
            out.push(' ');
        }

        match tt {
            TokenTree::Group(g) => {
                let delim = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => (' ', ' '),
                };
                if delim.0 != ' ' {
                    out.push(delim.0);
                }
                let mut sub = cursor.descend(&g, info.and_then(|i| i.inner));
                out.push_str(&process_tokens_iter(&mut sub, warnings, handler)?);
                if delim.1 != ' ' {
                    out.push(delim.1);
                }
                prev_tt = Some(TokenTree::Group(g));
            }
            TokenTree::Punct(p) => {
                if p.as_char() == '?' {
                    warnings.push(CssWarning {
                        message: "[Silex CSS Warning] Potentially ambiguous token '?' in CSS stream. If this is a Rust expression, wrap it in $(...).".to_string(),
                        span: p.span(),
                    });
                }
                out.push(p.as_char());
                prev_tt = Some(TokenTree::Punct(p));
            }
            TokenTree::Ident(id) => {
                out.push_str(&id.to_string());
                prev_tt = Some(TokenTree::Ident(id));
            }
            TokenTree::Literal(lit) => {
                out.push_str(&render_literal(&lit));
                prev_tt = Some(TokenTree::Literal(lit));
            }
        }
    }
    Ok(out)
}

/// 把 Rust 字面量转成 CSS 里的等价写法。
///
/// 字符串字面量**保留引号**：`content: "hello"`、`grid-template-areas: "a b" "c d"`、
/// `[data-x="1"]`、`url("a b.png")` 都依赖它。此前这里无条件剥离引号，产出的
/// `content:hello` 是无效声明、`quotes:" "` 更是把两个字符串并成了一个。
///
/// 走 `syn::Lit` 拿到字面量的真实内容（顺带支持 `r"…"` / `r#"…"#`），再按 CSS 的
/// 转义规则重新写出，而不是原样透传 Rust 的转义序列。
pub(crate) fn render_literal(lit: &proc_macro2::Literal) -> String {
    match syn::Lit::new(lit.clone()) {
        syn::Lit::Str(s) => escape_css_string(&s.value()),
        // 字节串是代码生成器的「逐字 CSS 文本」标记，见 `ast::verbatim_literal`
        syn::Lit::ByteStr(b) => String::from_utf8(b.value()).unwrap_or_default(),
        syn::Lit::Char(c) => escape_css_string(&c.value().to_string()),
        syn::Lit::CStr(_) | syn::Lit::Byte(_) => lit.to_string(),
        _ => lit.to_string(),
    }
}

/// 按 CSS 的 `<string>` 语法写出一个带引号的字符串。
pub fn escape_css_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            // CSS 字符串里不允许裸换行，必须写成 Unicode 转义
            '\n' => out.push_str("\\A "),
            '\r' => out.push_str("\\D "),
            // 其余控制字符也一律转义。除了本来就该这么写，这还保证了
            // `PLACEHOLDER_CLASS` / `PLACEHOLDER_VALUE` /
            // `PLACEHOLDER_SELECTOR_VALUE` /
            // `PLACEHOLDER_PENDING_CLASS` 这几个占位符不可能从用户的
            // 字符串字面量里冒出来
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\{:X} ", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn handle_dollar_path(iter: &mut CssTokens) -> syn::Result<Option<TokenStream>> {
    let mut sub_iter = iter.clone();
    if let Some(TokenTree::Ident(id)) = sub_iter.next() {
        // Try parsing as a path
        let mut tokens = vec![TokenTree::Ident(id)];
        while let Some(TokenTree::Punct(p)) = sub_iter.peek()
            && p.as_char() == ':'
        {
            let p1 = sub_iter.next().unwrap();
            if let Some(tt2) = sub_iter.next() {
                if let TokenTree::Punct(ref p2) = tt2
                    && p2.as_char() == ':'
                {
                    tokens.push(p1);
                    tokens.push(tt2);
                    if let Some(TokenTree::Ident(next_id)) = sub_iter.next() {
                        tokens.push(TokenTree::Ident(next_id));
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        *iter = sub_iter;
        return Ok(Some(tokens.into_iter().collect()));
    }
    Ok(None)
}

pub fn append_token_stream_strings(
    ts: &TokenStream,
    region: Option<Rc<str>>,
    warnings: &mut Vec<CssWarning>,
) -> Result<String> {
    // Basic version used for @-rules and such, no special $ or & handling
    process_tokens(ts, region, warnings, &mut |_, _, _, _| Ok(false))
}

/// 整段写成一个字符串字面量时（`"div > p" { … }``@media "(width >= 600px)"`）
/// 取其裸内容。
///
/// 这是 token 流无法表达的写法（复合元素选择器 `&div`、`:not(.a .b)` 这种依赖
/// 精确空白的选择器，以及 `(width >= 600px)` 这类会被 Rust 词法重排的条件）的
/// 逃生舱，所以这里——也只有这里——才剥引号。`tw` 的 codegen 正是走这条路把
/// 选择器与查询条件原样递给编译器的。
pub(crate) fn lone_string_literal(ts: &TokenStream) -> Option<String> {
    let mut iter = ts.clone().into_iter();
    let first = iter.next()?;
    if iter.next().is_some() {
        return None;
    }
    let TokenTree::Literal(lit) = first else {
        return None;
    };
    match syn::Lit::new(lit) {
        syn::Lit::Str(s) => Some(s.value()),
        _ => None,
    }
}

/// `$var` 后面跟什么算合法。
///
/// 判据是原文里有没有空白：`$theme.field` 是字段访问（必须写成 `$(…)`），
/// `$theme .x` 是后代选择器、`$c !important` 是优先级标记，两者都合法。
/// 原文不可得时保持从严——宁可要求用户显式写 `$(…)`，也不猜。
pub(crate) fn check_unexpected_complex_tokens(iter: &mut CssTokens) -> syn::Result<()> {
    let separated = iter.space_before_next() == Some(true);
    if let Some(next_tt) = iter.peek() {
        match next_tt {
            TokenTree::Punct(p_next)
                if matches!(p_next.as_char(), '.' | '!' | '?' | ':') && !separated =>
            {
                return Err(syn::Error::new(
                    p_next.span(),
                    format!(
                        "Unexpected '{}' after dynamic variable. Complex expressions like method calls, array indexing, or field access must be wrapped in $(...).",
                        p_next.as_char()
                    ),
                ));
            }
            // `(` 紧跟变量一律视为调用；`[` 只在紧贴时视为索引
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Parenthesis
                    || (g.delimiter() == Delimiter::Bracket && !separated) =>
            {
                return Err(syn::Error::new(
                    g.span(),
                    "Unexpected brackets/parentheses after dynamic variable. Complex expressions like method calls, array indexing, or field access must be wrapped in $(...).",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
