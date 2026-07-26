pub mod drift;
pub mod keyframes;
pub mod lint;
pub mod modifiers;
pub mod palette;
pub mod prefix_meta;
pub mod property_id;
pub mod reference_css;
pub mod tables;

/// 解析逻辑本身住在 `silex_tw_core`（与 `silex_macros` 共用的唯一真值源，见其 crate 文档）。
/// 这里只做转出，让 codegen 侧沿用既有的 `crate::tw::…` 路径。
pub use silex_tw_core::ColorShadeInfo;

pub use drift::*;
pub use keyframes::*;
pub use lint::*;
pub use modifiers::*;
pub use palette::*;
pub use prefix_meta::*;
pub use property_id::*;
pub use reference_css::*;
pub use tables::*;
