//! 写入 CSS 文本前的值净化。
//!
//! 动态值走 `element.style.setProperty`，由 CSSOM 过滤，天然安全。**静态**值
//! 却是直接 `write!("{}: {};")` 拼进样式表的，而 85% 的属性组都接受 `String`，
//! 于是任何用户可控字符串都能闭合当前声明、注入新规则：
//!
//! ```text
//! sty().grid_template_areas("a\"; color:red; x:\"")
//! //  → grid-template-areas: "a"; color:red; x:"";
//! ```
//!
//! 这里把值挡在声明边界内。同一个值来源不该因为走静态还是动态而有不同的安全性质。

use std::{borrow::Cow, fmt::Write as _};

/// 把任意字符串写成一个 CSS `<string>`（含引号）。
pub fn css_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            // CSS 字符串里不允许裸换行
            '\n' => out.push_str("\\A "),
            '\r' => out.push_str("\\D "),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 净化一个属性名，保证它不会越出 `<prop>: value;` 的左半边。
///
/// 注册表里的属性名是 `&'static str` 常量，天然安全；但 `Style::var()` 与
/// `Style::raw()` 的名字来自调用方，一个 `:` 就能把一条声明劈成两条。
///
/// 合法的名字原样返回（`Cow::Borrowed`），不产生额外分配。不合法的字符按 CSS
/// 的标识符转义写成 `\<hex> `——转义后的名字仍然是一个**单一**标识符，浏览器
/// 认不出这个属性会整条丢弃，而不会执行它。
pub fn property_name(name: &str) -> Cow<'_, str> {
    fn is_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'
            || (!c.is_ascii() && !c.is_whitespace() && !c.is_control())
    }

    let leading_digit = name.starts_with(|c: char| c.is_ascii_digit());
    if !leading_digit && name.chars().all(is_name_char) {
        return Cow::Borrowed(name);
    }

    let mut out = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if is_name_char(ch) && !(i == 0 && ch.is_ascii_digit()) {
            out.push(ch);
        } else {
            // `\<hex> ` 是 CSS 的标识符转义，尾随空格是转义序列的终止符
            let _ = write!(out, "\\{:x} ", ch as u32);
        }
    }
    Cow::Owned(out)
}

/// 将动态选择器片段限制为单个 CSS 标识符。
///
/// 选择器上下文不能复用声明值净化：`,` 会扩大选择器列表，空白会引入新的
/// 后代选择器，而 `:`、`[` 和 `(` 会改变匹配语义。将非标识符字符编码为 CSS
/// 转义序列后，整个输入仍是一个选择器片段，不会越出原来的选择器边界。
pub fn selector_fragment(value: &str) -> Cow<'_, str> {
    let first = value.chars().next();
    let clean = |(index, ch): (usize, char)| {
        (ch.is_ascii_alphanumeric()
            || ch == '-'
            || ch == '_'
            || (!ch.is_ascii() && !ch.is_whitespace() && !ch.is_control()))
            && !(index == 0 && ch.is_ascii_digit())
            && !(index == 1 && first == Some('-') && ch.is_ascii_digit())
    };

    if value.chars().enumerate().all(clean) {
        return Cow::Borrowed(value);
    }

    let mut out = String::with_capacity(value.len() + 4);
    for (index, ch) in value.chars().enumerate() {
        if clean((index, ch)) {
            out.push(ch);
        } else {
            // 尾随空格用于终止十六进制转义，避免它吞掉后面的字符。
            let _ = write!(out, "\\{:x} ", ch as u32);
        }
    }
    Cow::Owned(out)
}

/// 净化一条声明的值，保证它不会越出 `prop: <value>;` 的边界。
///
/// 顶层（不在字符串、不在括号里）出现的 `;`、`{`、`}` 会被替换成等价的 CSS
/// 转义序列——这些字符在合法的属性值里根本不会出现，出现即意味着越界。
/// 未闭合的引号与括号会在末尾补齐，避免把后续规则整体吞掉。
///
/// 值本身合法时原样返回（`Cow::Borrowed`），不产生额外分配。
pub fn declaration_value(value: &str) -> Cow<'_, str> {
    if is_clean(value) {
        return Cow::Borrowed(value);
    }

    let mut out = String::with_capacity(value.len() + 8);
    let mut quote: Option<char> = None;
    let mut depth: usize = 0;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                escaped = true;
                out.push(ch);
            }
            '"' | '\'' => {
                match quote {
                    Some(q) if q == ch => quote = None,
                    Some(_) => {}
                    None => quote = Some(ch),
                }
                out.push(ch);
            }
            _ if quote.is_some() => out.push(ch),
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                // 多余的 `)` 会提前关掉外层函数，同样是越界
                if depth == 0 {
                    out.push_str("\\29 ");
                } else {
                    depth -= 1;
                    out.push(ch);
                }
            }
            ';' if depth == 0 => out.push_str("\\3b "),
            '{' if depth == 0 => out.push_str("\\7b "),
            '}' if depth == 0 => out.push_str("\\7d "),
            c => out.push(c),
        }
    }

    // 结尾处于未闭合状态：补齐，别让它吃掉后面的规则
    if escaped {
        out.push('\\');
    }
    if let Some(q) = quote {
        out.push(q);
    }
    for _ in 0..depth {
        out.push(')');
    }
    Cow::Owned(out)
}

/// 快速判断：没有任何越界字符、且引号与括号都是平衡的。
fn is_clean(value: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut depth: usize = 0;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' | '\'' => match quote {
                Some(q) if q == ch => quote = None,
                Some(_) => {}
                None => quote = Some(ch),
            },
            _ if quote.is_some() => {}
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            ';' | '{' | '}' if depth == 0 => return false,
            _ => {}
        }
    }
    !escaped && quote.is_none() && depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_values_are_untouched() {
        for v in [
            "10px",
            "rgba(0, 0, 0, .5)",
            "\"Segoe UI\", sans-serif",
            "url(\"a b.png\")",
            "calc(100% - 10px)",
            "\"a b\" \"c d\"",
        ] {
            assert!(matches!(declaration_value(v), Cow::Borrowed(_)), "{v}");
        }
    }

    #[test]
    fn declaration_boundary_cannot_be_escaped() {
        let injected = declaration_value("red; } body { display: none");
        assert!(!injected.contains(';'), "{injected}");
        assert!(!injected.contains('{'), "{injected}");
        assert!(!injected.contains('}'), "{injected}");
    }

    #[test]
    fn unterminated_string_is_closed() {
        let v = declaration_value("\"a");
        assert_eq!(v, "\"a\"");
    }

    #[test]
    fn unbalanced_parens_are_repaired() {
        assert_eq!(declaration_value("calc(1px"), "calc(1px)");
        assert_eq!(declaration_value("1px)"), "1px\\29 ");
    }

    #[test]
    fn semicolons_inside_strings_and_functions_are_kept() {
        // 字符串与函数内部的 `;` 不越界，保持原样
        assert_eq!(declaration_value("\"a;b\""), "\"a;b\"");
    }

    #[test]
    fn css_string_escapes_quotes() {
        assert_eq!(css_string("a\"; x:\""), "\"a\\\"; x:\\\"\"");
    }

    #[test]
    fn ordinary_property_names_are_untouched() {
        for n in [
            "color",
            "--brand-primary",
            "-webkit-font-smoothing",
            "_private",
        ] {
            assert!(matches!(property_name(n), Cow::Borrowed(_)), "{n}");
        }
    }

    /// 名字来自调用方时，一个 `:` 就能把一条声明劈成两条
    #[test]
    fn a_property_name_cannot_open_a_second_declaration() {
        let n = property_name("color: red; background");
        assert!(!n.contains(':'), "{n}");
        assert!(!n.contains(';'), "{n}");
        assert!(!n.contains(' ') || n.contains('\\'), "{n}");
    }

    #[test]
    fn a_leading_digit_is_escaped() {
        assert_eq!(property_name("1x"), "\\31 x");
    }

    #[test]
    fn selector_fragments_cannot_expand_the_selector() {
        let fragment = selector_fragment(", body:hover [data-x='y']");
        assert!(!fragment.contains(','), "{fragment}");
        assert!(!fragment.contains(':'), "{fragment}");
        assert!(!fragment.contains('['), "{fragment}");
        assert!(!fragment.contains(']'), "{fragment}");
        assert!(!fragment.contains('('), "{fragment}");
        assert!(!fragment.contains(')'), "{fragment}");
        assert!(selector_fragment("dark") == "dark");
        assert!(selector_fragment("-1x").contains("\\31 x"));
        assert!(selector_fragment("a\u{2003}b").contains("\\2003 "));
    }
}
