//! 上游数据漂移闸门。
//!
//! 分析报告 §3.3：`scripts/export_tailwind` → `data/tailwind/*.json` → 生成代码
//! 这条链路上没有任何"这次生成改动了多少"的感知。Tailwind 升一个小版本可能悄悄改掉
//! 上千条类名的语义，而生成过程照常绿灯通过，没人会去逐行读 3 MB 的 `table.rs`。
//!
//! 这里给每份输入数据集算一个**分桶指纹**，与上次生成时的基线（`codegen_baseline.json`）比对：
//!
//! * 指纹完全一致 → 静默放行；
//! * 少量桶变化（小改动）→ 打印提示并自动刷新基线；
//! * 变化超过阈值 → **构建失败**，要求带 `--accept-drift` 重跑，逼人工确认一次。
//!
//! 之所以按桶而不是按文本行做 diff：生成代码写出来是未格式化的，落盘后还要过一遍
//! `cargo fmt`，逐行比对会被重排版淹没。分桶指纹只看数据本身，与排版无关。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 分桶数量。单条记录变化只会点亮 1 个桶（≈1.6%），批量变化会点亮绝大多数桶。
const BUCKETS: usize = 64;

/// 变化比例超过该阈值即视为"大规模变更"，需要人工确认
const DRIFT_THRESHOLD: f64 = 0.10;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct DatasetFingerprint {
    pub count: usize,
    pub buckets: Vec<u64>,
}

/// `codegen_baseline.json` 的结构：数据集名 → 指纹
pub type CodegenBaseline = BTreeMap<String, DatasetFingerprint>;

/// FNV-1a：不需要密码学强度，只需要稳定、跨平台一致、无外部依赖
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 为一组 `(键, 规范化后的值)` 记录计算分桶指纹。
///
/// 记录按**键**分桶，桶内对 `键=值` 做无序异或聚合——记录顺序变化不该被当成漂移。
pub fn fingerprint<'a, I>(records: I) -> DatasetFingerprint
where
    I: IntoIterator<Item = (&'a str, String)>,
{
    let mut buckets = vec![0u64; BUCKETS];
    let mut count = 0usize;
    for (key, value) in records {
        let bucket = (fnv1a(key.as_bytes()) as usize) % BUCKETS;
        buckets[bucket] ^= fnv1a(format!("{key}={value}").as_bytes());
        count += 1;
    }
    DatasetFingerprint { count, buckets }
}

/// 比对当前指纹与基线，返回需要打印的提示信息；超阈值时返回 `Err`。
pub fn check_drift(
    baseline: &CodegenBaseline,
    current: &CodegenBaseline,
    accept: bool,
) -> Result<Vec<String>, String> {
    let mut notices = Vec::new();
    let mut violations = Vec::new();

    for (name, now) in current {
        let Some(before) = baseline.get(name) else {
            notices.push(format!("[漂移] 新数据集 '{}'（{} 条）", name, now.count));
            continue;
        };
        if before == now {
            continue;
        }

        let changed_buckets = before
            .buckets
            .iter()
            .zip(&now.buckets)
            .filter(|(a, b)| a != b)
            .count();
        let ratio = changed_buckets as f64 / BUCKETS as f64;
        let count_delta = now.count as i64 - before.count as i64;

        let summary = format!(
            "'{}': {} → {} 条（{:+}），约 {:.0}% 的内容发生变化",
            name,
            before.count,
            now.count,
            count_delta,
            ratio * 100.0
        );

        if ratio > DRIFT_THRESHOLD && !accept {
            violations.push(summary);
        } else {
            notices.push(format!("[漂移] {}", summary));
        }
    }

    for name in baseline.keys() {
        if !current.contains_key(name) {
            notices.push(format!("[漂移] 数据集 '{}' 已不再生成", name));
        }
    }

    if violations.is_empty() {
        Ok(notices)
    } else {
        Err(format!(
            "检测到大规模数据漂移（超过 {:.0}% 阈值），生成已中止：\n  - {}\n\n\
             这通常意味着上游 Tailwind 版本变了。请先核对 `git diff data/tailwind/` 确认变更符合预期，\n\
             再带 `--accept-drift` 重新运行以刷新基线。",
            DRIFT_THRESHOLD * 100.0,
            violations.join("\n  - ")
        ))
    }
}

pub struct TailwindDatasetInputs<'a> {
    pub classes: &'a [String],
    pub dynamic_prefixes: &'a BTreeMap<String, Vec<String>>,
    pub prefix_metadata: &'a BTreeMap<String, super::PrefixMetaJson>,
    pub test_cases: &'a [String],
    pub palette: &'a BTreeMap<String, Vec<super::ColorShadeInfo>>,
    pub modifiers: &'a [super::ModifierMetaJson],
    pub keyframes: &'a [super::KeyframeMetaJson],
    pub reference_css: &'a super::ReferenceCssJson,
}

/// 为全部 Tailwind 输入数据集算指纹。
///
/// 值都压成一行紧凑文本——重点是"内容变没变"，不是可读性。
pub fn fingerprint_tw_datasets(inputs: TailwindDatasetInputs<'_>) -> CodegenBaseline {
    let TailwindDatasetInputs {
        classes,
        dynamic_prefixes,
        prefix_metadata,
        test_cases,
        palette,
        modifiers,
        keyframes,
        reference_css,
    } = inputs;
    let mut out = CodegenBaseline::new();

    out.insert(
        "classes".into(),
        fingerprint(classes.iter().map(|c| (c.as_str(), String::new()))),
    );
    out.insert(
        "test_cases".into(),
        fingerprint(test_cases.iter().map(|c| (c.as_str(), String::new()))),
    );
    out.insert(
        "dynamic_prefixes".into(),
        fingerprint(
            dynamic_prefixes
                .iter()
                .map(|(k, v)| (k.as_str(), v.join(","))),
        ),
    );
    out.insert(
        "prefix_metadata".into(),
        fingerprint(prefix_metadata.iter().map(|(k, m)| {
            (
                k.as_str(),
                format!(
                    "{}|{}|{}",
                    m.target_props.join(","),
                    m.unit_kind,
                    m.value_wrapper.as_deref().unwrap_or("")
                ),
            )
        })),
    );
    out.insert(
        "palette".into(),
        fingerprint(palette.iter().map(|(family, shades)| {
            (
                family.as_str(),
                shades
                    .iter()
                    .map(|s| format!("{}:{}", s.shade, s.raw))
                    .collect::<Vec<_>>()
                    .join(","),
            )
        })),
    );
    out.insert(
        "modifiers".into(),
        fingerprint(
            modifiers
                .iter()
                .map(|m| (m.key.as_str(), format!("{}|{}", m.kind, m.css_selector))),
        ),
    );
    out.insert(
        "keyframes".into(),
        fingerprint(keyframes.iter().map(|k| {
            (
                k.name.as_str(),
                k.steps
                    .iter()
                    .map(|s| {
                        format!(
                            "{}{{{}}}",
                            s.selector,
                            s.declarations
                                .iter()
                                .map(|(p, v)| format!("{p}:{v}"))
                                .collect::<Vec<_>>()
                                .join(";")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            )
        })),
    );
    out.insert(
        "reference_css".into(),
        fingerprint(reference_css.iter().map(|(class, decls)| {
            (
                class.as_str(),
                decls
                    .iter()
                    .map(|(p, v)| format!("{p}:{v}"))
                    .collect::<Vec<_>>()
                    .join(";"),
            )
        })),
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(items: &[(&str, &str)]) -> DatasetFingerprint {
        fingerprint(items.iter().map(|(k, v)| (*k, v.to_string())))
    }

    #[test]
    fn identical_data_produces_identical_fingerprints() {
        let a = fp(&[("p-4", "padding:1rem"), ("p-8", "padding:2rem")]);
        // 顺序变化不算漂移
        let b = fp(&[("p-8", "padding:2rem"), ("p-4", "padding:1rem")]);
        assert_eq!(a, b);
    }

    #[test]
    fn a_single_value_change_is_reported_but_allowed() {
        let before: CodegenBaseline = [("t".to_string(), fp(&[("p-4", "padding:1rem")]))].into();
        let after: CodegenBaseline = [("t".to_string(), fp(&[("p-4", "padding:2rem")]))].into();
        let notices = check_drift(&before, &after, false).expect("单条变化不应中止生成");
        assert_eq!(notices.len(), 1);
        assert!(notices[0].contains("'t'"), "{notices:?}");
    }

    #[test]
    fn mass_change_requires_explicit_acceptance() {
        let old_items: Vec<(String, String)> = (0..500)
            .map(|i| (format!("c-{i}"), format!("v{i}")))
            .collect();
        let new_items: Vec<(String, String)> = (0..500)
            .map(|i| (format!("c-{i}"), format!("changed{i}")))
            .collect();
        let before: CodegenBaseline = [(
            "t".to_string(),
            fingerprint(old_items.iter().map(|(k, v)| (k.as_str(), v.clone()))),
        )]
        .into();
        let after: CodegenBaseline = [(
            "t".to_string(),
            fingerprint(new_items.iter().map(|(k, v)| (k.as_str(), v.clone()))),
        )]
        .into();

        assert!(check_drift(&before, &after, false).is_err());
        assert!(check_drift(&before, &after, true).is_ok());
    }
}
