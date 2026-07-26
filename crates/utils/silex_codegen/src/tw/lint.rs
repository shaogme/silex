//! 生成链路的校验闸门。
//!
//! `scripts/export_tailwind` → `data/tailwind/*.json` → `silex_codegen` → `resolver/codegen/*.rs`
//! 这条链路此前没有任何断言，resolver 拼错的属性名（如 `skew-x`）会被原样收录进
//! `CssPropertyId` 表并生成到最终产物，用户侧表现为静默产出非法 CSS。

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use silex_tw_core::{ColorShadeInfo, JsonPalette, lookup_at_rule_utility, resolve_class};

/// 把一个类名解析成扁平的 `(属性, 值)` 序列，丢掉伴生选择器——
/// 属性名/值级别的校验不关心声明落在哪个选择器上。
fn flat_rules(
    class: &str,
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
) -> Option<Vec<(&'static str, String)>> {
    // at-rule 路径的类名走不到 `resolve_class`，但它们产出的声明同样要过属性/值闸门——
    // 否则这条路径就成了 lint 的盲区
    if let Some(u) = lookup_at_rule_utility(class) {
        return Some(
            u.groups
                .iter()
                .flat_map(|g| g.decls)
                .map(|&(prop, val)| (prop, val.to_string()))
                .chain(u.per_breakpoint.map(|prop| {
                    (
                        prop,
                        silex_tw_core::CONTAINER_TIERS
                            .first()
                            .map(|(_, w)| (*w).to_string())
                            .unwrap_or_default(),
                    )
                }))
                .collect(),
        );
    }
    let sets = resolve_class(class, &JsonPalette(palette))?;
    Some(
        sets.into_iter()
            .flat_map(|s| s.decls)
            .map(|d| (d.prop, d.value.into_owned()))
            .collect(),
    )
}

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
        let Some(rules) = flat_rules(class, palette) else {
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

/// 值必须写成函数调用（或 `none`）的组合型属性——裸值塞进去就是非法 CSS
const COMPOSITE_PROPS: &[&str] = &["filter", "backdrop-filter", "transform"];

/// 校验 codegen resolver 产出的 CSS **值**本身合法。
///
/// 属性名对了值仍可能是垃圾——报告 §2.2 的 `filter: 4px`、§2.3 的
/// `border-inline-start-style: 3px` 都是属性名合法、值荒谬的例子，
/// 只查属性名的 lint 拦不住它们。
pub fn validate_resolver_values(
    classes: &[String],
    test_cases: &[String],
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
) -> Result<(), String> {
    let mut errors: BTreeMap<String, String> = BTreeMap::new();
    let mut record = |key: String, detail: String| {
        errors.entry(key).or_insert(detail);
    };

    for class in classes.iter().chain(test_cases.iter()) {
        let Some(rules) = flat_rules(class, palette) else {
            continue;
        };
        for (prop, value) in rules {
            let value = value.trim();

            if value.is_empty() {
                record(
                    format!("{prop}/empty"),
                    format!("'{class}' 让 '{prop}' 取到了空值"),
                );
                continue;
            }

            if !parens_balanced(value) {
                record(
                    format!("{prop}/parens"),
                    format!("'{class}' 的 '{prop}: {value}' 括号不配对"),
                );
            }

            // 组合型属性只接受函数调用列表或 `none`
            if COMPOSITE_PROPS.contains(&prop)
                && value != "none"
                && !value.contains('(')
                && !value.starts_with("var(")
            {
                record(
                    format!("{prop}/bare"),
                    format!(
                        "'{class}' 的 '{prop}: {value}' 是裸值——组合型属性必须写成函数调用（缺 value_wrapper？）"
                    ),
                );
            }

            // 关键字属性不能收到长度/角度值
            if (prop.ends_with("-style") || prop.ends_with("-fit") || prop.ends_with("-repeat"))
                && looks_numeric(value)
            {
                record(
                    format!("{prop}/numeric"),
                    format!("'{class}' 给关键字属性 '{prop}' 赋了数值 '{value}'"),
                );
            }
        }
    }

    if errors.is_empty() {
        return Ok(());
    }
    Err(format!(
        "codegen resolver 产出了 {} 类非法 CSS 值:\n  - {}",
        errors.len(),
        errors.values().cloned().collect::<Vec<_>>().join("\n  - ")
    ))
}

fn parens_balanced(value: &str) -> bool {
    let mut depth = 0i32;
    for c in value.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// 是否形如 `3px` / `1.5rem` / `45deg` / `10` —— 关键字属性不该收到这种值
fn looks_numeric(value: &str) -> bool {
    let trimmed = value.trim_start_matches('-');
    let digits = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .count();
    digits > 0
        && trimmed[digits..]
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '%')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parens_balance_check() {
        assert!(parens_balanced("blur(4px)"));
        assert!(parens_balanced("repeat(3, minmax(0, 1fr))"));
        assert!(!parens_balanced("blur(4px"));
        assert!(!parens_balanced("blur4px)"));
    }

    #[test]
    fn numeric_detection_only_fires_on_numbers() {
        assert!(looks_numeric("3px"));
        assert!(looks_numeric("1.5rem"));
        assert!(looks_numeric("-45deg"));
        assert!(looks_numeric("10"));
        assert!(looks_numeric("50%"));
        assert!(!looks_numeric("solid"));
        assert!(!looks_numeric("var(--tw-border-style)"));
        assert!(!looks_numeric("cover"));
    }
}
