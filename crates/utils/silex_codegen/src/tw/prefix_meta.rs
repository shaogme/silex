use std::{collections::BTreeMap, fmt::Write};

#[derive(serde::Deserialize, Debug)]
pub struct PrefixMetaJson {
    pub target_props: Vec<String>,
    pub unit_kind: String,
    pub value_wrapper: Option<String>,
}

/// 生成 `silex_macros/src/css/tw/resolver/prefix_metadata.rs` 产物代码
pub fn generate_prefix_metadata_code(prefix_metadata: &BTreeMap<String, PrefixMetaJson>) -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的 Utility 前缀与单位元数据表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    code.push_str("pub enum UnitKind {\n");
    code.push_str("    RemScale,\n");
    code.push_str("    Pixel,\n");
    code.push_str("    Percentage,\n");
    code.push_str("    Degree,\n");
    code.push_str("    Milliseconds,\n");
    code.push_str("    Unitless,\n");
    code.push_str("    GridRepeat,\n");
    code.push_str("    GridSpan,\n");
    code.push_str("}\n\n");

    code.push_str("pub struct PrefixMeta {\n");
    code.push_str("    pub target_props: &'static [&'static str],\n");
    code.push_str("    pub unit_kind: UnitKind,\n");
    code.push_str("    pub value_wrapper: Option<&'static str>,\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PREFIX_METADATA: &[PrefixMeta] = &[\n");
    for meta in prefix_metadata.values() {
        let _ = write!(code, "    PrefixMeta {{ target_props: &[");
        for (i, p) in meta.target_props.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", p);
        }
        let wrapper_expr = match &meta.value_wrapper {
            Some(w) => format!("Some(\"{}\")", w),
            None => "None".to_string(),
        };
        let _ = writeln!(
            code,
            "], unit_kind: UnitKind::{}, value_wrapper: {} }},",
            meta.unit_kind, wrapper_expr
        );
    }
    code.push_str("];\n\n");

    code.push_str("/// 根据 Utility 前缀静态匹配对应的元数据配置\n");
    code.push_str("pub fn lookup_prefix_meta(prefix: &str) -> Option<&'static PrefixMeta> {\n");
    code.push_str("    match prefix {\n");
    for (i, (prefix, _)) in prefix_metadata.iter().enumerate() {
        let _ = writeln!(
            code,
            "        \"{}\" => Some(&PREFIX_METADATA[{}]),",
            prefix, i
        );
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}
