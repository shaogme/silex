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
    /// 第 n 个运行时取值
    Val(usize),
}

/// 把模板拼成一条完整的 CSS 规则。
pub fn render(parts: &[CssPart], class: &str, vals: &[String]) -> String {
    let capacity = parts
        .iter()
        .map(|p| match p {
            CssPart::Lit(s) => s.len(),
            CssPart::Class => class.len(),
            CssPart::Val(i) => vals.get(*i).map_or(0, String::len),
        })
        .sum();
    let mut out = String::with_capacity(capacity);
    for part in parts {
        match part {
            CssPart::Lit(s) => out.push_str(s),
            CssPart::Class => out.push_str(class),
            CssPart::Val(i) => {
                if let Some(v) = vals.get(*i) {
                    out.push_str(v);
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
        }
    }
    for v in vals {
        3u8.hash(&mut hasher);
        Normalized(v).hash(&mut hasher);
    }
    let mut buf = [0u8; 13];
    format!("{}-d{}", base, encode_base36(hasher.finish(), &mut buf))
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
            ("var(--slx-dyn-0)".to_string(), "var(--slx-dyn-1)".to_string()),
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
}
