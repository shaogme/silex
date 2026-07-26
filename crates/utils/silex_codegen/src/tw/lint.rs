//! 生成链路的校验闸门。
//!
//! `scripts/export_tailwind` → `data/tailwind/*.json` → `silex_codegen` → `resolver/codegen/*.rs`
//! 这条链路此前没有任何断言，resolver 拼错的属性名（如 `skew-x`）会被原样收录进
//! `CssPropertyId` 表并生成到最终产物，用户侧表现为静默产出非法 CSS。

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use super::{palette::ColorShadeInfo, resolver::resolve_css_rules};

/// MDN 属性表未收录、但确实合法/项目自定义的属性白名单
const PROPERTY_ALLOWLIST: &[&str] = &[
    "container-name",
    "container-type",
    "scrollbar-color",
    "scrollbar-width",
    "text-wrap",
    "field-sizing",
    "interpolate-size",
    "overlay",
];

fn is_acceptable_property(prop: &str, standard: &BTreeSet<String>) -> bool {
    prop.starts_with("--")
        || prop.starts_with("-webkit-")
        || prop.starts_with("-moz-")
        || prop.starts_with("-ms-")
        || prop.starts_with("-o-")
        || standard.contains(prop)
        || PROPERTY_ALLOWLIST.contains(&prop)
}

/// 校验 codegen 侧 resolver 产出的所有 CSS 属性名都是真实存在的属性。
///
/// 返回 `Err` 时构建必须失败——绝不能让拼错的属性名混进 `CssPropertyId` 表。
pub fn validate_resolver_properties(
    classes: &[String],
    test_cases: &[String],
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
    props_json_str: &str,
) -> Result<(), String> {
    let standard: BTreeSet<String> =
        serde_json::from_str::<BTreeMap<String, Value>>(props_json_str)
            .map_err(|e| format!("解析 MDN 属性表失败: {}", e))?
            .into_keys()
            .collect();

    // 属性名 → 首个触发它的类名（便于定位问题规则）
    let mut offenders: BTreeMap<String, String> = BTreeMap::new();

    for class in classes.iter().chain(test_cases.iter()) {
        let Some(rules) = resolve_css_rules(class, palette) else {
            continue;
        };
        for (prop, _) in rules {
            if !is_acceptable_property(prop, &standard) {
                offenders
                    .entry(prop.to_string())
                    .or_insert_with(|| class.clone());
            }
        }
    }

    if offenders.is_empty() {
        return Ok(());
    }

    let detail = offenders
        .iter()
        .map(|(prop, class)| format!("'{}' (由类名 '{}' 产生)", prop, class))
        .collect::<Vec<_>>()
        .join("\n  - ");
    Err(format!(
        "codegen resolver 产出了 {} 个未知 CSS 属性:\n  - {}\n\
         请修正 silex_codegen/src/tw/resolver/**，或在 lint.rs 的 PROPERTY_ALLOWLIST 中显式登记。",
        offenders.len(),
        detail
    ))
}
