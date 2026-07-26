use std::{collections::BTreeMap, fmt::Write};

use silex_tw_core::ColorShadeInfo;

/// 生成 `silex_macros/src/css/tw/resolver/palette_gen.rs` 产物代码
pub fn generate_palette_code(palette: &BTreeMap<String, Vec<ColorShadeInfo>>) -> String {
    let mut code = String::with_capacity(32 * 1024);
    code.push_str("// 自动生成的 Tailwind 标准色板表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PALETTE_TABLE: &[(&str, [&str; 11])] = &[\n");
    for (name, shades) in palette {
        let _ = write!(code, "    (\"{}\", [", name);
        for (i, info) in shades.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", info.hex);
        }
        code.push_str("]),\n");
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据色系名称获取标准的 11 阶梯 Hex 阵列
pub fn get_raw_palette(color_name: &str) -> Option<[&'static str; 11]> {
    let idx = PALETTE_TABLE.binary_search_by_key(&color_name, |&(k, _)| k).ok()?;
    Some(PALETTE_TABLE[idx].1)
}

"#,
    );

    code.push_str("/// 编译期生成的 O(1) 静态色板 Hex 匹配器\n");
    code.push_str("pub fn lookup_palette_color_fast(color_name: &str, shade: &str) -> Option<&'static str> {\n");
    code.push_str("    match (color_name, shade) {\n");
    for (name, shades) in palette {
        for info in shades {
            let _ = writeln!(
                code,
                "        (\"{}\", \"{}\") => Some(\"{}\"),",
                name, info.shade, info.hex
            );
        }
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}
