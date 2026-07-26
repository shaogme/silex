use std::{collections::BTreeMap, fmt::Write};

#[derive(serde::Deserialize, Debug)]
pub struct PrefixMetaJson {
    pub target_props: Vec<String>,
    pub unit_kind: String,
    pub value_wrapper: Option<String>,
}

/// `@property` 描述符成员，永远不应作为工具类的目标属性出现
const FORBIDDEN_TARGET_PROPS: &[&str] = &["syntax", "inherits", "initial-value"];

/// 必须配套 `value_wrapper` 的组合型属性（裸值写进去就是非法 CSS）
const WRAPPER_REQUIRED_PROPS: &[&str] = &["filter", "backdrop-filter", "transform"];

/// 数值单位种类（会给属性赋长度/角度值）
const NUMERIC_UNIT_KINDS: &[&str] = &["RemScale", "Pixel", "Degree", "Milliseconds"];

/// 校验 `prefix_metadata.json` 的合法性。
///
/// 这条链路（JS 探针 → JSON → 生成代码 → 宏）此前没有任何断言，
/// 探针把 `@property` 块里的 `syntax`/`inherits`/`initial-value` 当作 target_props
/// 一路生成到最终产物，用户侧表现为 `border-s-[3px]` 产出 `syntax:3px`。
pub fn validate_prefix_metadata(
    prefix_metadata: &BTreeMap<String, PrefixMetaJson>,
) -> Result<(), String> {
    let mut errors = Vec::new();

    for (prefix, meta) in prefix_metadata {
        if meta.target_props.is_empty() {
            errors.push(format!("'{}': target_props 为空", prefix));
        }

        for prop in &meta.target_props {
            if FORBIDDEN_TARGET_PROPS.contains(&prop.as_str()) {
                errors.push(format!(
                    "'{}': target_props 含 at-rule 描述符 '{}'（探针污染，应剥离 @property 块）",
                    prefix, prop
                ));
            }

            if WRAPPER_REQUIRED_PROPS.contains(&prop.as_str()) && meta.value_wrapper.is_none() {
                errors.push(format!(
                    "'{}': 目标属性 '{}' 为组合型属性，必须提供 value_wrapper（否则产出裸值的非法 CSS）",
                    prefix, prop
                ));
            }

            if prop.ends_with("-style") && NUMERIC_UNIT_KINDS.contains(&meta.unit_kind.as_str()) {
                errors.push(format!(
                    "'{}': 属性 '{}' 是关键字属性，不能配数值单位 UnitKind::{}",
                    prefix, prop, meta.unit_kind
                ));
            }
        }

        if let Some(w) = &meta.value_wrapper
            && !w.contains("{}")
        {
            errors.push(format!(
                "'{}': value_wrapper '{}' 缺少 '{{}}' 占位符",
                prefix, w
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "prefix_metadata.json 校验失败（{} 项）:\n  - {}",
            errors.len(),
            errors.join("\n  - ")
        ))
    }
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
