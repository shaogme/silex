use std::fmt::Write;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct KeyframeStepJson {
    pub selector: String,
    pub declarations: Vec<(String, String)>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct KeyframeMetaJson {
    pub name: String,
    pub steps: Vec<KeyframeStepJson>,
}

/// 生成 `silex_macros/src/css/tw/resolver/keyframes_gen.rs` 产物代码
pub fn generate_keyframes_code(keyframes: &[KeyframeMetaJson]) -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的动画 Keyframes 规则表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("pub struct KeyframeStep {\n");
    code.push_str("    pub selector: &'static str,\n");
    code.push_str("    pub declarations: &'static [(&'static str, &'static str)],\n");
    code.push_str("}\n\n");

    code.push_str("pub struct KeyframeMeta {\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    pub steps: &'static [KeyframeStep],\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static KEYFRAME_TABLE: &[KeyframeMeta] = &[\n");
    for meta in keyframes {
        let _ = writeln!(code, "    KeyframeMeta {{");
        let _ = writeln!(
            code,
            "        name: \"{}\",",
            meta.name.replace('\\', "\\\\").replace('"', "\\\"")
        );
        code.push_str("        steps: &[\n");
        for step in &meta.steps {
            let decls_str = step
                .declarations
                .iter()
                .map(|(p, v)| {
                    format!(
                        "(\"{}\", \"{}\")",
                        p.replace('\\', "\\\\").replace('"', "\\\""),
                        v.replace('\\', "\\\\").replace('"', "\\\"")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                code,
                "            KeyframeStep {{ selector: \"{}\", declarations: &[{}] }},",
                step.selector.replace('\\', "\\\\").replace('"', "\\\""),
                decls_str
            );
        }
        code.push_str("        ],\n");
        code.push_str("    },\n");
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据动画 keyframe 名称二分查找关键帧元数据配置
pub fn lookup_keyframe_meta(name: &str) -> Option<&'static KeyframeMeta> {
    let idx = KEYFRAME_TABLE.binary_search_by_key(&name, |k| k.name).ok()?;
    Some(&KEYFRAME_TABLE[idx])
}
"#,
    );

    code
}
