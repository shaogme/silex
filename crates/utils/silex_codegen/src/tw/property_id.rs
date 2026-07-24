use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
use serde_json::Value;

use super::{palette::ColorShadeInfo, resolver::resolve_css_rules};

pub fn to_pascal_case(s: &str) -> String {
    let (prefix, rest) = if let Some(stripped) = s.strip_prefix("--") {
        ("Var", stripped)
    } else if let Some(stripped) = s.strip_prefix('-') {
        ("Neg", stripped)
    } else {
        ("", s)
    };

    let pascal: String = rest
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();

    format!("{}{}", prefix, pascal)
}

fn flatten_prop(
    prop: &str,
    raw_map: &BTreeMap<String, Vec<String>>,
    out: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(prop.to_string()) {
        return;
    }
    if let Some(subs) = raw_map.get(prop) {
        for sub in subs {
            if raw_map.contains_key(sub) {
                flatten_prop(sub, raw_map, out, visited);
            } else {
                out.insert(sub.clone());
            }
        }
    } else {
        out.insert(prop.to_string());
    }
}

/// 生成 `silex_macros/src/css/tw/resolver/codegen/property_id.rs` 产物代码
pub fn generate_property_id_code(
    props_json_str: &str,
    classes: &[String],
    test_cases: &[String],
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
    extra_props: &[String],
    property_aliases: &BTreeMap<String, Vec<String>>,
    prefix_metadata: &BTreeMap<String, super::prefix_meta::PrefixMetaJson>,
) -> String {
    let mut props_set: BTreeSet<String> = BTreeSet::new();

    // 0. 从所有的 Tailwind 静态类及测试用例解析结果中收集用到的全量 CSS 属性名
    for class in classes.iter().chain(test_cases.iter()) {
        if let Some(rules) = resolve_css_rules(class, palette) {
            for (prop, _) in rules {
                props_set.insert(prop.to_string());
            }
        }
    }

    // 1. 从 MDN JSON 中提取标准 CSS 属性
    if let Ok(json_map) = serde_json::from_str::<BTreeMap<String, Value>>(props_json_str) {
        for prop_name in json_map.keys() {
            if prop_name.starts_with('-') || prop_name.starts_with("--") {
                continue;
            }
            props_set.insert(prop_name.clone());
        }
    }

    // 2. 动态注入从 Node.js 自动推导导出的 Tailwind / 简写与别名映射关系
    for (k, subs) in property_aliases {
        props_set.insert(k.clone());
        for s in subs {
            props_set.insert(s.clone());
        }
    }

    // 注入所有 Node 导出提取的 CSS 自定义变量与特殊前缀属性
    for v in extra_props {
        props_set.insert(v.clone());
    }

    // 注入所有 prefix_metadata 中的 target_props
    for meta in prefix_metadata.values() {
        for prop in &meta.target_props {
            props_set.insert(prop.clone());
        }
    }

    // 检测 Enum Variant 重复与碰撞
    let mut variant_map: BTreeMap<String, String> = BTreeMap::new();
    for prop in &props_set {
        let variant = to_pascal_case(prop);
        if let Some(existing) = variant_map.insert(variant.clone(), prop.clone()) {
            panic!(
                "CssPropertyId enum variant collision detected: '{}' and '{}' both map to variant '{}'",
                existing, prop, variant
            );
        }
    }

    // 构建 raw_map 以计算连通分量 (Bitmask Group)
    let mut raw_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Ok(json_map) = serde_json::from_str::<BTreeMap<String, Value>>(props_json_str) {
        for (prop_name, prop_val) in json_map {
            if prop_name.starts_with('-') || prop_name.starts_with("--") {
                continue;
            }
            if let Some(comp_arr) = prop_val.get("computed").and_then(|v| v.as_array()) {
                let subs: Vec<String> = comp_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| s != &prop_name)
                    .collect();
                if !subs.is_empty() {
                    raw_map.insert(prop_name.clone(), subs);
                }
            }
        }
    }

    // 载入自动推导出的简写属性关联
    for (k, subs) in property_aliases {
        raw_map.insert(k.clone(), subs.clone());
    }

    // 展开所有 shorthand 映射 final_map: prop -> BTreeSet<atomic_subproperty>
    let mut final_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for prop in raw_map.keys() {
        let mut atomic_set = BTreeSet::new();
        flatten_prop(prop, &raw_map, &mut atomic_set, &mut BTreeSet::new());
        if !atomic_set.is_empty() {
            let mut list: Vec<String> = atomic_set.into_iter().collect();
            list.sort();
            final_map.insert(prop.clone(), list);
        }
    }

    // 确保所有在 final_map 中出现的 subproperties 也在 props_set 中
    for (k, subs) in &final_map {
        props_set.insert(k.clone());
        for s in subs {
            props_set.insert(s.clone());
        }
    }

    // --- 计算连通分量 (Connected Components for Bitmask Groups) ---
    let mut all_atomic_subprops: BTreeSet<String> = BTreeSet::new();
    for subs in final_map.values() {
        for s in subs {
            all_atomic_subprops.insert(s.clone());
        }
    }

    let atomic_list: Vec<String> = all_atomic_subprops.into_iter().collect();
    let mut adj: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for a in &atomic_list {
        adj.insert(a.clone(), BTreeSet::new());
    }
    for subs in final_map.values() {
        for i in 0..subs.len() {
            for j in (i + 1)..subs.len() {
                adj.get_mut(&subs[i]).unwrap().insert(subs[j].clone());
                adj.get_mut(&subs[j]).unwrap().insert(subs[i].clone());
            }
        }
    }

    let mut atomic_info: BTreeMap<String, (u16, u64)> = BTreeMap::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut current_group_id: u16 = 1;

    for node in &atomic_list {
        if visited.contains(node) {
            continue;
        }
        let group_id = current_group_id;
        current_group_id += 1;

        let mut queue = vec![node.clone()];
        visited.insert(node.clone());
        let mut group_nodes = Vec::new();

        while let Some(curr) = queue.pop() {
            group_nodes.push(curr.clone());
            if let Some(neighbors) = adj.get(&curr) {
                for nbr in neighbors {
                    if visited.insert(nbr.clone()) {
                        queue.push(nbr.clone());
                    }
                }
            }
        }

        assert!(
            group_nodes.len() <= 64,
            "Bitmask group {} size overflow: contains {} nodes (max 64 supported)",
            group_id,
            group_nodes.len()
        );

        for (bit_idx, name) in group_nodes.iter().enumerate() {
            let mask = 1u64 << bit_idx;
            atomic_info.insert(name.clone(), (group_id, mask));
        }
    }

    let mut prop_bitmasks: BTreeMap<String, (u16, u64)> = BTreeMap::new();
    for (prop, subs) in &final_map {
        let mut combined_mask = 0u64;
        let mut g_id = 0u16;
        for s in subs {
            if let Some(&(gid, m)) = atomic_info.get(s) {
                g_id = gid;
                combined_mask |= m;
            }
        }
        if g_id != 0 {
            prop_bitmasks.insert(prop.clone(), (g_id, combined_mask));
        }
    }
    for (s, &(gid, m)) in &atomic_info {
        prop_bitmasks.insert(s.clone(), (gid, m));
    }

    for p in &props_set {
        if !prop_bitmasks.contains_key(p) {
            let gid = current_group_id;
            current_group_id += 1;
            prop_bitmasks.insert(p.clone(), (gid, 1u64));
        }
    }

    // 生成 Rust 代码
    let mut code = String::with_capacity(64 * 1024);
    code.push_str("// 自动生成的 CSS 属性 Enum 与 Bitmask 对照表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum CssPropertyId {\n");
    for prop in &props_set {
        let variant = to_pascal_case(prop);
        let _ = writeln!(code, "    {},", variant);
    }
    code.push_str("}\n\n");

    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    code.push_str("pub struct PropertyBitmask {\n");
    code.push_str("    pub group_id: u16,\n");
    code.push_str("    pub mask: u64,\n");
    code.push_str("}\n\n");

    code.push_str("impl CssPropertyId {\n");
    code.push_str("    #[inline]\n");
    code.push_str("    #[must_use]\n");
    code.push_str("    pub fn as_str(&self) -> &str {\n");
    code.push_str("        match self {\n");
    for prop in &props_set {
        let variant = to_pascal_case(prop);
        let _ = writeln!(code, "            Self::{} => \"{}\",", variant, prop);
    }
    code.push_str("        }\n");
    code.push_str("    }\n\n");

    code.push_str("    #[inline]\n");
    code.push_str("    #[must_use]\n");
    code.push_str("    pub fn parse(s: &str) -> Option<Self> {\n");
    code.push_str("        match s {\n");
    for prop in &props_set {
        let variant = to_pascal_case(prop);
        let _ = writeln!(code, "            \"{}\" => Some(Self::{}),", prop, variant);
    }
    code.push_str("            _ => None,\n");
    code.push_str("        }\n");
    code.push_str("    }\n\n");

    code.push_str("    #[inline]\n");
    code.push_str("    #[must_use]\n");
    code.push_str("    pub fn bitmask(&self) -> PropertyBitmask {\n");
    code.push_str("        match self {\n");
    for prop in &props_set {
        let variant = to_pascal_case(prop);
        let (gid, mask) = prop_bitmasks.get(prop).copied().unwrap_or((0xffff, 1));
        let _ = writeln!(
            code,
            "            Self::{} => PropertyBitmask {{ group_id: {}, mask: {} }},",
            variant, gid, mask
        );
    }
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str("impl PartialEq<&str> for CssPropertyId {\n");
    code.push_str("    #[inline]\n");
    code.push_str("    fn eq(&self, other: &&str) -> bool {\n");
    code.push_str("        self.as_str() == *other\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str("impl PartialEq<CssPropertyId> for &str {\n");
    code.push_str("    #[inline]\n");
    code.push_str("    fn eq(&self, other: &CssPropertyId) -> bool {\n");
    code.push_str("        *self == other.as_str()\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str("impl PartialEq<str> for CssPropertyId {\n");
    code.push_str("    #[inline]\n");
    code.push_str("    fn eq(&self, other: &str) -> bool {\n");
    code.push_str("        self.as_str() == other\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str("impl std::fmt::Display for CssPropertyId {\n");
    code.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
    code.push_str("        write!(f, \"{}\", self.as_str())\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}
