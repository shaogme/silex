//! 从原文恢复 token 之间的空白信息。
//!
//! Rust 的 `TokenStream` 不保留 token 间的空白，而 CSS 里空白是有语义的：
//! `.a .b`（后代）与 `.a.b`（复合）、`& span` 与 `&span`、`and (…)` 与 `and(…)`
//! 是完全不同的东西。靠「相邻 token 的类型」去猜必然会在某些组合上猜错，而且
//! 猜错时产出的仍是合法 CSS——只是匹配了另一批元素，不报错、不告警。
//!
//! 所以这里不猜，回到原文去看。stable 工具链上 [`proc_macro2::Span::byte_range`]
//! 恒为 `0..0`（那是 nightly 才有的信息），但 `Span::call_site().source_text()`
//! 能拿到**整个宏调用的源码**。于是：把 token 序列当作「中间只允许夹空白与注释」
//! 的模式去那段原文里匹配，匹配处唯一（或多处匹配结论一致）时，每个 token 前
//! 是否有空白就是确定的。
//!
//! `Group` 走括号配对而不是逐字比对，顺带算出组内文本在原文里的范围，递归下去时
//! 直接切片就行，不必再为子层单独找原文。同时，只要 `Group` 自己的 `source_text`
//! 拿得到（真实宏展开里总是拿得到），就用它把候选钉死——否则「一个括号组」这样的
//! token 流会在原文里的每个左括号上都匹配成功。
//!
//! 匹配不上（例如 token 是 `@apply` 由 tw 展开出来的、根本不存在于原文中）时返回
//! `None`，由调用方回退到保守的类型启发式。

use proc_macro2::{Delimiter, TokenTree};
use std::ops::Range;

/// 一个 token 在原文里的位置信息。
#[derive(Clone, Debug, PartialEq)]
pub struct TokenSpacing {
    /// 它前面是否有空白（首个 token 恒为 `false`）
    pub space_before: bool,
    /// `Group` 组内文本在 region 中的字节范围（不含定界符）
    pub inner: Option<Range<usize>>,
}

impl TokenSpacing {
    fn new(space_before: bool) -> Self {
        Self {
            space_before,
            inner: None,
        }
    }
}

/// 在 `region` 里定位 `tokens`，返回每个 token 的位置信息。
///
/// 返回 `None` 表示无法确定，调用方不应假装知道。
pub fn recover(tokens: &[TokenTree], region: &str) -> Option<Vec<TokenSpacing>> {
    if tokens.is_empty() {
        return Some(Vec::new());
    }

    let mut found: Option<Vec<TokenSpacing>> = None;
    for start in 0..=region.len() {
        if !region.is_char_boundary(start) {
            continue;
        }
        let Some(candidate) = try_match(tokens, region, start) else {
            continue;
        };
        match &found {
            None => found = Some(candidate),
            // 原文里出现多处，但**结论**一致 —— 依然是确定的。
            // 比的是空白与组内文本，不是字节下标：同一段 CSS 写两遍时
            // 下标必然不同，结论却完全一样。
            Some(prev) if conclusion(region, prev) == conclusion(region, &candidate) => {}
            // 结论冲突：说不清是哪一处，交回给调用方
            Some(_) => return None,
        }
    }
    found
}

/// 一次匹配在语义上给出的结论：每个 token 前是否有空白 + 每个组的组内文本。
fn conclusion<'a>(src: &'a str, info: &[TokenSpacing]) -> Vec<(bool, Option<&'a str>)> {
    info.iter()
        .map(|i| (i.space_before, i.inner.clone().and_then(|r| src.get(r))))
        .collect()
}

/// 从 `start` 开始逐个吃掉 `tokens`，中间只允许空白/注释。
fn try_match(tokens: &[TokenTree], src: &str, start: usize) -> Option<Vec<TokenSpacing>> {
    let bytes = src.as_bytes();
    let mut pos = start;
    let mut out = Vec::with_capacity(tokens.len());

    for (i, tt) in tokens.iter().enumerate() {
        let space_before = if i == 0 {
            false
        } else {
            let next = skip_trivia(src, pos)?;
            let skipped = next > pos;
            pos = next;
            skipped
        };

        match tt {
            TokenTree::Group(g) => {
                // 隐式定界的组来自宏插值，原文里没有对应文本
                let (open, close) = delimiters(g.delimiter())?;
                if bytes.get(pos) != Some(&open) {
                    return None;
                }
                let end = scan_balanced(src, pos, open, close)?;
                // 真实宏展开里组能拿到自己的源码片段（含定界符与内部空白）。
                // 用它把候选钉死：否则单个组的 token 流会在原文里的**每个**
                // 左括号上都匹配成功，组内范围各不相同，结论互相冲突。
                if let Some(text) = g.span().source_text()
                    && src.get(pos..=end) != Some(text.as_str())
                {
                    return None;
                }
                out.push(TokenSpacing {
                    space_before,
                    inner: Some(pos + 1..end),
                });
                pos = end + 1;
            }
            other => {
                // Ident / Punct / Literal 的 `to_string()` 与原文逐字一致
                let atom = other.to_string();
                if !bytes[pos..].starts_with(atom.as_bytes()) {
                    return None;
                }
                out.push(TokenSpacing::new(space_before));
                pos += atom.len();
            }
        }
    }
    Some(out)
}

fn delimiters(d: Delimiter) -> Option<(u8, u8)> {
    match d {
        Delimiter::Parenthesis => Some((b'(', b')')),
        Delimiter::Brace => Some((b'{', b'}')),
        Delimiter::Bracket => Some((b'[', b']')),
        Delimiter::None => None,
    }
}

/// 从 `open_at`（指向开定界符）扫到配对的闭定界符，返回其下标。
///
/// 字符串字面量与注释里的括号不计入配对。
fn scan_balanced(src: &str, open_at: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    let mut pos = open_at;

    while pos < bytes.len() {
        match bytes[pos] {
            b'"' => {
                pos = skip_string(src, pos)?;
                continue;
            }
            b'/' if bytes[pos..].starts_with(b"//") || bytes[pos..].starts_with(b"/*") => {
                pos = skip_trivia(src, pos)?;
                continue;
            }
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

/// 跳过一个以 `"` 开头的字符串字面量（含 `r"…"` / `r#"…"#` 的收尾部分）。
fn skip_string(src: &str, open_at: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    // 数一下前面紧挨着几个 `#`，raw 字符串靠它决定结尾
    let mut hashes = 0usize;
    let mut back = open_at;
    while back > 0 && bytes[back - 1] == b'#' {
        hashes += 1;
        back -= 1;
    }
    let is_raw = back > 0 && (bytes[back - 1] == b'r' || bytes[back - 1] == b'b');

    let mut pos = open_at + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' if !is_raw => pos += 2,
            b'"' => {
                if !is_raw {
                    return Some(pos + 1);
                }
                let end = pos + 1 + hashes;
                if end <= bytes.len() && bytes[pos + 1..end].iter().all(|&c| c == b'#') {
                    return Some(end);
                }
                pos += 1;
            }
            _ => pos += 1,
        }
    }
    None
}

/// 跳过空白与 Rust 注释（块注释可嵌套），返回新的位置。
fn skip_trivia(src: &str, mut pos: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    loop {
        if pos >= bytes.len() {
            return Some(pos);
        }
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }
        if bytes[pos..].starts_with(b"//") {
            pos += 2;
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if bytes[pos..].starts_with(b"/*") {
            let mut depth = 1usize;
            pos += 2;
            while depth > 0 {
                if pos + 1 >= bytes.len() {
                    return None; // 注释未闭合：这段原文对不上，放弃本次匹配
                }
                if bytes[pos..].starts_with(b"/*") {
                    depth += 1;
                    pos += 2;
                } else if bytes[pos..].starts_with(b"*/") {
                    depth -= 1;
                    pos += 2;
                } else {
                    pos += 1;
                }
            }
            continue;
        }
        return Some(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;

    fn spaces(src: &str) -> Vec<bool> {
        let ts: TokenStream = src.parse().unwrap();
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        recover(&tokens, src)
            .expect("原文可用时不应放弃")
            .into_iter()
            .map(|i| i.space_before)
            .collect()
    }

    #[test]
    fn descendant_and_compound_are_distinguished() {
        // `& span` 的 `span` 前有空白，`&span` 没有
        assert_eq!(spaces("& span"), vec![false, true]);
        assert_eq!(spaces("&span"), vec![false, false]);
    }

    #[test]
    fn function_call_is_not_split_from_its_name() {
        // `not(...)` 紧贴，`and (...)` 分开
        assert_eq!(spaces("not(.a)"), vec![false, false]);
        assert_eq!(
            spaces("screen and (min-width: 1px)"),
            vec![false, true, true]
        );
    }

    #[test]
    fn class_chains_keep_their_shape() {
        assert_eq!(spaces(".a .b"), vec![false, false, true, false]);
        assert_eq!(spaces(".a.b"), vec![false, false, false, false]);
    }

    #[test]
    fn comments_count_as_separation() {
        assert_eq!(spaces("a/* x */b"), vec![false, true]);
    }

    #[test]
    fn group_inner_range_points_at_the_original_text() {
        let src = "&:not(.a .b)";
        let ts: TokenStream = src.parse().unwrap();
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        let info = recover(&tokens, src).unwrap();
        let inner = info
            .iter()
            .find_map(|i| i.inner.clone())
            .expect("括号组应当带上组内范围");
        assert_eq!(&src[inner], ".a .b");
    }

    #[test]
    fn braces_and_strings_do_not_confuse_the_scanner() {
        let src = r#"content: "a)b" ; div { color: red }"#;
        let ts: TokenStream = src.parse().unwrap();
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        let info = recover(&tokens, src).unwrap();
        let brace = info.last().unwrap().inner.clone().unwrap();
        assert_eq!(&src[brace], " color: red ");
    }

    #[test]
    fn tokens_absent_from_the_source_are_reported_as_unknown() {
        let ts: TokenStream = "& span".parse().unwrap();
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        assert!(recover(&tokens, "something else entirely").is_none());
    }
}
