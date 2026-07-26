//! 解析上下文：把两侧各自的色板/配置后端抽象成同一个接口。
//!
//! 同一份颜色解析逻辑要同时服务两个宿主：
//!
//! - codegen 侧的色板来自运行时反序列化的 `palette.json`（`BTreeMap<String, Vec<ColorShadeInfo>>`）；
//! - macro 侧的色板来自编译期生成的静态表，且还要叠加用户 `silex.toml` 里的自定义颜色。
//!
//! 用 trait 而不是让 core 直接持有某一种后端，是为了避免"codegen 生成的表被 codegen 自己依赖"
//! 的自举循环——两侧各自注入，core 只负责规则。

/// 颜色解析所需的宿主能力
pub trait TwContext {
    /// 用户配置（`silex.toml`）中的自定义颜色。
    ///
    /// codegen 侧没有配置文件，用默认实现返回 `None` 即可。
    fn config_color(&self, _name: &str) -> Option<String> {
        None
    }

    /// 标准色板的精确色阶查找：`("slate", "900")` → `#0f172b`
    fn palette_shade(&self, family: &str, shade: &str) -> Option<&str>;

    /// 色系完整的 11 阶梯度，用于非标色阶（`slate-850`、`indigo-25`）的 RGB 线性插值。
    ///
    /// 顺序固定为 `50, 100, 200, …, 900, 950`。色阶不齐的色系返回 `None`。
    fn palette_ramp(&self, family: &str) -> Option<[&str; 11]>;
}
