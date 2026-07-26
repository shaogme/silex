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
    /// 静态取值校验的三层开关，形如：
    ///
    /// ```toml
    /// [css.validation]
    /// keywords  = "error"   # error | warn | off
    /// functions = "error"
    /// arity     = "error"
    /// ```
    ///
    /// 三层都默认 `error`。判据来自 MDN 的值定义语法（见
    /// `css::value_check`），MDN 数据滞后时可以逐层降级成 `warn` 或 `off`
    /// ——单条声明还可以走 `unsafe { … }` 块。
    #[serde(default)]
    pub validation: ValidationConfig,
}

/// 一层校验的严格程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationLevel {
    /// 判错，中断编译
    #[default]
    Error,
    /// 降级成编译警告（走 `CssWarning` 通道），不中断编译
    Warn,
    /// 整层关掉
    Off,
}

/// 静态取值校验的三层开关。
///
/// 分成三层而不是一个总开关，是因为三层的数据可靠性并不一样：关键字表最全，
/// 函数表是手写的、最稳，多分量判据依赖 MDN 对 `multi` 的标注。哪一层出了
/// 误报就只降那一层。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct ValidationConfig {
    /// 裸关键字（`align-items: centre`）
    pub keywords: ValidationLevel,
    /// 函数式取值（`align-items: rgb(0 0 0)`）
    pub functions: ValidationLevel,
    /// 分量个数（`color: 1px solid red`）
    pub arity: ValidationLevel,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            keywords: ValidationLevel::Error,
            functions: ValidationLevel::Error,
            arity: ValidationLevel::Error,
        }
    }
}

/// 取当前项目的取值校验级别。
///
/// 没有 `silex.toml`、或者没写 `[css.validation]` 时用默认值（三层都 `error`）。
pub fn validation_levels() -> ValidationConfig {
    get_config().map(|c| c.css.validation).unwrap_or_default()
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
            {
                // 解析失败不能静默跳过。`[css.targets]` 之前的每个字段都是
                // `HashMap<String, String>`，怎么写都能解析出来，于是这条
                // `Err` 分支从来没被走到过；`[css.validation]` 的级别是个
                // 枚举，一个拼错的 `"lenient"` 会让**整份** silex.toml
                // 连同主题、断点一起消失，用户只看到「主题变量没生效」。
                //
                // 与 `compiler::get_compiler_targets` 对错配置的处理一致：
                // 宁可炸在编译期，也不把用户写的配置当没看见。
                match toml::from_str::<SilexConfig>(&content) {
                    Ok(config) => return Some((config, Some(candidate))),
                    Err(e) => panic!("解析 {} 失败：{e}", candidate.display()),
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> SilexConfig {
        toml::from_str(toml_src).expect("配置应当能解析")
    }

    /// 三层校验都默认 `error`：新判据的目的就是把错误取值挡在编译期，
    /// 默认放行等于什么都没做
    #[test]
    fn validation_defaults_to_error_on_all_three_layers() {
        let cfg = parse("[theme]\nprefix = \"x\"\n");
        assert_eq!(cfg.css.validation.keywords, ValidationLevel::Error);
        assert_eq!(cfg.css.validation.functions, ValidationLevel::Error);
        assert_eq!(cfg.css.validation.arity, ValidationLevel::Error);
    }

    /// 逃生口：MDN 数据滞后时能逐层降级，且**只**降指定的那一层
    #[test]
    fn each_layer_can_be_downgraded_independently() {
        let cfg = parse("[css.validation]\nkeywords = \"warn\"\narity = \"off\"\n");
        assert_eq!(cfg.css.validation.keywords, ValidationLevel::Warn);
        assert_eq!(cfg.css.validation.arity, ValidationLevel::Off);
        // 没写的那一层保持默认
        assert_eq!(cfg.css.validation.functions, ValidationLevel::Error);
    }

    /// 已有的 `[css.targets]` 不能被新增的 `validation` 挤掉
    #[test]
    fn targets_and_validation_coexist() {
        let cfg =
            parse("[css.targets]\nchrome = \"111\"\n\n[css.validation]\nfunctions = \"off\"\n");
        assert_eq!(
            cfg.css.targets.get("chrome").map(String::as_str),
            Some("111")
        );
        assert_eq!(cfg.css.validation.functions, ValidationLevel::Off);
    }

    /// 级别写错了必须是解析错误，而不是静默退回默认值。
    ///
    /// `get_config_with_path` 会把这个 `Err` 变成一次 panic（即一条编译错误），
    /// 见那里的注释——静默跳过会让整份 silex.toml 一起失效。
    #[test]
    fn an_unknown_level_is_a_parse_error() {
        assert!(
            toml::from_str::<SilexConfig>("[css.validation]\nkeywords = \"lenient\"\n").is_err()
        );
    }
}
