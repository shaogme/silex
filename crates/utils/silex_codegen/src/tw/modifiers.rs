use std::fmt::Write;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ModifierMetaJson {
    pub key: String,
    pub kind: String,
    pub priority: u32,
    pub css_selector: String,
}

/// 转义为可嵌入 Rust 字符串字面量的形式
fn escape_rust_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 生成 `silex_macros/src/css/tw/resolver/modifiers_gen.rs` 产物代码
pub fn generate_modifiers_code(modifiers: &[ModifierMetaJson]) -> String {
    let mut code = String::with_capacity(32 * 1024);
    code.push_str("// 自动生成的 Tailwind 修饰符与断点规则表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");
    code.push_str("use crate::css::tw::ast::Modifier;\n\n");

    code.push_str("pub struct ModifierMeta {\n");
    code.push_str("    pub key: &'static str,\n");
    code.push_str("    pub priority: u32,\n");
    code.push_str("    pub css_selector: &'static str,\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static MODIFIER_TABLE: &[ModifierMeta] = &[\n");
    for meta in modifiers {
        let _ = writeln!(
            code,
            "    ModifierMeta {{ key: \"{}\", priority: {}, css_selector: \"{}\" }},",
            meta.key,
            meta.priority,
            escape_rust_str(&meta.css_selector)
        );
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据修饰符 key 二分查找对应的元数据配置
pub fn lookup_modifier_meta(key: &str) -> Option<&'static ModifierMeta> {
    let idx = MODIFIER_TABLE.binary_search_by_key(&key, |m| m.key).ok()?;
    Some(&MODIFIER_TABLE[idx])
}

fn split_state_and_name_fast(rest: &str) -> (String, Option<String>) {
    if let Some(slash_idx) = rest.rfind('/') {
        let name_part = &rest[slash_idx + 1..];
        let state_part = &rest[..slash_idx];
        if !name_part.is_empty()
            && name_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let open_brackets = state_part.chars().filter(|&c| c == '[').count();
            let close_brackets = state_part.chars().filter(|&c| c == ']').count();
            if open_brackets == close_brackets {
                return (state_part.to_string(), Some(name_part.to_string()));
            }
        }
    }
    (rest.to_string(), None)
}

fn parse_bracket_kv_fast(rest: &str) -> (String, Option<String>) {
    if rest.starts_with('[') && rest.ends_with(']') {
        let inner = &rest[1..rest.len() - 1];
        if let Some((k, v)) = inner.split_once('=') {
            (k.to_string(), Some(v.to_string()))
        } else {
            (inner.to_string(), None)
        }
    } else {
        (rest.to_string(), None)
    }
}

fn parse_container_query_fast(container_spec: &str) -> Modifier {
    let (c_name, spec) = if let Some((name, rest)) = container_spec.split_once('/') {
        (Some(name.to_string()), rest)
    } else {
        (None, container_spec)
    };

    let min_width = match spec {
        "sm" => "640px".to_string(),
        "md" => "768px".to_string(),
        "lg" => "1024px".to_string(),
        "xl" => "1280px".to_string(),
        "2xl" => "1536px".to_string(),
        _ => {
            let cleaned = spec.strip_prefix("min-").unwrap_or(spec);
            let cleaned = cleaned.strip_prefix('-').unwrap_or(cleaned);
            if cleaned.starts_with('[') && cleaned.ends_with(']') {
                cleaned[1..cleaned.len() - 1].to_string()
            } else {
                cleaned.to_string()
            }
        }
    };

    Modifier::ContainerQuery {
        name: c_name,
        min_width,
    }
}

/// 编译期生成的 Modifier 状态机/快速匹配器
pub fn parse_modifier_fast(prefix: &str) -> Option<Modifier> {
    // 1. 编译期静态 match 确切 Modifier (比二分查找更快的 Match DFA)
    match prefix {
"#,
    );

    for meta in modifiers {
        let arm_expr = match meta.kind.as_str() {
            "Child" => "Modifier::Child".to_string(),
            "Descendant" => "Modifier::Descendant".to_string(),
            "Dark" => "Modifier::Dark".to_string(),
            "MediaBreakpoint" => format!("Modifier::MediaBreakpoint(\"{}\".to_string())", meta.key),
            "MediaFeature" => format!(
                "Modifier::MediaQuery(\"{}\".to_string())",
                escape_rust_str(&meta.css_selector)
            ),
            "SelectorVariant" => format!(
                "Modifier::SelectorVariant(\"{}\".to_string())",
                escape_rust_str(&meta.css_selector)
            ),
            "PseudoClass" => format!("Modifier::PseudoClass(\"{}\".to_string())", meta.key),
            "PseudoElement" => format!("Modifier::PseudoElement(\"{}\".to_string())", meta.key),
            _ => continue,
        };
        let _ = writeln!(
            code,
            "        \"{}\" => return Some({}),",
            meta.key, arm_expr
        );
    }

    code.push_str(
        r#"        _ => {}
    }

    // 2. 前缀状态匹配 (Prefix Dispatcher)
    if let Some(spec) = prefix.strip_prefix('@') {
        return Some(parse_container_query_fast(spec));
    }

    if let Some(rest) = prefix.strip_prefix("group-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Group { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("peer-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Peer { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("data-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        return Some(Modifier::DataAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("aria-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        let value = value.or_else(|| Some("true".to_string()));
        return Some(Modifier::AriaAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("has-") {
        return Some(Modifier::Has(format!("has-{}", rest)));
    }

    if prefix.starts_with('[') && prefix.ends_with(']') {
        return Some(Modifier::CustomSelector(prefix[1..prefix.len() - 1].to_string()));
    }

    None
}
"#,
    );

    code
}
