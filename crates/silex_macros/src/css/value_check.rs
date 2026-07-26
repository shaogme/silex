//! `css!` / `styled!` 里**静态**取值的校验。
//!
//! 阶段四只接回了「一眼能定型」的字面量：`color: 10px` 与 `width: #fff` 会被
//! `ValidFor` 拦住（见 `compiler::classify_static_value`），其余形态一律放行——
//! 当时宏侧没有可靠的判据，宁可漏也不能误报。于是这些全是编译通过、无警告、
//! 浏览器丢弃：
//!
//! ```text
//! align-items: centre;        // 拼错的关键字
//! align-items: rgb(0 0 0);    // 不接受颜色的属性写了颜色函数
//! color: 1px solid red;       // 单值属性写了多分量
//! ```
//!
//! 判据现在有了：阶段二为 `define_props!` 写的那个 MDN 值定义语法解析器，其
//! `Analysis { singles, keywords, multi }` 正是判断一条取值合不合法所需的全部
//! 信息。`silex_codegen` 把它导出成两张表（[`property_keywords`]、
//! [`property_caps`]），这里按三层判据消费：
//!
//! | 层 | 判据 | 招牌反例 |
//! | --- | --- | --- |
//! | 裸关键字 | 单个 ident ∉ 该属性的关键字表 | `align-items: centre` |
//! | 函数式取值 | 函数产出的能力 ∉ 属性的能力集 | `align-items: rgb(0 0 0)` |
//! | 多分量 | 取值有顶层空白 ∧ 属性不是 `MULTI` | `color: 1px solid red` |
//!
//! **不误报优先。** 每一层都有放行闸门，任何拿不准的形态一律放过：
//!
//! - 属性查不到（自定义变量、注册表外的厂商前缀属性）→ 整条跳过
//! - 属性是 `OPEN`（语法里有 `<custom-ident>` / `<string>` / 解析不出来的引用）
//!   → 整条跳过，`animation-name: fadeIn`、`font-family: Inter` 靠这一条过关
//! - 关键字以 `-` 开头（`display: -webkit-box`）→ 跳过，MDN 没有厂商关键字的数据
//! - 有效关键字表为空 → 跳过，表为空说明我们对这个属性一无所知
//! - 函数名不认识 → 跳过，只有确定语义的那十几个函数才参与判断
//!
//! 逃生口有三层：`unsafe { … }` 块（`state.is_unsafe`）、`silex.toml` 的
//! `[css.validation]` 三个开关（`error` / `warn` / `off`），以及 `Style::raw()`。

use proc_macro2::Span;
use syn::Result;

use crate::css::compiler::CssWarning;
use crate::css::config::{ValidationLevel, validation_levels};
use crate::css::property_caps::{PROPERTY_CAPS, cap};
use crate::css::property_keywords::{COLOR_KEYWORDS, PROPERTY_KEYWORDS, UNIVERSAL_KEYWORDS};
use crate::css::table::{canonical_property_name, closest_match};

/// 三层判据各自对应 `[css.validation]` 里的一个开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Keywords,
    Functions,
    Arity,
}

impl Layer {
    fn level(self) -> ValidationLevel {
        let cfg = validation_levels();
        match self {
            Self::Keywords => cfg.keywords,
            Self::Functions => cfg.functions,
            Self::Arity => cfg.arity,
        }
    }
}

/// 校验一条**没有任何插值**的静态取值。
///
/// 调用点在 `compiler::process_css_block`，且已经过了 `state.validate &&
/// !state.is_unsafe` 这道闸门。有插值的取值不走这里——插值的类型由
/// `ValidFor` 在展开产物里管，取值文本里的 `var(--…)` 占位符没什么可查的。
pub(crate) fn check_static_value(
    property: &str,
    value: &str,
    span: Span,
    warnings: &mut Vec<CssWarning>,
) -> Result<()> {
    // 注册表外的属性名没有语法数据。`--*` 与厂商前缀属性走的正是这条路，
    // 它们本来就是「原样透传」的出口
    let Some(prop) = canonical_property_name(property) else {
        return Ok(());
    };
    let Some(caps) = caps_of(prop) else {
        return Ok(());
    };
    // 语法里有裸标识符/字符串，或者有解析不出来的引用：这个属性能取什么
    // 我们并不知道，三层判据全部放行
    if caps & cap::OPEN != 0 {
        return Ok(());
    }

    let value = strip_important(value.trim());
    if value.is_empty() {
        return Ok(());
    }

    // --- A-3　多分量 ---
    let segments = top_level_segments(value);
    if segments.len() > 1 {
        if caps & cap::MULTI == 0 {
            return report(
                Layer::Arity,
                span,
                warnings,
                arity_message(prop, value, segments.len()),
            );
        }
        // 分量个数合法时不再逐个分量查关键字：`border: 1px solid red` 里的
        // `solid` 与 `red` 各自属于不同的子语法，逐个查会把 `<line-width>` 的
        // 关键字表套到 `<color>` 的位置上去
        return Ok(());
    }

    // --- A-1　裸关键字 ---
    if let Some(kw) = as_bare_keyword(value) {
        return check_keyword(prop, caps, kw, span, warnings);
    }

    // --- A-2　函数式取值 ---
    if let Some(name) = as_function_name(value) {
        return check_function(prop, caps, &name, span, warnings);
    }

    Ok(())
}

// ==========================================
// A-1　裸关键字
// ==========================================

fn check_keyword(
    prop: &str,
    caps: u16,
    kw: &str,
    span: Span,
    warnings: &mut Vec<CssWarning>,
) -> Result<()> {
    let lower = kw.to_ascii_lowercase();

    // `inherit` / `initial` / `unset` / `revert` / `revert-layer` 对每个属性都合法
    if UNIVERSAL_KEYWORDS.binary_search(&lower.as_str()).is_ok() {
        return Ok(());
    }

    let listed = keywords_of(prop).unwrap_or(&[]);
    if listed.binary_search(&lower.as_str()).is_ok() {
        return Ok(());
    }

    // 属性侧的语法分析把 `<color>` 当作终点，所以具名颜色不在任何属性的
    // 关键字表里——接受颜色的属性额外拿全局颜色表放行。
    // `caret-color: red`、`border: red` 靠的就是这一条
    let takes_color = caps & cap::COLOR != 0;
    if takes_color && COLOR_KEYWORDS.binary_search(&lower.as_str()).is_ok() {
        return Ok(());
    }

    // 有效关键字表为空 = 我们对这个属性的关键字一无所知 → 放行。
    // 这是**误报闸门**：MDN 数据对某些属性的关键字收录不全，只在「这个属性
    // 明明有一张关键字表、而你写的词不在表上」时才判错
    if listed.is_empty() && !takes_color {
        return Ok(());
    }

    let mut msg = format!("`{kw}` 不是 CSS 属性 `{prop}` 的合法取值");
    let candidates = listed
        .iter()
        .copied()
        .chain(if takes_color {
            COLOR_KEYWORDS.iter().copied()
        } else {
            [].iter().copied()
        })
        .chain(UNIVERSAL_KEYWORDS.iter().copied());
    if let Some(hit) = closest_match(&lower, candidates) {
        msg.push_str(&format!("，是否想写 `{hit}`？"));
    }
    if !listed.is_empty() {
        msg.push_str(&format!("\n注：`{prop}` 接受 {}", preview(listed)));
        if takes_color {
            msg.push_str("，以及任意具名颜色");
        }
    } else {
        msg.push_str(&format!("\n注：`{prop}` 只接受颜色"));
    }
    msg.push_str("。\n实验特性等表外的取值请放进 `unsafe { … }` 块里原样透传。");
    report(Layer::Keywords, span, warnings, msg)
}

/// 关键字表太长时只列前几项——报错里塞 29 个 `display` 关键字没人会读。
fn preview(listed: &[&str]) -> String {
    const MAX: usize = 8;
    let head: Vec<&str> = listed.iter().take(MAX).copied().collect();
    if listed.len() > MAX {
        format!("{} 等 {} 个关键字", head.join(" / "), listed.len())
    } else {
        head.join(" / ")
    }
}

/// 取值是不是一个可以查表的裸关键字。
///
/// 特意**不认**以 `-` 开头的标识符：`display: -webkit-box`、
/// `-webkit-appearance: -apple-pay-button` 这类厂商关键字在 MDN 数据里没有
/// 收录，认了就会把真实可用的写法拒掉。
fn as_bare_keyword(value: &str) -> Option<&str> {
    let mut chars = value.chars();
    if !chars.next()?.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Some(value)
    } else {
        None
    }
}

// ==========================================
// A-2　函数式取值
// ==========================================

fn check_function(
    prop: &str,
    caps: u16,
    name: &str,
    span: Span,
    warnings: &mut Vec<CssWarning>,
) -> Result<()> {
    let Some((needed, what)) = function_capability(name) else {
        return Ok(());
    };
    if caps & needed != 0 {
        return Ok(());
    }
    let msg = format!(
        "`{name}()` 产出的是{what}，而 CSS 属性 `{prop}` 不接受{what}\n\
         注：`{prop}` 只接受{}。\n\
         实验特性等表外的取值请放进 `unsafe {{ … }}` 块里原样透传。",
        describe_caps(caps)
    );
    report(Layer::Functions, span, warnings, msg)
}

/// 函数名 → (它产出的值需要哪一位能力, 人话描述)。
///
/// 手写而非生成：CSS 的取值函数是有限且稳定的，而 MDN 的 `syntaxes.json` 里
/// 函数的产出类型并不好机械提取。返回 `None` 的一律放行。
fn function_capability(name: &str) -> Option<(u16, &'static str)> {
    Some(match name {
        "rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color"
        | "color-mix" | "light-dark" | "device-cmyk" | "contrast-color" => (cap::COLOR, "颜色"),
        "url" | "src" | "image" | "image-set" | "cross-fade" | "element" | "paint" => {
            (cap::URL, "图像或资源引用")
        }
        // `<image>`：`linear-gradient` / `radial-gradient` / `conic-gradient`
        // 与它们的 `repeating-` 、厂商前缀变体
        n if n.ends_with("-gradient") => (cap::URL, "渐变图像"),
        // `calc()` 一族的量纲取决于操作数，宏侧不做求值——只在属性**完全不接受
        // 任何数值类能力**时才判错（例：`align-items: calc(1px + 1px)`）
        "calc" | "min" | "max" | "clamp" | "round" | "mod" | "rem" | "abs" | "sign" => {
            (cap::ANY_NUMERIC, "数值")
        }
        // `var()` / `env()` / `attr()` 的结果在编译期不可知——永远放行。
        // 写在这里而不是靠 `_ =>` 兜底，是为了让「这是有意放行」这件事留在代码里
        "var" | "env" | "attr" => return None,
        // 不认识的函数一律放行：`fit-content()`、`repeat()`、`translateX()`、
        // `cubic-bezier()`、`blur()`、`counter()` …… 它们的产出类型五花八门，
        // 猜错的代价（拒绝合法 CSS）比漏报高得多
        _ => return None,
    })
}

/// 取值是不是 `name(…)` 形态，是则给出小写的函数名。
///
/// 只认整条取值就是一次函数调用的情况——`0 0 4px rgb(0 0 0)` 是多分量，
/// 由 A-3 处理，不走这里。
fn as_function_name(value: &str) -> Option<String> {
    if !value.ends_with(')') {
        return None;
    }
    let open = value.find('(')?;
    let name = &value[..open];
    if name.is_empty() {
        return None;
    }
    // 括号必须在末尾那个 `)` 处闭合，否则 `a(1) b(2)` 会被当成一次调用
    if !closes_at_end(&value[open..]) {
        return None;
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '-') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

/// `(…)` 开头的这一段是否恰好在末尾闭合。
fn closes_at_end(s: &str) -> bool {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            c if Some(c) == quote => quote = None,
            _ if quote.is_some() => {}
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i + ch.len_utf8() == s.len();
                }
            }
            _ => {}
        }
    }
    false
}

/// 把能力位掩码写成人话，用在报错的 `注：` 一行里。
fn describe_caps(caps: u16) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if caps & (cap::LENGTH | cap::LEN_CALC) != 0 {
        parts.push("长度");
    }
    if caps & cap::PERCENT != 0 {
        parts.push("百分比");
    }
    if caps & (cap::NUM | cap::INT) != 0 {
        parts.push("数字");
    }
    if caps & cap::ANGLE != 0 {
        parts.push("角度");
    }
    if caps & cap::TIME != 0 {
        parts.push("时间");
    }
    if caps & cap::FLEX != 0 {
        parts.push("网格弹性系数（`fr`）");
    }
    if caps & cap::COLOR != 0 {
        parts.push("颜色");
    }
    if caps & cap::URL != 0 {
        parts.push("图像或资源引用");
    }
    if parts.is_empty() {
        return "关键字".to_string();
    }
    format!("关键字、{}", parts.join("、"))
}

// ==========================================
// A-3　多分量
// ==========================================

fn arity_message(prop: &str, value: &str, count: usize) -> String {
    format!(
        "CSS 属性 `{prop}` 只接受单个取值，这里写了 {count} 个：`{value}`\n\
         注：分量之间的空格在 CSS 里是有意义的；`{prop}` 的语法里没有多分量形式，\
         浏览器会整条丢弃这条声明。\n\
         确实需要原样透传时请放进 `unsafe {{ … }}` 块里。"
    )
}

/// 把一条取值按**顶层空白**切开：括号内、引号内的空白不算分界。
///
/// 与 `silex_css::escape::declaration_value` 的括号/引号状态机是同一套判据。
/// 两边各写一份是因为 proc-macro crate 不能依赖运行时 crate——`layers.rs` 的
/// 层名常量也是这么处理的。
///
/// 逗号**不算**分界：`repeat(2, 1fr)` 的逗号在括号里，而顶层逗号只出现在
/// `<x>#` 这类语法里，那些属性本来就带 `MULTI` 位。少切一刀是漏报，切错一刀
/// 是误报，这里选漏报。
pub(crate) fn top_level_segments(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    let mut escaped = false;
    let mut start: Option<usize> = None;

    for (i, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        let boundary = match ch {
            '\\' => {
                escaped = true;
                false
            }
            c if Some(c) == quote => {
                quote = None;
                false
            }
            _ if quote.is_some() => false,
            '"' | '\'' => {
                quote = Some(ch);
                false
            }
            '(' | '[' => {
                depth += 1;
                false
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                false
            }
            c if c.is_whitespace() && depth == 0 => true,
            _ => false,
        };

        match (boundary, start) {
            (true, Some(s)) => {
                out.push(&value[s..i]);
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(&value[s..]);
    }
    out
}

/// 剥掉尾部的 `!important`。
///
/// 它是优先级标记而不是取值分量，不剥掉的话 `color: red !important` 会被
/// A-3 判成「两个分量」——这是最常见的写法之一，误报代价极高。
fn strip_important(value: &str) -> &str {
    let lower = value.to_ascii_lowercase();
    if let Some(pos) = lower.rfind('!')
        && lower[pos + 1..].trim() == "important"
    {
        return value[..pos].trim_end();
    }
    value
}

// ==========================================
// 表查找与报错分派
// ==========================================

fn caps_of(prop: &str) -> Option<u16> {
    PROPERTY_CAPS
        .binary_search_by_key(&prop, |(name, _)| name)
        .ok()
        .map(|i| PROPERTY_CAPS[i].1)
}

fn keywords_of(prop: &str) -> Option<&'static [&'static str]> {
    PROPERTY_KEYWORDS
        .binary_search_by_key(&prop, |(name, _)| name)
        .ok()
        .map(|i| PROPERTY_KEYWORDS[i].1)
}

/// 按 `[css.validation]` 配的级别分派：报错、降级成警告，或者什么都不做。
fn report(layer: Layer, span: Span, warnings: &mut Vec<CssWarning>, message: String) -> Result<()> {
    match layer.level() {
        ValidationLevel::Error => Err(syn::Error::new(span, message)),
        ValidationLevel::Warn => {
            warnings.push(CssWarning {
                message: format!("[Silex CSS] {message}"),
                span,
            });
            Ok(())
        }
        ValidationLevel::Off => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(prop: &str, value: &str) -> std::result::Result<(), String> {
        let mut warnings = Vec::new();
        check_static_value(prop, value, Span::call_site(), &mut warnings).map_err(|e| e.to_string())
    }

    fn err(prop: &str, value: &str) -> String {
        check(prop, value).expect_err(&format!("`{prop}: {value}` 应当被拒绝"))
    }

    fn ok(prop: &str, value: &str) {
        if let Err(e) = check(prop, value) {
            panic!("`{prop}: {value}` 应当放行，却报了：{e}");
        }
    }

    // --- 生成的两张表本身 ---

    #[test]
    fn the_generated_tables_are_sorted() {
        assert!(
            PROPERTY_CAPS.windows(2).all(|w| w[0].0 < w[1].0),
            "PROPERTY_CAPS 必须升序，否则二分查找会漏"
        );
        assert!(PROPERTY_KEYWORDS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(COLOR_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(UNIVERSAL_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        for (name, list) in PROPERTY_KEYWORDS {
            assert!(!list.is_empty(), "{name} 的关键字表是空的，不该被收录");
            assert!(
                list.windows(2).all(|w| w[0] < w[1]),
                "{name} 的关键字表没排序"
            );
        }
    }

    /// 大小写：`currentColor` 与 `Canvas` 在 MDN 数据里是驼峰的，
    /// 表里必须已经小写，否则对小写取值二分就会漏掉它们
    #[test]
    fn the_color_table_is_lowercase() {
        assert!(COLOR_KEYWORDS.contains(&"currentcolor"), "缺 currentcolor");
        assert!(COLOR_KEYWORDS.contains(&"canvas"), "缺 canvas");
        assert!(
            COLOR_KEYWORDS
                .iter()
                .all(|k| !k.chars().any(|c| c.is_uppercase())),
            "颜色表里还有大写"
        );
    }

    // --- A-1　裸关键字 ---

    #[test]
    fn a_misspelled_keyword_is_rejected_with_a_suggestion() {
        let e = err("align-items", "centre");
        assert!(e.contains("`centre` 不是"), "{e}");
        assert!(e.contains("`center`"), "{e}");
    }

    #[test]
    fn a_keyword_from_another_property_is_rejected() {
        assert!(err("z-index", "red").contains("`red` 不是"));
        assert!(err("display", "blok").contains("`display`"));
        assert!(err("position", "sticy").contains("`sticky`"));
    }

    /// 具名颜色不在任何属性的关键字表里，靠 `COLOR` 位 + 全局颜色表放行
    #[test]
    fn named_colors_pass_on_color_capable_properties() {
        ok("color", "red");
        ok("color", "currentColor");
        ok("background-color", "transparent");
        ok("caret-color", "rebeccapurple");
        // `border` 的关键字表里只有线宽与线型，颜色得靠 `COLOR` 位
        ok("border", "red");
        ok("border", "solid");
    }

    /// 但接受颜色的属性上，拼错的颜色名照样要被拦住
    #[test]
    fn a_misspelled_color_is_still_rejected() {
        let e = err("color", "reed");
        assert!(e.contains("`red`"), "{e}");
    }

    #[test]
    fn universal_keywords_pass_everywhere() {
        for kw in ["inherit", "initial", "unset", "revert", "revert-layer"] {
            ok("z-index", kw);
            ok("align-items", kw);
            ok("color", kw);
        }
    }

    /// `<custom-ident>` 一族的属性必须整条放行——它们的关键字表只有
    /// `none` / `auto`，照表判错会把最常见的写法全拒掉
    #[test]
    fn open_properties_accept_arbitrary_identifiers() {
        ok("animation-name", "fadeIn");
        ok("font-family", "Inter");
        ok("grid-area", "header");
        ok("will-change", "transform");
        ok("view-transition-name", "card");
        ok("transition-property", "opacity");
    }

    /// 厂商关键字 MDN 没有收录，一律放行
    #[test]
    fn vendor_prefixed_keywords_pass() {
        ok("display", "-webkit-box");
        ok("-webkit-appearance", "none");
        ok("-webkit-font-smoothing", "antialiased");
    }

    /// 自定义变量没有语法数据
    #[test]
    fn custom_properties_are_never_checked() {
        ok("--brand", "whatever-you-like");
        ok("--brand", "1px solid red");
        ok("--brand", "rgb(0 0 0)");
    }

    // --- A-2　函数式取值 ---

    #[test]
    fn a_color_function_on_a_keyword_property_is_rejected() {
        let e = err("align-items", "rgb(0 0 0)");
        assert!(e.contains("`rgb()`"), "{e}");
        assert!(e.contains("颜色"), "{e}");
    }

    #[test]
    fn color_functions_pass_on_color_properties() {
        ok("color", "rgb(0 0 0)");
        ok("color", "rgba(0, 0, 0, 0.5)");
        ok("color", "oklch(0.7 0.1 200)");
        ok("color", "color-mix(in srgb, red, blue)");
        ok("background-color", "hsl(200 50% 50%)");
    }

    #[test]
    fn gradients_pass_on_image_properties_and_fail_elsewhere() {
        ok("background-image", "linear-gradient(red, blue)");
        ok("background-image", "repeating-conic-gradient(red, blue)");
        assert!(err("z-index", "linear-gradient(red, blue)").contains("渐变图像"));
    }

    /// `calc()` 一族只在属性完全不接受数值时才判错
    #[test]
    fn calc_is_only_rejected_on_properties_with_no_numeric_capability() {
        ok("width", "calc(100% - 10px)");
        ok("z-index", "calc(1 + 2)");
        ok("opacity", "clamp(0, 0.5, 1)");
        ok("rotate", "min(45deg, 1turn)");
        assert!(err("align-items", "calc(1px + 1px)").contains("数值"));
    }

    /// `var()` / `env()` / `attr()` 的结果在编译期不可知
    #[test]
    fn indirection_functions_always_pass() {
        ok("align-items", "var(--x)");
        ok("z-index", "env(safe-area-inset-top)");
        ok("color", "var(--brand, red)");
    }

    /// 不认识的函数一律放行——猜错的代价比漏报高得多
    #[test]
    fn unknown_functions_pass() {
        ok("width", "fit-content(50%)");
        ok("grid-template-columns", "repeat(2, minmax(0, 1fr))");
        ok("transform", "translateX(10px)");
        ok("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)");
        ok("filter", "blur(4px)");
        ok("width", "calc-size(auto, size)");
    }

    // --- A-3　多分量 ---

    #[test]
    fn a_multi_component_value_on_a_single_value_property_is_rejected() {
        let e = err("color", "1px solid red");
        assert!(e.contains("只接受单个取值"), "{e}");
        assert!(err("z-index", "1 2").contains("2 个"));
    }

    #[test]
    fn real_shorthands_accept_multiple_components() {
        ok("border", "1px solid red");
        ok("margin", "0 auto");
        ok("padding", "4px 8px 12px 16px");
        ok("box-shadow", "0 1px 2px rgba(0, 0, 0, 0.2)");
        ok("font", "italic bold 12px/1.5 serif");
        ok("background", "url(a.png) no-repeat center / cover");
        ok("grid-template-columns", "repeat(2, 1fr) 100px");
        ok("transition", "color 0.3s ease-in-out");
        // `<overflow-position>? <self-position>`：`safe center` 是合法取值
        ok("align-items", "safe center");
    }

    /// `!important` 是优先级标记而不是取值分量
    #[test]
    fn important_is_not_counted_as_a_component() {
        ok("color", "red !important");
        ok("z-index", "10 !important");
        ok("align-items", "center!important");
    }

    /// 引号与括号里的空白不是分界
    #[test]
    fn whitespace_inside_quotes_and_parens_is_not_a_boundary() {
        assert_eq!(top_level_segments("rgb(0 0 0)"), vec!["rgb(0 0 0)"]);
        assert_eq!(
            top_level_segments("1px  solid   red"),
            vec!["1px", "solid", "red"]
        );
        assert_eq!(
            top_level_segments(r#""a b" "c d""#),
            vec![r#""a b""#, r#""c d""#]
        );
        assert_eq!(top_level_segments("url(a b.png)"), vec!["url(a b.png)"]);
        assert_eq!(top_level_segments(""), Vec::<&str>::new());
    }

    #[test]
    fn a_function_call_is_only_recognised_when_it_is_the_whole_value() {
        assert_eq!(as_function_name("rgb(0 0 0)").as_deref(), Some("rgb"));
        assert_eq!(
            as_function_name("-webkit-gradient(x)").as_deref(),
            Some("-webkit-gradient")
        );
        // 两次调用并列 → 不是「整条取值就是一次调用」
        assert_eq!(as_function_name("rgb(0 0 0) rgb(1 1 1)"), None);
        assert_eq!(as_function_name("red"), None);
        assert_eq!(as_function_name("(0 0)"), None);
    }

    // --- 常见写法的回归闸门 ---

    /// 一批日常写法，一个都不能被新判据拒掉。
    /// 这是「不误报」的实证——比任何论证都可靠
    #[test]
    fn everyday_declarations_are_never_rejected() {
        let cases = [
            ("display", "flex"),
            ("display", "grid"),
            ("display", "inline-block"),
            ("position", "absolute"),
            ("overflow", "hidden"),
            ("overflow", "auto"),
            ("cursor", "pointer"),
            ("text-align", "center"),
            ("align-items", "center"),
            ("justify-content", "space-between"),
            ("flex-direction", "column"),
            ("flex-wrap", "wrap"),
            ("box-sizing", "border-box"),
            ("white-space", "nowrap"),
            ("text-overflow", "ellipsis"),
            ("word-break", "break-all"),
            ("overflow-wrap", "break-word"),
            ("font-weight", "bold"),
            ("font-style", "italic"),
            ("text-transform", "uppercase"),
            ("text-decoration", "underline"),
            ("line-height", "normal"),
            ("vertical-align", "middle"),
            ("visibility", "hidden"),
            ("pointer-events", "none"),
            ("user-select", "none"),
            ("resize", "none"),
            ("appearance", "none"),
            ("list-style", "none"),
            ("outline", "none"),
            ("border", "none"),
            ("background", "none"),
            ("object-fit", "cover"),
            ("background-repeat", "no-repeat"),
            ("background-size", "cover"),
            ("background-position", "center"),
            ("border-collapse", "collapse"),
            ("table-layout", "fixed"),
            ("scroll-behavior", "smooth"),
            ("mix-blend-mode", "multiply"),
            ("isolation", "isolate"),
            ("touch-action", "manipulation"),
            ("float", "left"),
            ("clear", "both"),
            ("flex", "none"),
            ("flex", "1"),
            ("margin", "auto"),
            ("width", "auto"),
            ("width", "fit-content"),
            ("height", "100%"),
            ("max-width", "none"),
            ("grid-auto-flow", "row"),
            ("place-items", "center"),
            ("place-content", "center"),
            ("backdrop-filter", "none"),
            ("border-style", "solid"),
            ("border-width", "thin"),
            ("content", "\"\""),
            ("all", "unset"),
        ];
        for (prop, value) in cases {
            ok(prop, value);
        }
    }
}
