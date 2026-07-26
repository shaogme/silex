//! 与真实 `tailwindcss` 的对拍测试（differential testing）。
//!
//! 分析报告 §6.3.2：`table_examples.rs`（现有 e2e 夹具）与 `table.rs`（被测数据）由同一个
//! codegen 从同一份 JSON 生成，验证的是"生成器与自己一致"——§2.3 的 `syntax:3px` 污染正是
//! 这样逃过检测的。本模块引入一个**外部真值**：`reference_css.rs` 里的每一条都是真实
//! `tailwindcss@4.3.3` 自己编译出来的 CSS（theme 变量已展开、`calc()` 已求值、oklch 已转 hex）。
//!
//! 比较策略分两层：
//!
//! * **属性覆盖**——Tailwind 产出的每个"实体"CSS 属性都必须出现在 `tw!` 的产物里。
//!   这一层能抓住 ring→outline-color、blur 丢 wrapper、`syntax`/`inherits` 污染、
//!   `object-fill`、`skew-x` 这类映射错误。
//! * **值一致**——共有属性的值规范化后必须相等。
//!
//! 两侧实现机制不同（Tailwind 大量使用 `--tw-*` 运行时变量组合，silex 直接求值）之处，
//! 记录在 [`KNOWN_DIVERGENCES`] 台账里并说明理由；**台账之外的任何差异都会让测试失败**。
//! 台账本身也被测试守护：登记了但实际已经一致的条目会被报为"过期"，强制清理。

use super::css_probe::{class_declarations, decls_to_map, values_equivalent};
use crate::css::tw::resolver::codegen::reference_css::REFERENCE_CSS;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// 已知偏差台账
// ---------------------------------------------------------------------------

/// 偏差类别——决定该类名在对拍中被豁免到什么程度
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Divergence {
    /// silex 尚未支持该类名（`tw!` 直接报错）。属功能缺口，不是错误产出。
    Unsupported,
    /// 属性集合一致，但值的表达方式不同（例如 color-mix vs 预乘 alpha 的 hex）。
    ValueOnly,
    /// 实现机制不同导致属性集合也不同（例如 ring / shadow 的变量体系）。
    Mechanism,
}

/// `(类名, 类别, 理由)`
///
/// 新增条目必须写清楚理由，并在报告的路线图里有对应的待办项——
/// 台账是"已知且已决定暂不处理"的清单，不是"跑不过就往里塞"的垃圾桶。
#[rustfmt::skip]
const KNOWN_DIVERGENCES: &[(&str, Divergence, &str)] = &[
    // --- 缺陷：多义前缀盲映射（报告 §2.8，第三阶段第 13 项）-------------------
    ("text-[14px]", Divergence::Mechanism,
     "text-<长度> 应映射到 font-size，当前 color_prefix_to_prop 无条件返回 color，产出 `color:14px`"),

    // --- 缺陷：数值型 flex 被当作长度（本轮对拍新发现）-----------------------
    ("flex-4",  Divergence::ValueOnly, "flex-<数字> 应为 `flex:4`，当前按 RemScale 求值成 `flex:1rem`"),
    ("flex-10", Divergence::ValueOnly, "同 flex-4"),

    // --- 缺陷：columns 映射到了 column-count（本轮对拍新发现）----------------
    // `columns-lg` 这类值是宽度（32rem），赋给 column-count 是非法 CSS。
    ("columns-0",   Divergence::Mechanism, "应产出 columns 简写，当前产出 column-count"),
    ("columns-1",   Divergence::Mechanism, "同 columns-0"),
    ("columns-2",   Divergence::Mechanism, "同 columns-0"),
    ("columns-4",   Divergence::Mechanism, "同 columns-0"),
    ("columns-32",  Divergence::Mechanism, "同 columns-0"),
    ("columns-48",  Divergence::Mechanism, "同 columns-0"),
    ("columns-60",  Divergence::Mechanism, "同 columns-0"),
    ("columns-80",  Divergence::Mechanism, "同 columns-0"),
    ("columns-auto",Divergence::Mechanism, "同 columns-0"),
    ("columns-xs",  Divergence::Mechanism, "宽度档位赋给 column-count，是非法 CSS"),
    ("columns-sm",  Divergence::Mechanism, "同 columns-xs"),
    ("columns-md",  Divergence::Mechanism, "同 columns-xs"),
    ("columns-lg",  Divergence::Mechanism, "同 columns-xs"),
    ("columns-xl",  Divergence::Mechanism, "同 columns-xs"),
    ("columns-4xl", Divergence::Mechanism, "同 columns-xs"),

    // --- 缺陷：outline-none / outline-hidden 语义互换（报告 §2.10）-----------
    ("outline-none",   Divergence::Mechanism, "v4 中应为 outline-style:none，当前产出 outline:2px solid transparent"),
    ("outline-hidden", Divergence::Mechanism, "v4 中 outline-hidden 才是 2px solid transparent + outline-offset"),

    // --- 缺陷：container 被当成 container-type（报告 §2.10）------------------
    ("container", Divergence::Mechanism,
     "Tailwind 的 container 是 width:100% + 各断点 max-width 的容器工具类，不是 container-type"),

    // --- 功能缺口：transition 家族不完整（第四阶段）--------------------------
    ("transition",        Divergence::Mechanism, "缺 transition-duration / timing-function；property 列表停留在 v3"),
    ("transition-all",    Divergence::Mechanism, "同 transition"),
    ("transition-colors", Divergence::Mechanism, "同 transition"),
    ("transition-normal", Divergence::Mechanism, "未支持 transition-behavior"),

    // --- 功能缺口：mask / sr-only 的现代属性（第四阶段）----------------------
    ("-mask-conic-0",              Divergence::Mechanism, "未产出 mask-composite"),
    ("mask-conic-from-violet-800", Divergence::Mechanism, "未产出 mask-composite"),
    ("mask-l-to-yellow-600",       Divergence::Mechanism, "未产出 mask-composite"),
    ("mask-r-from-yellow-100",     Divergence::Mechanism, "未产出 mask-composite"),
    ("mask-radial-to-yellow-300",  Divergence::Mechanism, "未产出 mask-composite"),
    ("mask-x-from-yellow-500",     Divergence::Mechanism, "未产出 mask-composite"),
    ("sr-only",     Divergence::Mechanism, "只产出了旧的 clip:rect()，缺现代的 clip-path:inset(50%)"),
    ("not-sr-only", Divergence::Mechanism, "缺 clip-path:none"),

    // --- 设计取舍：divide / space 的 reverse 变量体系（报告 §3.2）------------
    ("divide-x-2", Divergence::Mechanism,
     "Tailwind 用 --tw-divide-x-reverse 支持 divide-x-reverse；silex 直接产出固定方向的边框"),
    ("space-x-4", Divergence::Mechanism,
     "同 divide-x-2，Tailwind 用 --tw-space-x-reverse 走 margin-inline-start/end"),

    // --- 设计取舍：字号与行高解耦（报告 §3.2）--------------------------------
    // Tailwind 用 `var(--tw-leading, var(--text-sm--line-height))` 让 `leading-*` 能单独覆盖行高，
    // silex 在编译期把行高求成字面量。视觉结果相同，但 `text-sm leading-8` 的覆盖依赖声明顺序。
    ("text-xs",  Divergence::ValueOnly, "line-height 求值为字面量而非 --tw-leading 变量链"),
    ("text-sm",  Divergence::ValueOnly, "同 text-xs"),
    ("text-lg",  Divergence::ValueOnly, "同 text-xs"),
    ("text-xl",  Divergence::ValueOnly, "同 text-xs"),
    ("text-2xl", Divergence::ValueOnly, "同 text-xs"),
    ("text-8xl", Divergence::ValueOnly, "同 text-xs"),

    // --- 设计取舍：渐变方向内联而非走 --tw-gradient-position ------------------
    ("bg-linear-to-r", Divergence::ValueOnly, "方向内联进 linear-gradient()，Tailwind 放在 --tw-gradient-position"),
    ("bg-linear-to-b", Divergence::ValueOnly, "同 bg-linear-to-r"),
    ("-bg-conic-0",    Divergence::ValueOnly, "同 bg-linear-to-r（且 `from-0deg` 少了空格，见报告 §11）"),

    // --- 设计取舍：不透明度用预乘 alpha 而非 color-mix ------------------------
    ("bg-red-500/50", Divergence::ValueOnly,
     "silex 产出 #fb2c3680（sRGB 预乘），Tailwind 产出 color-mix(in oklab,…)——色域不同，视觉有极小差异"),

    // --- 设计取舍：无穷大圆角写成 9999px --------------------------------------
    // Tailwind v4 用 `calc(infinity * 1px)`，silex 用 9999px。两者在任何实际尺寸下等效。
    ("rounded-full",    Divergence::ValueOnly, "9999px 代替 calc(infinity * 1px)"),
    ("rounded-b-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-t-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-l-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-r-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-s-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-e-full",  Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-bl-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-br-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-tl-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-tr-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-ss-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-se-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-es-full", Divergence::ValueOnly, "同 rounded-full"),
    ("rounded-ee-full", Divergence::ValueOnly, "同 rounded-full"),

    // --- 其余零散差异 --------------------------------------------------------
    ("font-mono", Divergence::ValueOnly,
     "字体栈少了 \"Liberation Mono\" / \"Courier New\" 两个回退项"),
    ("font-stretch-normal", Divergence::ValueOnly, "产出 100%，Tailwind 产出关键字 normal（等价）"),
    ("place-items-stretch", Divergence::ValueOnly, "产出双值 `stretch stretch`，Tailwind 用单值简写（等价）"),
];

fn divergence_of(class: &str) -> Option<Divergence> {
    KNOWN_DIVERGENCES
        .iter()
        .find(|(c, _, _)| *c == class)
        .map(|(_, d, _)| *d)
}

// ---------------------------------------------------------------------------
// 属性级豁免
// ---------------------------------------------------------------------------

/// 不参与对拍的属性前缀/名称。
///
/// `--tw-*` 是两侧各自的**内部实现细节**：Tailwind 靠它们在运行时组合 transform/filter/shadow，
/// silex 在编译期就求好值。逐个比对这些变量等于把对方的实现方案当成规范，没有意义；
/// 真正要比的是最终落到实体 CSS 属性上的结果。
fn is_plumbing_property(prop: &str) -> bool {
    prop.starts_with("--tw-")
}

/// 值是否**完全**由 `--tw-*` 变量拼接而成（分隔符之外没有实质内容）。
///
/// 例如 Tailwind 的 `transform: var(--tw-rotate-x,) var(--tw-rotate-y,) …`——
/// 这条声明本身不携带任何语义，真正的值在那些变量里。silex 在编译期直接把变换求值成
/// `translateX(2px)`，两侧的字符串永远不可能相等，比较它没有意义。
/// 这种情况只校验属性是否存在，不校验值。
fn is_plumbing_value(value: &str) -> bool {
    let mut rest = value;
    let mut had_var = false;
    let mut residue = String::new();

    while let Some(idx) = rest.find("var(--tw-") {
        residue.push_str(&rest[..idx]);
        let after = &rest[idx + 4..];
        let mut depth = 0i32;
        let mut end = after.len();
        for (i, c) in after.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        had_var = true;
        rest = &after[end..];
    }
    residue.push_str(rest);

    had_var
        && residue
            .chars()
            .all(|c| c.is_whitespace() || c == ',' || c == '/')
}

/// 逻辑属性 → silex 实际产出的物理属性。
///
/// Tailwind v4 的 `mx-4` 产出 `margin-inline: 1rem`，silex 产出
/// `margin-left: 1rem; margin-right: 1rem`。对于**对称**工具类（`mx`/`my`/`px`/`border-x` …
/// 两侧取同一个值）这两种写法在任何书写方向下都等价，属于表达方式差异而非语义差异。
///
/// 只有当全部物理对应属性都存在**且值相同**时才算满足——少写一边会被照常报出来。
#[rustfmt::skip]
const LOGICAL_EQUIVALENTS: &[(&str, &[&str])] = &[
    ("margin-inline",  &["margin-left", "margin-right"]),
    ("margin-block",   &["margin-top", "margin-bottom"]),
    ("padding-inline", &["padding-left", "padding-right"]),
    ("padding-block",  &["padding-top", "padding-bottom"]),
    ("inset-inline",   &["left", "right"]),
    ("inset-block",    &["top", "bottom"]),
    ("border-inline-width", &["border-left-width", "border-right-width"]),
    ("border-block-width",  &["border-top-width", "border-bottom-width"]),
    ("border-inline-color", &["border-left-color", "border-right-color"]),
    ("border-block-color",  &["border-top-color", "border-bottom-color"]),
    ("scroll-margin-inline",  &["scroll-margin-left", "scroll-margin-right"]),
    ("scroll-margin-block",   &["scroll-margin-top", "scroll-margin-bottom"]),
    ("scroll-padding-inline", &["scroll-padding-left", "scroll-padding-right"]),
    ("scroll-padding-block",  &["scroll-padding-top", "scroll-padding-bottom"]),
    ("inset-inline-start", &["left"]),
    ("inset-inline-end",   &["right"]),
];

/// Tailwind v4 用**独立**的 `rotate` / `scale` / `translate` 属性，silex 统一走 `transform`
/// 函数式写法。两种写法的渲染结果一致（浏览器把独立属性合成到同一个变换矩阵），
/// 属于机制差异，只要 `transform` 里出现了对应的变换函数就算满足。
#[rustfmt::skip]
const TRANSFORM_EQUIVALENTS: &[(&str, &[&str])] = &[
    ("rotate",    &["rotate("]),
    ("scale",     &["scale(", "scalex(", "scaley("]),
    ("translate", &["translate(", "translatex(", "translatey("]),
];

/// 去掉厂商前缀得到基础属性名；不是厂商前缀属性时返回 `None`。
///
/// Tailwind 会同时产出 `-webkit-backdrop-filter` 与 `backdrop-filter`，也会用
/// `-moz-osx-font-smoothing` 与 `-webkit-font-smoothing` 这种"同一件事、不同厂商、
/// 连取值词汇都不同"的组合。加不加前缀、加哪家的前缀由 LightningCSS 按 browser targets
/// 决定，是构建配置问题而不是语义问题——只要基础属性在 silex 侧出现过就认为满足。
fn vendor_base(prop: &str) -> Option<&str> {
    for p in ["-webkit-", "-moz-osx-", "-moz-", "-ms-", "-o-"] {
        if let Some(rest) = prop.strip_prefix(p) {
            return Some(rest);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 对拍
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Mismatch {
    /// `tw!` 拒绝了一个合法的 Tailwind 类名
    Rejected(String),
    /// Tailwind 产出了某属性，silex 没有
    MissingProperty { prop: String, expected: String },
    /// 同一属性两侧的值不同
    ValueDiff {
        prop: String,
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
}

fn compare(class: &str, reference: &[(&str, &str)]) -> Vec<Mismatch> {
    let actual_decls = match class_declarations(class) {
        Ok(d) => d,
        Err(e) => return vec![Mismatch::Rejected(e)],
    };

    let expected = decls_to_map(reference.iter().copied());
    let actual = decls_to_map(
        actual_decls
            .iter()
            .map(|(p, v)| (p.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
    );
    // 值完全由 `--tw-*` 变量拼成的声明本身不携带语义，整条跳过
    let plumbing_only: BTreeSet<&str> = reference
        .iter()
        .filter(|(_, v)| is_plumbing_value(v))
        .map(|(p, _)| *p)
        .collect();

    let mut out = Vec::new();
    for (prop, expected_values) in &expected {
        if is_plumbing_property(prop) || plumbing_only.contains(&prop[..]) {
            continue;
        }
        if let Some(m) = compare_property(prop, expected_values, &actual) {
            out.push(m);
        }
    }
    out
}

/// 校验单个参考属性是否被 silex 的产物满足。
///
/// 依次尝试：同名属性 → 去掉厂商前缀后的同名属性 → 逻辑属性的物理等价形式。
fn compare_property(
    prop: &str,
    expected_values: &BTreeSet<String>,
    actual: &BTreeMap<String, BTreeSet<String>>,
) -> Option<Mismatch> {
    if let Some(actual_values) = actual.get(prop) {
        // silex 允许产出更多值（如把断点内的 max-width 全部展开），
        // 但 Tailwind 声明过的每个值都必须能在 silex 侧找到
        let satisfied = expected_values
            .iter()
            .all(|e| actual_values.iter().any(|a| values_equivalent(prop, e, a)));
        return (!satisfied).then(|| Mismatch::ValueDiff {
            prop: prop.to_string(),
            expected: expected_values.clone(),
            actual: actual_values.clone(),
        });
    }

    // 厂商前缀由 LightningCSS 按 browser targets 负责，基础属性出现过即可
    if let Some(base) = vendor_base(prop)
        && actual
            .keys()
            .any(|k| k == base || vendor_base(k) == Some(base))
    {
        return None;
    }

    // 逻辑属性 ↔ 物理属性：所有物理对应项都必须存在且取同一个值
    if let Some((_, physical)) = LOGICAL_EQUIVALENTS.iter().find(|(l, _)| *l == prop)
        && physical.iter().all(|p| {
            actual.get(*p).is_some_and(|vals| {
                expected_values
                    .iter()
                    .all(|e| vals.iter().any(|a| values_equivalent(p, e, a)))
            })
        })
    {
        return None;
    }

    // 独立变换属性 ↔ `transform` 里的对应函数
    if let Some((_, functions)) = TRANSFORM_EQUIVALENTS.iter().find(|(t, _)| *t == prop)
        && actual
            .get("transform")
            .is_some_and(|vals| vals.iter().any(|v| functions.iter().any(|f| v.contains(f))))
    {
        return None;
    }

    Some(Mismatch::MissingProperty {
        prop: prop.to_string(),
        expected: expected_values
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" | "),
    })
}

fn classify(mismatches: &[Mismatch]) -> Divergence {
    if mismatches
        .iter()
        .any(|m| matches!(m, Mismatch::Rejected(_)))
    {
        Divergence::Unsupported
    } else if mismatches
        .iter()
        .any(|m| matches!(m, Mismatch::MissingProperty { .. }))
    {
        Divergence::Mechanism
    } else {
        Divergence::ValueOnly
    }
}

fn render(class: &str, mismatches: &[Mismatch]) -> String {
    let mut s = format!("  {class}\n");
    for m in mismatches {
        match m {
            Mismatch::Rejected(e) => s.push_str(&format!("      ✗ tw! 拒绝该类名: {e}\n")),
            Mismatch::MissingProperty { prop, expected } => {
                s.push_str(&format!("      ✗ 缺少属性 {prop} (Tailwind: {expected})\n"))
            }
            Mismatch::ValueDiff {
                prop,
                expected,
                actual,
            } => s.push_str(&format!(
                "      ✗ {prop}: 期望 {:?}，实得 {:?}\n",
                expected, actual
            )),
        }
    }
    s
}

#[test]
fn differential_against_real_tailwind() {
    assert!(
        REFERENCE_CSS.len() > 500,
        "对拍夹具异常地小（{} 条），reference_css.json 可能没有正确生成",
        REFERENCE_CSS.len()
    );

    let mut failures = String::new();
    let mut failure_count = 0usize;
    let mut stale_ledger: Vec<&str> = Vec::new();
    let mut miscategorized: Vec<String> = Vec::new();

    for &(class, reference) in REFERENCE_CSS {
        let mismatches = compare(class, reference);
        let ledger = divergence_of(class);

        match (mismatches.is_empty(), ledger) {
            (true, None) => {}
            // 已登记但实际上已经一致 —— 台账过期，必须清理
            (true, Some(_)) => stale_ledger.push(class),
            (false, None) => {
                failure_count += 1;
                failures.push_str(&render(class, &mismatches));
            }
            (false, Some(recorded)) => {
                let actual = classify(&mismatches);
                if actual != recorded {
                    miscategorized.push(format!(
                        "  {class}: 台账记为 {recorded:?}，实际是 {actual:?}"
                    ));
                }
            }
        }
    }

    let mut report = String::new();
    if failure_count > 0 {
        report.push_str(&format!(
            "与真实 Tailwind 的对拍发现 {failure_count} 个未登记的差异：\n{failures}\n\
             修复实现，或在 KNOWN_DIVERGENCES 里登记并写明理由。\n\n"
        ));
    }
    if !stale_ledger.is_empty() {
        report.push_str(&format!(
            "KNOWN_DIVERGENCES 中有 {} 条已经不再存在差异，请删除：\n  {}\n\n",
            stale_ledger.len(),
            stale_ledger.join("\n  ")
        ));
    }
    if !miscategorized.is_empty() {
        report.push_str(&format!(
            "KNOWN_DIVERGENCES 的类别标注与实际不符：\n{}\n",
            miscategorized.join("\n")
        ));
    }

    assert!(report.is_empty(), "{report}");
}

#[test]
fn known_divergences_are_all_covered_by_the_fixture() {
    // 台账里写了一个参考数据中根本不存在的类名，"过期检查"永远不会触发它——
    // 该条目会静静地留在表里，看起来像是还有个未解决的问题。
    let fixture: BTreeSet<&str> = REFERENCE_CSS.iter().map(|(c, _)| *c).collect();
    let orphans: Vec<&str> = KNOWN_DIVERGENCES
        .iter()
        .map(|(c, _, _)| *c)
        .filter(|c| !fixture.contains(c))
        .collect();
    assert!(
        orphans.is_empty(),
        "KNOWN_DIVERGENCES 中的这些类名不在对拍夹具里，无法被验证，请删除或修正：\n  {}",
        orphans.join("\n  ")
    );

    // 每条都必须写理由
    let unexplained: Vec<&str> = KNOWN_DIVERGENCES
        .iter()
        .filter(|(_, _, reason)| reason.trim().is_empty())
        .map(|(c, _, _)| *c)
        .collect();
    assert!(
        unexplained.is_empty(),
        "KNOWN_DIVERGENCES 中这些条目没有写明理由：{unexplained:?}"
    );
}
