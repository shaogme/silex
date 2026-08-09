//! 动态规则的结构化模板。
//!
//! 运行时规则里有两处需要在每次求值时填进去的东西：组件那一轮的动态类名，和
//! 各个响应式取值。此前两者都是**事后文本替换**：
//!
//! - `resolved_rule.replace(".slx-st-x", ".slx-st-x-dyn-h")`——规则里若同时存在
//!   `.foo` 与 `.foo-bar`，后者会被改成 `.foo-dyn-h-bar`；
//! - 按顺序逐个 `String::replace(pattern, value)`——前一个替换写进去的**值内容**
//!   里若含有后一个 pattern，会被二次替换；
//! - `res.find("{}")` 每轮从头搜索，且与 CSS 内容里真实出现的 `{}` 冲突。
//!
//! 现在模板在编译期就被切成片段，运行时只做拼接：没有模式匹配，也就没有误伤。

use crate::escape::{declaration_value, selector_fragment};
use silex_hash::{
    css::{Normalized, encode_base36},
    css_hasher,
};
use std::hash::{Hash, Hasher};

/// 模板的一个片段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssPart {
    /// 原样输出的字面片段
    Lit(&'static str),
    /// 本轮的动态类名（不含前导 `.`，`.` 留在字面片段里）
    Class,
    /// 第 n 个运行时声明值
    Val(usize),
    /// 第 n 个运行时选择器片段。
    ///
    /// 与声明值分开建模，避免把动态值中的逗号、空白或伪类语法带出原选择器。
    SelectorVal(usize),
    /// 第 n 个构造阶段静态声明值。
    StaticVal(usize),
}

/// 把模板拼成一条完整的 CSS 规则。
pub fn render(parts: &[CssPart], class: &str, vals: &[String]) -> String {
    render_with(parts, class, vals, &[], false)
}

/// 把模板拼成一条完整的 CSS 规则，并填入构造阶段静态值。
pub fn render_with_static(
    parts: &[CssPart],
    class: &str,
    vals: &[String],
    static_vals: &[String],
) -> String {
    render_with(parts, class, vals, static_vals, false)
}

/// 把模板拼成一条以动态选择器片段为主的 CSS 规则。
///
/// `inject_managed_dynamic_style` 的 positional getter 历史上使用 `Val` 表示选择器
/// 片段。该入口保留这一形状，新的调用点可使用 `SelectorVal` 获得同样的显式语义。
pub fn render_selector(parts: &[CssPart], class: &str, vals: &[String]) -> String {
    render_with(parts, class, vals, &[], true)
}

/// 选择器模板的静态值版本。静态插值仍然只能出现在声明位置，因此使用声明值
/// 转义；selector positional getter 继续使用选择器片段转义。
pub fn render_selector_with_static(
    parts: &[CssPart],
    class: &str,
    vals: &[String],
    static_vals: &[String],
) -> String {
    render_with(parts, class, vals, static_vals, true)
}

fn render_with(
    parts: &[CssPart],
    class: &str,
    vals: &[String],
    static_vals: &[String],
    positional_values_are_selectors: bool,
) -> String {
    let capacity = parts
        .iter()
        .map(|p| match p {
            CssPart::Lit(s) => s.len(),
            CssPart::Class => class.len(),
            CssPart::Val(i) | CssPart::SelectorVal(i) => vals.get(*i).map_or(0, String::len),
            CssPart::StaticVal(i) => static_vals.get(*i).map_or(0, String::len),
        })
        .sum();
    let mut out = String::with_capacity(capacity);
    for part in parts {
        match part {
            CssPart::Lit(s) => out.push_str(s),
            CssPart::Class => out.push_str(class),
            CssPart::Val(i) => {
                if let Some(v) = vals.get(*i) {
                    if positional_values_are_selectors {
                        out.push_str(&selector_fragment(v));
                    } else {
                        out.push_str(&declaration_value(v));
                    }
                }
            }
            CssPart::SelectorVal(i) => {
                if let Some(v) = vals.get(*i) {
                    out.push_str(&selector_fragment(v));
                }
            }
            CssPart::StaticVal(i) => {
                if let Some(v) = static_vals.get(*i) {
                    out.push_str(&declaration_value(v));
                }
            }
        }
    }
    out
}

/// 本轮的动态类名。
///
/// 由「模板结构 + 本轮取值」决定，**不含类名自身**——类名要靠它算出来，
/// 拿它参与哈希就成了循环依赖（此前是先用基类名渲染一遍、哈希、再把基类名
/// 文本替换成动态类名，绕的正是这个圈）。
pub fn dynamic_class(base: &str, parts: &[CssPart], vals: &[String]) -> String {
    dynamic_class_with_static(base, parts, vals, &[])
}

/// 计算带静态值的动态规则类名。
pub fn dynamic_class_with_static(
    base: &str,
    parts: &[CssPart],
    vals: &[String],
    static_vals: &[String],
) -> String {
    let mut hasher = css_hasher!();
    b"silex-dyn-v4".hash(&mut hasher);
    for part in parts {
        match part {
            CssPart::Lit(s) => {
                0u8.hash(&mut hasher);
                Normalized(s).hash(&mut hasher);
            }
            CssPart::Class => 1u8.hash(&mut hasher),
            CssPart::Val(i) => {
                2u8.hash(&mut hasher);
                i.hash(&mut hasher);
            }
            CssPart::SelectorVal(i) => {
                4u8.hash(&mut hasher);
                i.hash(&mut hasher);
            }
            CssPart::StaticVal(i) => {
                5u8.hash(&mut hasher);
                i.hash(&mut hasher);
            }
        }
    }
    for v in vals {
        3u8.hash(&mut hasher);
        Normalized(v).hash(&mut hasher);
    }
    for v in static_vals {
        5u8.hash(&mut hasher);
        Normalized(v).hash(&mut hasher);
    }
    let mut buf = [0u8; 13];
    format!("{}-d{}", base, encode_base36(hasher.finish(), &mut buf))
}

/// 渲染经过 lightningcss 压缩后的静态 CSS 模板。
///
/// 静态表达式先以 `var(--slx-static-N)` 作为可被 CSS parser 接受的占位符进入
/// 模板，真正输出时只扫描一次并对每个值执行声明值转义。替换结果不会再次被
/// 当作占位符扫描，因此值内容里的 `var(--slx-static-1)` 也不会发生二次替换。
pub fn render_static_template(template: &str, values: &[String]) -> String {
    let pairs: Vec<(String, String)> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                format!("var(--slx-static-{index})"),
                declaration_value(value).into_owned(),
            )
        })
        .collect();
    replace_placeholders(template, &pairs)
}

/// 一遍扫描完成多组「模式 → 取值」替换。
///
/// 用于全局样式里的 `var(--slx-dyn-N)` 占位符——那段模板要先过一遍
/// lightningcss（解析 + 压缩 + 打印），位置信息在那之后就不存在了，只能按文本
/// 找。但**替换写进去的内容不再参与后续匹配**，这就堵住了二次替换：某个取值
/// 里恰好含有 `var(--slx-dyn-1)` 时不会被当成占位符再替一次。
pub fn replace_placeholders(src: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    // `i` 始终落在字符边界上：要么按整个 pattern 前进（`starts_with` 命中即对齐），
    // 要么按一个完整字符前进
    'outer: while let Some(ch) = src[i..].chars().next() {
        for (pattern, value) in pairs {
            if !pattern.is_empty() && src[i..].starts_with(pattern.as_str()) {
                out.push_str(value);
                i += pattern.len();
                continue 'outer;
            }
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 报告 P2-8：`.foo` 与 `.foo-bar` 同时出现时，文本替换会把后者也改掉
    #[test]
    fn a_longer_class_that_starts_with_the_base_class_is_untouched() {
        let parts = [
            CssPart::Lit("."),
            CssPart::Class,
            CssPart::Lit(" .foo-bar{color:"),
            CssPart::Val(0),
            CssPart::Lit("}"),
        ];
        let out = render(&parts, "foo-d1", &["red".to_string()]);
        assert_eq!(out, ".foo-d1 .foo-bar{color:red}");
    }

    /// 取值里出现 `{}` 不会被当成占位符
    #[test]
    fn values_are_never_rescanned_for_placeholders() {
        let parts = [CssPart::Lit("content:"), CssPart::Val(0), CssPart::Lit(";")];
        let out = render(&parts, "x", &["\"{} var(--slx-dyn-1)\"".to_string()]);
        assert_eq!(out, "content:\"{} var(--slx-dyn-1)\";");
    }

    /// 类名由模板结构与取值决定，与基类名一起构成完整类名
    #[test]
    fn the_dynamic_class_changes_with_the_values_and_nothing_else() {
        let parts = [CssPart::Lit("color:"), CssPart::Val(0)];
        let a = dynamic_class("base", &parts, &["red".to_string()]);
        let b = dynamic_class("base", &parts, &["red".to_string()]);
        let c = dynamic_class("base", &parts, &["blue".to_string()]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("base-d"));
    }

    /// 报告 P2-8：顺序替换时，前一个替换写入的值会被后一个 pattern 二次命中
    #[test]
    fn replacement_output_is_not_rescanned() {
        let pairs = [
            (
                "var(--slx-dyn-0)".to_string(),
                "var(--slx-dyn-1)".to_string(),
            ),
            ("var(--slx-dyn-1)".to_string(), "red".to_string()),
        ];
        let out = replace_placeholders("a:var(--slx-dyn-0);b:var(--slx-dyn-1)", &pairs);
        assert_eq!(out, "a:var(--slx-dyn-1);b:red");
    }

    #[test]
    fn replacement_keeps_multibyte_content_intact() {
        let pairs = [("{X}".to_string(), "值".to_string())];
        assert_eq!(replace_placeholders("前{X}后", &pairs), "前值后");
    }

    #[test]
    fn a_dynamic_selector_fragment_cannot_open_a_new_rule() {
        let parts = [
            CssPart::Lit(".base "),
            CssPart::SelectorVal(0),
            CssPart::Lit("{color:red}"),
        ];
        let out = render(&parts, "base", &["x} body { display: none".to_string()]);
        assert!(!out.contains("body { display"), "{out}");
        assert_eq!(out.matches('}').count(), 1, "{out}");
    }

    #[test]
    fn selector_values_cannot_widen_a_rule_to_another_selector() {
        let parts = [
            CssPart::Lit(".base "),
            CssPart::SelectorVal(0),
            CssPart::Lit("{color:red}"),
        ];
        let out = render(&parts, "base", &[", body:hover".to_string()]);
        assert!(!out.contains(','), "{out}");
        assert!(!out.contains(":hover"), "{out}");
    }

    #[test]
    fn static_values_are_rendered_in_declaration_context() {
        let parts = [
            CssPart::Lit(".base{color:"),
            CssPart::StaticVal(0),
            CssPart::Lit("}"),
        ];
        let out = render_with_static(
            &parts,
            "base",
            &[],
            &["red; } body { display: none".to_string()],
        );
        assert!(!out.contains("body { display"), "{out}");
        assert!(!out.contains(';') || out.ends_with('}'), "{out}");
    }

    #[test]
    fn static_template_replacement_is_not_rescanned() {
        let out = render_static_template(
            "a:var(--slx-static-0);b:var(--slx-static-1)",
            &["var(--slx-static-1)".to_string(), "red".to_string()],
        );
        assert_eq!(out, "a:var(--slx-static-1);b:red");
    }
}
