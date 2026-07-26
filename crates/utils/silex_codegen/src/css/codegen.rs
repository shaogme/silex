use super::types::{ProcessedProp, ValueCap};
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

/// 生成过程宏侧的「属性 → 可单独取的关键字」表。
///
/// 宏里的静态取值此前只有「一眼能定型」的字面量（`10px`、`#fff`）会过类型
/// 系统，裸关键字一律放行：`align-items: centre`、`display: blok`、
/// `z-index: red` 全是编译通过、无警告、浏览器丢弃。
///
/// 数据源与 `silex_css` 的 `keywords_gen.rs` 完全同源（都是
/// `Analysis::keywords`），所以这张表判合法的取值一定能写成对应的关键字枚举，
/// 两边不会打架。
///
/// 只收录关键字表**非空**的属性：查不到就等于没有判据，宏侧一律放行。
pub fn generate_property_keywords_code(
    props: &[ProcessedProp],
    color_keywords: &[String],
) -> String {
    let mut code = String::new();
    code.push_str("//! 自动生成：每个属性可以单独取的字面关键字。\n");
    code.push_str("//!\n");
    code.push_str("//! 由 `silex_codegen` 从 MDN 值定义语法生成，与 `silex_css` 的\n");
    code.push_str("//! `keywords_gen.rs` 同源——都来自 `Analysis::keywords`。\n");
    code.push_str("//!\n");
    code.push_str("//! 「可以**单独**取」是关键：`align-items` 的 `safe` 不在表里，因为\n");
    code.push_str("//! `<overflow-position>` 必须跟着一个 `<self-position>`，`safe` 单独出现\n");
    code.push_str("//! 并不是合法取值。多分量取值由 `property_caps` 的 `MULTI` 位负责。\n\n");

    code.push_str("/// (属性名, 该属性的关键字表)。属性名升序，关键字表内部也升序，\n");
    code.push_str("/// 两层都可二分查找。\n");
    code.push_str("///\n");
    code.push_str("/// 关键字表为空的属性**不收录**：查不到就是没有判据，一律放行。\n");
    code.push_str("///\n");
    code.push_str("/// 全部小写：CSS 的关键字是 ASCII 大小写无关的，查表前把取值一并\n");
    code.push_str("/// 转小写就够了，不必两边各留一份大小写。\n");
    code.push_str("pub static PROPERTY_KEYWORDS: &[(&str, &[&str])] = &[\n");
    for prop in props {
        let keywords = lowercased(&prop.keywords);
        if keywords.is_empty() {
            continue;
        }
        let list: Vec<String> = keywords.iter().map(|k| format!("\"{k}\"")).collect();
        let _ = writeln!(code, "    (\"{}\", &[{}]),", prop.name, list.join(", "));
    }
    code.push_str("];\n\n");

    code.push_str("/// 全局具名颜色关键字（小写、升序）。\n");
    code.push_str("///\n");
    code.push_str("/// 属性侧的语法分析把 `<color>` 当作终点，所以 `red` / `transparent` /\n");
    code.push_str("/// `currentcolor` 不在任何属性的关键字表里。接受 `<color>` 的属性\n");
    code.push_str("/// （`property_caps` 里带 `COLOR` 位）额外拿这张表放行。\n");
    code.push_str("///\n");
    code.push_str("/// 与 `keywords_gen.rs` 的 `ColorKeyword` 同源，但这里统一小写：那边\n");
    code.push_str("/// 要按规范原样输出 `currentColor` / `Canvas`，这边只用来查表。\n");
    code.push_str("pub static COLOR_KEYWORDS: &[&str] = &[\n");
    for kw in lowercased(color_keywords) {
        let _ = writeln!(code, "    \"{kw}\",");
    }
    code.push_str("];\n\n");

    code.push_str("/// 对**所有**属性都合法的 CSS 全局取值，不必逐属性登记（升序）。\n");
    code.push_str("pub static UNIVERSAL_KEYWORDS: &[&str] = &[\n");
    for kw in ["inherit", "initial", "revert", "revert-layer", "unset"] {
        let _ = writeln!(code, "    \"{kw}\",");
    }
    code.push_str("];\n");
    code
}

/// 小写化 + 排序 + 去重。
///
/// `currentColor` 与 `Canvas` 这类系统颜色在 MDN 数据里是驼峰的，而排序是按
/// ASCII 的——大写字母排在所有小写之前。不统一小写，查表时对小写取值二分
/// 就会漏掉它们。
fn lowercased(keywords: &[String]) -> Vec<String> {
    let mut out: Vec<String> = keywords.iter().map(|k| k.to_ascii_lowercase()).collect();
    out.sort();
    out.dedup();
    out
}

/// 生成过程宏侧的「属性 → 取值能力与形态」表。
///
/// 与 `for_all_properties!` 的第四栏同源，但多带两位形态标志：`MULTI`
/// （取值可由多个顶层分量拼成）与 `OPEN`（语法里有裸标识符/字符串，能取什么
/// 无法穷举）。这两位在 `caps` 里都被压成了同一个 `Str`，不单独留一份，宏侧
/// 就分不清 `align-items`（`Str` 来自 `multi`）和 `font-family`（`Str` 来自
/// `<family-name>`）——前者该校验关键字，后者必须整条放行。
pub fn generate_property_caps_code(props: &[ProcessedProp]) -> String {
    let mut code = String::new();
    code.push_str("//! 自动生成：每个属性的取值能力与取值形态。\n");
    code.push_str("//!\n");
    code.push_str("//! 由 `silex_codegen` 从 MDN 值定义语法生成，与 `silex_css` 的\n");
    code.push_str("//! `for_all_properties!` 第四栏同源。\n\n");

    code.push_str("/// 位定义。\n");
    code.push_str("///\n");
    code.push_str("/// 前 11 位与 `silex_codegen::css::types::ValueCap` 一一对应，后两位是\n");
    code.push_str("/// 取值形态，`ValueCap` 里没有对应项。\n");
    code.push_str("pub mod cap {\n");
    let bits: &[(&str, &str)] = &[
        ("LENGTH", "`Px` / `Rem` / `Em` / `Vw` / `Vh`"),
        ("PERCENT", "`Percent`"),
        ("LEN_CALC", "`CalcValue<LengthMark>`"),
        ("NUM", "全部数值类型（含整数）"),
        ("INT", "仅整数类型"),
        ("ANGLE", "`Deg` / `Rad` / `Turn`"),
        ("TIME", "`Sec` / `Ms`"),
        ("FLEX", "`Fr`——网格轨道的 `<flex>`"),
        ("COLOR", "`Rgba` / `Hex` / `Hsl` / `ColorKeyword`"),
        ("URL", "`Url`——含 `<image>`"),
        ("STR", "裸字符串兜底"),
    ];
    for (i, (name, doc)) in bits.iter().enumerate() {
        let _ = writeln!(code, "    /// {doc}\n    pub const {name}: u16 = 1 << {i};");
    }
    code.push_str(
        "    /// 取值可以由多个**顶层分量**拼成（`<length>{1,4}`、`a && b`、`<x>#`）。\n",
    );
    code.push_str("    ///\n");
    code.push_str("    /// 没有这一位的属性写成 `color: 1px solid red` 是确定的错误。\n");
    let _ = writeln!(code, "    pub const MULTI: u16 = 1 << {};", bits.len());
    code.push_str("    /// 语法里有 `<custom-ident>` / `<string>` / 解析不出来的引用——\n");
    code.push_str("    /// 能取什么在编译期无法穷举，所有取值校验一律放行。\n");
    code.push_str("    ///\n");
    code.push_str("    /// `animation-name: fadeIn`、`font-family: Inter`、`grid-area: header`\n");
    code.push_str("    /// 都靠这一位过关。\n");
    let _ = writeln!(code, "    pub const OPEN: u16 = 1 << {};", bits.len() + 1);
    code.push('\n');
    code.push_str("    /// 数值类能力的并集。\n");
    code.push_str("    ///\n");
    code.push_str("    /// `calc()` 一族的量纲取决于操作数，宏侧不做求值；只在属性**完全\n");
    code.push_str("    /// 不接受任何数值类能力**时才判错。\n");
    code.push_str(
        "    pub const ANY_NUMERIC: u16 =\n        LENGTH | PERCENT | LEN_CALC | NUM | INT | ANGLE | TIME | FLEX;\n",
    );
    code.push_str("}\n\n");

    code.push_str("/// (属性名, 位掩码)，按属性名升序，可二分查找。\n");
    code.push_str("///\n");
    code.push_str("/// 查不到的属性（自定义变量、注册表外的厂商前缀属性）没有语法数据，\n");
    code.push_str("/// 宏侧一律跳过取值校验。\n");
    code.push_str("pub static PROPERTY_CAPS: &[(&str, u16)] = &[\n");
    for prop in props {
        let mut flags: Vec<&str> = prop
            .caps
            .iter()
            .map(|c| match c {
                ValueCap::Length => "LENGTH",
                ValueCap::Percent => "PERCENT",
                ValueCap::LenCalc => "LEN_CALC",
                ValueCap::Num => "NUM",
                ValueCap::Int => "INT",
                ValueCap::Angle => "ANGLE",
                ValueCap::Time => "TIME",
                ValueCap::Flex => "FLEX",
                ValueCap::Color => "COLOR",
                ValueCap::Url => "URL",
                ValueCap::Str => "STR",
            })
            .collect();
        if prop.multi {
            flags.push("MULTI");
        }
        if prop.open {
            flags.push("OPEN");
        }
        let mask = if flags.is_empty() {
            "0".to_string()
        } else {
            flags
                .iter()
                .map(|f| format!("cap::{f}"))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let _ = writeln!(code, "    (\"{}\", {}),", prop.name, mask);
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
