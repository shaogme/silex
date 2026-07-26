use std::collections::BTreeMap;

use crate::context::TwContext;

/// Tailwind 调色板中的单个色阶。
///
/// 由 `scripts/export_tailwind` 抽取到 `data/tailwind/palette.json`，
/// `silex_codegen` 反序列化后既用于生成静态色板表，也直接喂给本 crate 的 resolver。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ColorShadeInfo {
    pub shade: String,
    pub hex: String,
    pub raw: String,
    pub rgb: [u8; 3],
}

/// 标准色阶顺序，用于把 `Vec<ColorShadeInfo>` 摊平成插值需要的 11 阶数组
const STANDARD_SHADES: [&str; 11] = [
    "50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950",
];

/// 由 `palette.json` 反序列化结果驱动的 [`TwContext`]，供 codegen 侧使用。
///
/// macro 侧另有一个由生成的静态表 + `silex.toml` 驱动的实现——
/// 两者只是**数据来源**不同，规则本身共用 [`crate::color`]。
pub struct JsonPalette<'a>(pub &'a BTreeMap<String, Vec<ColorShadeInfo>>);

impl TwContext for JsonPalette<'_> {
    fn palette_shade(&self, family: &str, shade: &str) -> Option<&str> {
        Some(
            self.0
                .get(family)?
                .iter()
                .find(|s| s.shade == shade)?
                .hex
                .as_str(),
        )
    }

    fn palette_ramp(&self, family: &str) -> Option<[&str; 11]> {
        let shades = self.0.get(family)?;
        let mut ramp = [""; 11];
        for (slot, name) in ramp.iter_mut().zip(STANDARD_SHADES) {
            *slot = shades.iter().find(|s| s.shade == name)?.hex.as_str();
        }
        Some(ramp)
    }
}
