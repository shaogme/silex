use super::types::ProcessedProp;
use heck::AsPascalCase;

pub fn generate_registry_macro(props: &[ProcessedProp]) -> String {
    let mut code = String::new();
    code.push_str("/// 自动生成的 CSS 属性注册表\n");
    code.push_str("#[macro_export]\n");
    code.push_str("macro_rules! for_all_properties {\n");
    code.push_str("    ($callback:ident) => {\n");
    code.push_str("        $callback! {\n");

    let items: Vec<String> = props
        .iter()
        .map(|prop| {
            format!(
                "            ({}, \"{}\", {}, {})",
                prop.method_name,
                prop.name,
                prop.struct_name,
                prop.group.as_str()
            )
        })
        .collect();

    code.push_str(&items.join(",\n"));
    code.push_str("\n        }\n");
    code.push_str("    };\n");
    code.push_str("}\n");
    code
}

pub fn generate_keywords_code(props: &[ProcessedProp]) -> String {
    let mut code = String::new();
    code.push_str("// 自动生成的 CSS 关键字 Enums\n\n");

    let mut keyword_types = Vec::new();

    for prop in props {
        if !prop.keywords.is_empty() {
            let enum_name = format!("{}Keyword", prop.struct_name);
            keyword_types.push(enum_name.clone());

            code.push_str(&format!(
                "define_css_enum!({} (props::{}) {{\n",
                enum_name, prop.struct_name
            ));
            for kw in &prop.keywords {
                let mut variant = AsPascalCase(kw).to_string();

                // 1. If starts with digit, prepend underscore
                if variant.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    variant = format!("_{}", variant);
                }

                // 2. Handle Rust keywords and common conflicts
                if is_reserved_word(&variant) {
                    variant = format!("{}_", variant);
                }

                code.push_str(&format!("    {} => \"{}\",\n", variant, kw));
            }
            code.push_str("});\n\n");
        }
    }

    // Generate a helper macro to implement traits for all keywords
    code.push_str("macro_rules! register_generated_keywords {\n");
    code.push_str("    ($callback:ident) => {\n");
    code.push_str("        $callback! {\n");

    // Ensure deterministic order for keyword list
    keyword_types.sort();

    for (i, kt) in keyword_types.iter().enumerate() {
        if i == keyword_types.len() - 1 {
            code.push_str(&format!("            {}", kt));
        } else {
            code.push_str(&format!("            {},\n", kt));
        }
    }
    code.push_str("\n        }\n");
    code.push_str("    };\n");
    code.push_str("}\n");

    code
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
