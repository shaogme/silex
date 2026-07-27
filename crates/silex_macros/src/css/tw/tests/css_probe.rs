//! 对 `tw!` 产物做结构化断言的公共工具。
//!
//! `differential.rs`（与真实 Tailwind 对拍）与 `e2e.rs`（与生成夹具对拍）都需要
//! "把编译出来的 CSS 拆成声明、再抹平无语义的格式差异"这套能力。放在一处，
//! 免得两边各写一份规范化规则又各自漂移——报告 §3.1 讲的正是重复实现的下场。

use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// 编译单个 tw 词条并返回最终 CSS（component_css + static_css）
pub fn compile_class(class: &str) -> Result<String, String> {
    let input: TwInput = syn::parse2(quote!(#class)).map_err(|e| e.to_string())?;
    let block = build_css_block_from_tw(input).map_err(|e| e.to_string())?;
    let compiled = crate::css::compiler::CssCompiler::compile_block(
        &block,
        proc_macro2::Span::call_site(),
        false,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "{}\n{}",
        compiled.component_css, compiled.static_css
    ))
}

/// 编译单个 tw 词条并抽取其全部声明
pub fn class_declarations(class: &str) -> Result<Vec<(String, String)>, String> {
    compile_class(class).map(|css| extract_declarations(&css))
}

// ---------------------------------------------------------------------------
// 声明抽取
// ---------------------------------------------------------------------------

/// 从一段 CSS 文本里抽取所有 `prop: value` 声明（含嵌套 at-rule 内部的）。
///
/// 手写扫描而非正则：值里可能含 `;`（如 `content: "a;b"`）与嵌套括号
/// （`repeat(3, minmax(0, 1fr))`），正则会切错。
pub fn extract_declarations(css: &str) -> Vec<(String, String)> {
    let bytes: Vec<char> = css.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    // 当前收集中的 token（可能是选择器/at-rule 前奏，也可能是属性名）
    let mut buf = String::new();

    while i < bytes.len() {
        match bytes[i] {
            '{' | '}' => {
                buf.clear();
                i += 1;
            }
            ';' => {
                buf.clear();
                i += 1;
            }
            ':' => {
                let prop = buf.trim().to_string();
                buf.clear();
                // 伪类/伪元素出现在选择器里也会有 `:`——属性名不含空格且不以 `.`/`&` 开头
                if !is_property_name(&prop) {
                    i += 1;
                    continue;
                }
                // 扫描到值的结尾：`;` 或 `}`（在括号/引号外）
                let mut value = String::new();
                let mut depth = 0i32;
                let mut quote: Option<char> = None;
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if let Some(q) = quote {
                        value.push(c);
                        if c == q {
                            quote = None;
                        }
                        i += 1;
                        continue;
                    }
                    match c {
                        '"' | '\'' => {
                            quote = Some(c);
                            value.push(c);
                        }
                        '(' | '[' => {
                            depth += 1;
                            value.push(c);
                        }
                        ')' | ']' => {
                            depth -= 1;
                            value.push(c);
                        }
                        ';' if depth <= 0 => {
                            i += 1;
                            break;
                        }
                        '}' if depth <= 0 => break,
                        '{' if depth <= 0 => {
                            // `prop:` 后面跟 `{` 说明这其实是选择器（如 `&:hover {`），丢弃
                            value.clear();
                            break;
                        }
                        _ => value.push(c),
                    }
                    i += 1;
                }
                let value = value.trim().to_string();
                if !value.is_empty() {
                    out.push((prop, value));
                }
            }
            c => {
                buf.push(c);
                i += 1;
            }
        }
    }
    out
}

pub fn is_property_name(s: &str) -> bool {
    !s.is_empty()
        && !s.contains(char::is_whitespace)
        && !s.starts_with('.')
        && !s.starts_with('&')
        && !s.starts_with('@')
        && !s.starts_with('*')
        && !s.contains('(')
        && !s.contains(')')
        && !s.contains('>')
        && !s.contains('+')
        && !s.contains('[')
        && !s.contains(']')
        && (s.starts_with("--")
            || s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '-')
}

/// 把声明序列折叠成 `属性 → 值集合`。
///
/// 用集合而非单值：`container` 会在不同断点下多次给 `max-width` 赋值，
/// 两侧的断点包装方式不同，能比的只有"出现过哪些值"。
pub fn decls_to_map<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
    decls: I,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (prop, value) in decls {
        map.entry(prop.to_string())
            .or_default()
            .insert(normalize_value(value));
    }
    map
}

// ---------------------------------------------------------------------------
// 值规范化
// ---------------------------------------------------------------------------

/// 抹平两侧格式差异后再比较：
/// LightningCSS 会把 `0.5rem` 压成 `.5rem`、去掉逗号后空格、把 `#ffffff` 压成 `#fff`。
/// 这些是无语义差别的排版差异，不该让对拍失败。
///
/// **保留 token 之间的单个空格**——`animation: spin 1s linear infinite` 这类简写属性
/// 需要按 token 比较（LightningCSS 会重排顺序），全部抹掉空格就没法拆分了。
pub fn normalize_value(value: &str) -> String {
    let lowered = value.to_ascii_lowercase().replace(['"', '\''], "");
    let mut out = String::with_capacity(lowered.len());
    let mut chars = lowered.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // 空白折叠成单个空格
            c if c.is_whitespace() => {
                while chars.peek().is_some_and(|n| n.is_whitespace()) {
                    chars.next();
                }
                if !out.is_empty() {
                    out.push(' ');
                }
            }
            _ => out.push(c),
        }
    }

    // 分隔符两侧的空格无意义：`repeat(3, minmax(0, 1fr))` ≡ `repeat(3,minmax(0,1fr))`
    let mut compacted = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    for (i, &c) in bytes.iter().enumerate() {
        if c == ' ' {
            // 只吃掉分隔符**内侧**的空格。`)` 之后的空格必须保留，否则
            // `cubic-bezier(…) infinite` 会粘成一个 token，简写属性就没法按 token 比较了。
            let prev = compacted.chars().last();
            let next = bytes.get(i + 1).copied();
            if matches!(prev, Some(',') | Some('(') | Some('/'))
                || matches!(next, Some(',') | Some(')') | Some('/'))
            {
                continue;
            }
        }
        compacted.push(c);
    }

    round_numbers(&canonicalize_colors(compacted.trim()))
}

/// 把值里的颜色统一成 8 位小写 hex。
///
/// 同一个颜色在两侧可能写成 `rgba(0, 0, 0, 0.05)`、`#0000000d`、`#fff`——
/// LightningCSS 按最短形式输出，夹具里保留的是源码里的写法。
fn canonicalize_colors(s: &str) -> String {
    let s = &expand_transparent_keyword(s);
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();

    loop {
        let hash = rest.find('#');
        let func = ["rgba(", "rgb("]
            .iter()
            .filter_map(|f| rest.find(f).map(|i| (i, *f)))
            .min_by_key(|(i, _)| *i);

        match (hash, func) {
            (Some(h), f) if f.is_none_or(|(i, _)| h < i) => {
                out.push_str(&rest[..h]);
                let after = &rest[h + 1..];
                let len = after
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .count()
                    .min(8);
                out.push_str(&normalize_hex(&after[..len]));
                rest = &after[len..];
            }
            (_, Some((i, kw))) => {
                out.push_str(&rest[..i]);
                let after = &rest[i + kw.len()..];
                match after.find(')') {
                    Some(end) => {
                        match parse_rgb_args(&after[..end]) {
                            Some(hex) => out.push_str(&hex),
                            None => {
                                out.push_str(kw);
                                out.push_str(&after[..end]);
                                out.push(')');
                            }
                        }
                        rest = &after[end + 1..];
                    }
                    None => {
                        out.push_str(kw);
                        out.push_str(after);
                        return out;
                    }
                }
            }
            _ => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

/// `transparent` 关键字展开成 `#00000000`。
///
/// 规范里 `transparent` 就是 `rgba(0, 0, 0, 0)`，LightningCSS 会把它压成 `#0000`；
/// 夹具里保留的是 Tailwind 源码的关键字写法。这是无损改写，两种写法必须视作同一个值。
/// 只替换独立 token——`transparent-ish` 这种自定义标识符不能被吃掉。
fn expand_transparent_keyword(s: &str) -> String {
    const KW: &str = "transparent";
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';

    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(KW) {
        let before_ok = rest[..i].chars().next_back().is_none_or(|c| !is_word(c));
        let after = &rest[i + KW.len()..];
        let after_ok = after.chars().next().is_none_or(|c| !is_word(c));
        out.push_str(&rest[..i]);
        out.push_str(if before_ok && after_ok {
            "#00000000"
        } else {
            KW
        });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// 3/4/6/8 位 hex 一律展开成 8 位；不透明时省略 alpha
fn normalize_hex(hex: &str) -> String {
    let expanded: String = match hex.len() {
        3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => hex.to_string(),
        _ => return format!("#{hex}"),
    };
    match expanded.len() {
        8 if expanded.ends_with("ff") => format!("#{}", &expanded[..6]),
        _ => format!("#{expanded}"),
    }
}

/// `0,0,0,0.05` → `#0000000d`
fn parse_rgb_args(args: &str) -> Option<String> {
    let parts: Vec<&str> = args.split([',', '/']).map(str::trim).collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let channel = |p: &str| -> Option<u8> {
        match p.strip_suffix('%') {
            Some(n) => n
                .parse::<f64>()
                .ok()
                .map(|v| (v * 255.0 / 100.0).round() as u8),
            None => p.parse::<f64>().ok().map(|v| v.round() as u8),
        }
    };
    let (r, g, b) = (channel(parts[0])?, channel(parts[1])?, channel(parts[2])?);
    let alpha = match parts.get(3) {
        None => 255u8,
        Some(a) => match a.strip_suffix('%') {
            Some(n) => (n.parse::<f64>().ok()? * 255.0 / 100.0).round() as u8,
            None => (a.parse::<f64>().ok()? * 255.0).round() as u8,
        },
    };
    Some(if alpha == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{alpha:02x}")
    })
}

/// 函数层面的等价改写。
///
/// * `translateX(v)` 与单参数的 `translate(v)` 是同一个变换，LightningCSS 输出后者。
///   只做这一处：`scaleX` vs `scale` 的参数语义不同，不能合并。
/// * `blur()` 省略参数时按规范取 0（filter effects 规范的 lacuna value），
///   LightningCSS 会把 `blur(0px)` 压成 `blur()`。
fn canonical_functions(value: &str) -> String {
    value
        .replace("translatex(", "translate(")
        .replace("blur()", "blur(0)")
}

/// 单个 token 的等价规范化。
///
/// 全是 LightningCSS 的合法改写：`center` → `50%`（位置关键字）、
/// `flex-end` → `end`（对齐关键字在 v4 里已统一）、`100ms` ↔ `.1s`（时间单位）。
/// 这些改写不改变渲染结果，把它们当成差异只会淹没真正的问题。
pub fn canonical_token(token: &str) -> String {
    match token {
        "center" => return "50%".to_string(),
        "flex-end" => return "end".to_string(),
        "flex-start" => return "start".to_string(),
        _ => {}
    }
    if let Some(ms) = parse_time_ms(token) {
        return format!("{ms}ms");
    }
    token.to_string()
}

/// `1.5s` / `150ms` → 毫秒数；不是时间值时返回 `None`
pub fn parse_time_ms(token: &str) -> Option<f64> {
    if let Some(num) = token.strip_suffix("ms") {
        num.parse::<f64>().ok()
    } else if let Some(num) = token.strip_suffix('s') {
        num.parse::<f64>().ok().map(|v| v * 1000.0)
    } else {
        None
    }
}

/// 两个规范化后的值是否等价。
///
/// 依次尝试：逐字符相等 → token 多重集相等（吸收简写属性的重排）→ 属性特定的等价规则。
pub fn values_equivalent(prop: &str, expected: &str, actual: &str) -> bool {
    if expected == actual {
        return true;
    }

    let canon = |v: &str| {
        let mut tokens: Vec<String> = canonical_functions(v)
            .split(' ')
            .map(canonical_token)
            .collect();
        tokens.sort();
        tokens
    };
    if canon(expected) == canon(actual) {
        return true;
    }

    // `opacity: 50%` ≡ `opacity: .5`（这些属性同时接受百分比与无单位数）
    if matches!(
        prop,
        "opacity" | "fill-opacity" | "stroke-opacity" | "stop-opacity"
    ) && let (Some(e), Some(a)) = (as_ratio(expected), as_ratio(actual))
    {
        return (e - a).abs() < 1e-6;
    }

    // `aspect-ratio: 1/1` ≡ `aspect-ratio: 1`
    if prop == "aspect-ratio"
        && let (Some(e), Some(a)) = (as_ratio_pair(expected), as_ratio_pair(actual))
    {
        return (e - a).abs() < 1e-6;
    }

    // 阴影的扩散半径缺省即 0，LightningCSS 会省略它：
    // `0 1px 3px 0 #0000001a` ≡ `0 1px 3px #0000001a`。
    // 只对阴影类属性放开——`margin: 0 1rem` 与 `margin: 1rem` 可不等价。
    if prop == "box-shadow" || prop == "text-shadow" || prop.starts_with("--tw-shadow") {
        let drop_zeros = |v: &str| {
            v.split(',')
                .map(|layer| {
                    layer
                        .split(' ')
                        .filter(|t| *t != "0")
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        if drop_zeros(expected) == drop_zeros(actual) {
            return true;
        }
    }

    // 无穷大圆角半径 / 长度（Tailwind v4 产出 calc(infinity * 1px)，LightningCSS 求值化简为 3.4028e38px，旧值 9999px）
    let is_infinity_length = |v: &str| -> bool {
        let s = v.trim();
        s == "calc(infinity * 1px)"
            || s == "calc(infinity*1px)"
            || s == "9999px"
            || (s.starts_with("3.4028") && s.ends_with("px"))
            || s.starts_with("calc(3.4028")
    };
    if is_infinity_length(expected) && is_infinity_length(actual) {
        return true;
    }

    false
}

/// `50%` → 0.5；`.5` → 0.5
pub fn as_ratio(value: &str) -> Option<f64> {
    match value.strip_suffix('%') {
        Some(num) => num.parse::<f64>().ok().map(|v| v / 100.0),
        None => value.parse::<f64>().ok(),
    }
}

/// `1/1` → 1.0；`1.5` → 1.5
pub fn as_ratio_pair(value: &str) -> Option<f64> {
    match value.split_once('/') {
        Some((a, b)) => {
            let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
            (b != 0.0).then_some(a / b)
        }
        None => value.parse::<f64>().ok(),
    }
}

/// 可以在数值为 0 时省略的单位——`0deg` ≡ `0`，LightningCSS 会做这个压缩
const DROPPABLE_ZERO_UNITS: &[&str] = &["px", "rem", "em", "deg", "rad", "turn", "ms", "s", "%"];

/// 把值里的所有数字规范化：统一带前导零、截到 4 位小数、零值去掉单位。
///
/// 三处纯格式差异：LightningCSS 把 `0.25rem` 压成 `.25rem`、把 `rotate(0deg)` 压成
/// `rotate(0)`；分数类工具（`w-4/6`）两侧各自做浮点除法再格式化，Tailwind 给
/// `66.666667%` 而 LightningCSS 给 `66.6667%`。
///
/// **跳过 `#` 后面的 hex 串**——`#0000000d` 里的 `0000000` 不是数字，
/// 按数字处理会把颜色压成 `#0d`。
pub fn round_numbers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' {
            out.push('#');
            i += 1;
            while i < chars.len() && chars[i].is_ascii_hexdigit() {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            match raw.parse::<f64>() {
                Ok(n) => {
                    let rounded = (n * 10_000.0).round() / 10_000.0;
                    // `-0` ≡ `0`（`-outline-offset-0` 会产出前者）。只吃紧贴数字的负号，
                    // `calc(100% - 0px)` 里的减号前有空格，不受影响。
                    if rounded == 0.0 && out.ends_with('-') {
                        out.pop();
                    }
                    // `{}` 对 f64 总是带前导零（0.25），`.25rem` 与 `0.25rem` 就此统一
                    let _ = write!(out, "{rounded}");
                    if rounded == 0.0 {
                        let unit_start = i;
                        while i < chars.len() && chars[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        let unit: String = chars[unit_start..i].iter().collect();
                        if i < chars.len() && chars[i] == '%' {
                            i += 1;
                        } else if !DROPPABLE_ZERO_UNITS.contains(&unit.as_str()) {
                            out.push_str(&unit);
                        }
                    }
                }
                Err(_) => out.push_str(&raw),
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 规范化器自身的单元测试——它写错会让上层所有断言失去意义
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_value_erases_only_formatting() {
        assert_eq!(normalize_value("0.5rem"), normalize_value(".5rem"));
        assert_eq!(
            normalize_value("repeat(3, minmax(0, 1fr))"),
            normalize_value("repeat(3,minmax(0,1fr))")
        );
        assert_eq!(normalize_value("#FFF"), normalize_value("#ffffff"));
        assert_eq!(normalize_value("0px"), normalize_value("0"));
        assert_eq!(
            normalize_value("2px solid transparent"),
            normalize_value("2px solid #0000")
        );
        // 只吃独立 token
        assert_ne!(
            normalize_value("var(--transparent-bg)"),
            normalize_value("var(--#00000000-bg)")
        );
        // 真实差异必须保留
        assert_ne!(normalize_value("1rem"), normalize_value("1px"));
        assert_ne!(normalize_value("#fb2c36"), normalize_value("#fb2c37"));
        assert_ne!(normalize_value("0.5rem"), normalize_value("5rem"));
    }

    #[test]
    fn values_equivalent_accepts_only_lossless_rewrites() {
        // LightningCSS 的合法改写
        assert!(values_equivalent(
            "animation",
            &normalize_value("spin 1s linear infinite"),
            &normalize_value("1s linear infinite spin")
        ));
        assert!(values_equivalent(
            "transition-delay",
            &normalize_value("100ms"),
            &normalize_value("0.1s")
        ));
        assert!(values_equivalent(
            "opacity",
            &normalize_value("50%"),
            &normalize_value("0.5")
        ));
        // 语义不同的值必须判为不等
        assert!(!values_equivalent(
            "animation",
            &normalize_value("spin 1s linear infinite"),
            &normalize_value("spin 2s linear infinite")
        ));
        assert!(!values_equivalent(
            "opacity",
            &normalize_value("50%"),
            &normalize_value("0.6")
        ));
        assert!(!values_equivalent(
            "padding",
            &normalize_value("1rem"),
            &normalize_value("1px")
        ));
    }

    #[test]
    fn extract_declarations_handles_nesting_and_parens() {
        let css = ".a{grid-template-columns:repeat(3, minmax(0, 1fr));color:red}\
                   @media (min-width:768px){.a{padding:1rem}}";
        assert_eq!(
            extract_declarations(css),
            vec![
                (
                    "grid-template-columns".to_string(),
                    "repeat(3, minmax(0, 1fr))".to_string()
                ),
                ("color".to_string(), "red".to_string()),
                ("padding".to_string(), "1rem".to_string()),
            ]
        );
    }

    #[test]
    fn extract_declarations_ignores_selector_colons() {
        let css = ".a:hover{color:red}.group[data-state=open] .b{display:flex}";
        assert_eq!(
            extract_declarations(css),
            vec![
                ("color".to_string(), "red".to_string()),
                ("display".to_string(), "flex".to_string()),
            ]
        );
    }
}
