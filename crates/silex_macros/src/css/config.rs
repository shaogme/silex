use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SilexConfig {
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub css: CssConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CssConfig {
    /// 编译产物的浏览器基线，形如：
    ///
    /// ```toml
    /// [css.targets]
    /// chrome = "111"
    /// safari = "16.4"
    /// firefox = "113"
    /// ```
    ///
    /// 不写则用内置默认值（见 `compiler::DEFAULT_TARGETS`）。此前这三个版本号
    /// 是硬编码的、且与运行时真正需要的能力（adoptedStyleSheets、`@layer`、
    /// `color-mix()`）对不上——声明的 Safari 13 根本跑不起来。
    #[serde(default)]
    pub targets: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub dark_mode: Option<String>,
    #[serde(default)]
    pub colors: HashMap<String, String>,
    /// `theme!` 从配色表补出字段时，用哪个 Rust 类型。
    ///
    /// 键与 `[theme.colors]` 相同，值是 CSS 值类型名（`Hex`、`Px`、`String` …）。
    /// 不写则按取值猜：像十六进制颜色的用 `Hex`，其余用 `String`。
    ///
    /// 这一项存在的原因是：字段类型此前被硬编码成 `String`，而 `CssVar<String>`
    /// 不是 `ValidFor<props::Color>`——配置驱动的主题色恰恰**不能**用在
    /// `color()` / `background_color()` 上。
    #[serde(default)]
    pub field_types: HashMap<String, String>,
    #[serde(default)]
    pub dark_colors: HashMap<String, String>,
    #[serde(default)]
    pub breakpoints: HashMap<String, String>,
}

static CONFIG_CACHE: OnceLock<Option<(SilexConfig, Option<PathBuf>)>> = OnceLock::new();

/// 获取当前项目的 silex.toml 解析配置（编译期单例缓存）
pub fn get_config() -> Option<&'static SilexConfig> {
    get_config_with_path().as_ref().map(|(cfg, _)| cfg)
}

/// 获取 silex.toml 文件的绝对路径（若存在）
pub fn get_config_path() -> Option<&'static Path> {
    get_config_with_path()
        .as_ref()
        .and_then(|(_, path)| path.as_deref())
}

fn get_config_with_path() -> &'static Option<(SilexConfig, Option<PathBuf>)> {
    CONFIG_CACHE.get_or_init(|| {
        let mut search_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .ok()?;

        loop {
            let candidate = search_dir.join("silex.toml");
            if candidate.is_file()
                && let Ok(content) = std::fs::read_to_string(&candidate)
                && let Ok(config) = toml::from_str::<SilexConfig>(&content)
            {
                return Some((config, Some(candidate)));
            }
            if !search_dir.pop() {
                break;
            }
        }
        None
    })
}

/// 生成包含 silex.toml 路径追溯的 TokenStream，用于保障 Cargo 增量编译失效通知
pub fn generate_config_dependency_tokens() -> proc_macro2::TokenStream {
    if let Some(path) = get_config_path() {
        let path_str = path.to_string_lossy();
        quote::quote! {
            const _: () = {
                #[allow(dead_code)]
                const _SILEX_TOML_DEP: &[u8] = include_bytes!(#path_str);
            };
        }
    } else {
        quote::quote! {}
    }
}
