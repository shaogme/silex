use super::types::ProcessedProp;
use heck::AsPascalCase;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn generate_properties_macro(props: &[ProcessedProp]) -> String {
    let mut code = String::new();
    code.push_str("/// 自动生成的 CSS 属性注册表\n");
    code.push_str("///\n");
    code.push_str("/// 每项的第四栏是该属性允许的值类型能力集合，由 MDN 值定义语法解析得出\n");
    code.push_str("/// （见 `silex_codegen::css::syntax`），而不是过去那 8 个粗粒度分组。\n");
    code.push_str("#[macro_export]\n");
    code.push_str("macro_rules! for_all_properties {\n");
    code.push_str("    ($callback:ident) => {\n");
    code.push_str("        $callback! {\n");

    let items: Vec<String> = props
        .iter()
        .map(|prop| {
            let caps: Vec<&str> = prop.caps.iter().map(|c| c.as_str()).collect();
            format!(
                "            ({}, \"{}\", {}, [{}])",
                prop.method_name,
                prop.name,
                prop.struct_name,
                caps.join(" ")
            )
        })
        .collect();

    code.push_str(&items.join(",\n"));
    code.push_str("\n        }\n");
    code.push_str("    };\n");
    code.push_str("}\n");
    code
}

/// 生成过程宏侧的属性名白名单。
///
/// `css!` / `styled!` 里的静态声明此前完全不经过校验：`colr: red` 会原样
/// 输出成 `colr:red`，编译通过、无警告、浏览器丢弃。宏侧要挡住这一类拼写
/// 错误，就得知道哪些属性名是存在的。
pub fn generate_property_names_code(props: &[ProcessedProp]) -> String {
    let mut code = String::new();
    code.push_str("//! 自动生成：CSS 标准属性名白名单（已排序，可二分查找）。\n");
    code.push_str("//!\n");
    code.push_str("//! 由 `silex_codegen` 从 MDN 数据生成，与 `silex_css` 的\n");
    code.push_str("//! `for_all_properties!` 注册表同源。\n\n");
    code.push_str("/// 所有已注册的 CSS 属性名（kebab-case，升序）。\n");
    code.push_str("pub static CSS_PROPERTY_NAMES: &[&str] = &[\n");

    let mut names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    for name in names {
        let _ = writeln!(code, "    \"{}\",", name);
    }
    code.push_str("];\n");
    code
}

pub fn generate_keywords_code(props: &[ProcessedProp], color_keywords: &[String]) -> String {
    let mut code = String::new();
    code.push_str("// 自动生成的 CSS 关键字 Enums\n");
    code.push_str("//\n");
    code.push_str("// 关键字集合完全相同的属性共用同一个枚举，其余属性以类型别名指向它。\n");
    code.push_str("// 此前是每个属性一个独立枚举（361 个 enum / 7 520 个 variant，实际只有\n");
    code.push_str("// 194 种不同的关键字集合），且 `AlignItemsKeyword::Center` 与\n");
    code.push_str("// `JustifyContentKeyword::Center` 是两个互不相干的类型。\n\n");
    code.push_str("use crate::define_css_enum;\n");
    code.push_str("use crate::types::{Auto, NoneValue, ValidFor, props};\n");
    code.push_str("use std::fmt::{Display, Formatter, Result};\n\n");

    let mut keyword_types: Vec<String> = Vec::new();

    // --- 全局具名颜色表 ---
    code.push_str("// 全局具名颜色关键字。\n");
    code.push_str("// 所有接受 `<color>` 的属性共用这一份，不再每个属性复制一遍\n");
    code.push_str("//（此前有 18 个枚举是同一份 31 项系统颜色表的逐字拷贝）。\n");
    code.push_str("define_css_enum!(ColorKeyword (props::Color) {\n");
    let mut color_variants = VariantNamer::default();
    for kw in color_keywords {
        let _ = writeln!(code, "    {} => \"{}\",", color_variants.name_for(kw), kw);
    }
    code.push_str("});\n\n");
    keyword_types.push("ColorKeyword".to_string());

    // --- 按关键字集合去重 ---
    // BTreeMap 保证输出顺序稳定；键就是关键字集合本身
    let mut groups: BTreeMap<Vec<String>, Vec<&ProcessedProp>> = BTreeMap::new();
    for prop in props {
        if prop.keywords.is_empty() {
            continue;
        }
        groups.entry(prop.keywords.clone()).or_default().push(prop);
    }

    let mut aliases: Vec<String> = Vec::new();
    let mut trait_impls = String::new();

    for (keywords, members) in &groups {
        // `auto` / `none` 单独一项的属性直接复用已有的 `Auto` / `NoneValue`，
        // 不再生成 32 个「唯一变体是 auto」和 27 个「唯一变体是 none」的枚举
        let is_bare_auto = keywords.len() == 1 && keywords[0] == "auto";
        let is_bare_none = keywords.len() == 1 && keywords[0] == "none";

        if !is_bare_auto && !is_bare_none {
            let canonical = format!("{}Keyword", members[0].struct_name);
            keyword_types.push(canonical.clone());

            let props_list: Vec<String> = members
                .iter()
                .map(|m| format!("props::{}", m.struct_name))
                .collect();
            let _ = writeln!(
                code,
                "define_css_enum!({} ({}) {{",
                canonical,
                props_list.join(", ")
            );
            let mut namer = VariantNamer::default();
            for kw in keywords {
                let _ = writeln!(code, "    {} => \"{}\",", namer.name_for(kw), kw);
            }
            code.push_str("});\n\n");

            for m in members.iter().skip(1) {
                aliases.push(format!(
                    "pub type {}Keyword = {};\n",
                    m.struct_name, canonical
                ));
            }
        }

        // 全局 `Auto` / `NoneValue` 对每个把它们列为合法关键字的属性都有效
        for m in members {
            if keywords.iter().any(|k| k == "auto") {
                let _ = writeln!(
                    trait_impls,
                    "impl ValidFor<props::{}> for Auto {{}}",
                    m.struct_name
                );
            }
            if keywords.iter().any(|k| k == "none") {
                let _ = writeln!(
                    trait_impls,
                    "impl ValidFor<props::{}> for NoneValue {{}}",
                    m.struct_name
                );
            }
        }
    }

    if !aliases.is_empty() {
        code.push_str("// --- 关键字集合相同的属性共用同一个枚举 ---\n");
        for a in &aliases {
            code.push_str(a);
        }
        code.push('\n');
    }

    code.push_str("// --- 全局 `auto` / `none` ---\n");
    code.push_str(&trait_impls);
    code.push('\n');

    // Generate a helper macro to implement traits for all keywords
    code.push_str("#[macro_export]\n");
    code.push_str("macro_rules! register_generated_keywords {\n");
    code.push_str("    ($callback:ident) => {\n");
    code.push_str("        $callback! {\n");

    // 只登记规范类型：别名指向同一个类型，重复登记会产生冲突 impl
    keyword_types.sort();
    keyword_types.dedup();

    for (i, kt) in keyword_types.iter().enumerate() {
        if i == keyword_types.len() - 1 {
            let _ = write!(code, "            {}", kt);
        } else {
            let _ = writeln!(code, "            {},", kt);
        }
    }
    code.push_str("\n        }\n");
    code.push_str("    };\n");
    code.push_str("}\n");

    code
}

/// 变体命名：处理 Rust 关键字、数字开头，以及同一个枚举里的重名。
#[derive(Default)]
struct VariantNamer {
    seen: std::collections::HashSet<String>,
}

impl VariantNamer {
    fn name_for(&mut self, kw: &str) -> String {
        let mut variant = AsPascalCase(kw).to_string();

        // 1. If starts with digit, prepend underscore
        if variant.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            variant = format!("_{}", variant);
        }

        // 2. Handle Rust keywords and common conflicts
        if is_reserved_word(&variant) {
            variant = format!("{}_", variant);
        }

        // 3. `AccentColor` 与 `accent-color` 会撞成同一个变体名，加序号区分
        //    而不是静默丢掉其中一个
        if self.seen.contains(&variant) {
            let mut n = 2;
            while self.seen.contains(&format!("{}{}", variant, n)) {
                n += 1;
            }
            variant = format!("{}{}", variant, n);
        }
        self.seen.insert(variant.clone());
        variant
    }
}

fn is_reserved_word(s: &str) -> bool {
    matches!(
        s,
        "Self"
            | "Self_"
            | "Super"
            | "Move"
            | "Continue"
            | "Break"
            | "Default"
            | "Loop"
            | "Match"
            | "If"
            | "Else"
            | "While"
            | "For"
            | "In"
            | "Let"
            | "Const"
            | "Static"
            | "Mut"
            | "Pub"
            | "Crate"
            | "Mod"
            | "Struct"
            | "Enum"
            | "Trait"
            | "Type"
            | "As"
            | "Async"
            | "Await"
            | "Fn"
            | "Dyn"
            | "Impl"
            | "Where"
            | "Unsafe"
            | "Extern"
            | "Ref"
            | "Use"
            | "Try"
            | "Yield"
    )
}
