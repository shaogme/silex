//! 对拍测试（differential testing）参考数据的代码生成。
//!
//! 数据来自 `scripts/export_tailwind/reference_css.js`——真实 `tailwindcss` 包自己编译出来的
//! CSS，theme 变量已展开、`calc()` 已求值、oklch 已转 hex。它是**独立于本项目 resolver** 的
//! 语义真值，因此能发现生成器自身的错误（分析报告 §6.2 指出 `table_examples.rs` 与 `table.rs`
//! 同源，属于自我参照，无法承担这个职责）。

use std::{collections::BTreeMap, fmt::Write};

/// `reference_css.json` 的结构：类名 → 声明序列
pub type ReferenceCssJson = BTreeMap<String, Vec<(String, String)>>;

/// 生成 `silex_macros/src/css/tw/resolver/codegen/reference_css.rs`
pub fn generate_reference_css_code(reference: &ReferenceCssJson) -> String {
    let mut code = String::with_capacity(256 * 1024);
    code.push_str("// 自动生成的 Tailwind 对拍参考数据（供 silex_macros 的差分测试使用）\n");
    code.push_str("// 由 silex_codegen 从 data/tailwind/reference_css.json 生成，切勿手写修改！\n");
    code.push_str("//\n");
    code.push_str(
        "// 每一项是 `(类名, &[(CSS 属性, CSS 值)])`——真实 tailwindcss 对该类名的编译结果，\n",
    );
    code.push_str("// theme 变量已展开、calc() 已求值、oklch 已转 hex。\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static REFERENCE_CSS: &[(&str, &[(&str, &str)])] = &[\n");
    for (class, decls) in reference {
        let _ = write!(code, "    ({}, &[", quote_rust_str(class));
        for (i, (prop, value)) in decls.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(
                code,
                "({}, {})",
                quote_rust_str(prop),
                quote_rust_str(value)
            );
        }
        code.push_str("]),\n");
    }
    code.push_str("];\n");

    code
}

/// 用 Rust 字符串字面量语法转义——参考值里含有 `"`（如 `content: \"\"`）与 `\`
fn quote_rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
