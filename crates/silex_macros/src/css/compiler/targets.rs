use lightningcss::targets::Targets;

/// 默认浏览器基线。
///
/// 此前硬编码的是 chrome 80 / safari 13 / firefox 75，而运行时实际要求高得多：
///
/// | 依赖 | 最低版本 |
/// | --- | --- |
/// | `document.adoptedStyleSheets` + `new CSSStyleSheet()`（主注入路径） | Chrome 73 / Safari 16.4 / Firefox 101 |
/// | `@layer`（层序声明无条件输出） | Chrome 99 / Safari 15.4 / Firefox 97 |
/// | `color-mix()`（`CssVar::alpha`） | Chrome 111 / Safari 16.2 / Firefox 113 |
///
/// 声明的 Safari 13 目标根本跑不起来，lightningcss 为此做的降级（`::before`
/// → `:before` 之类）全是无用功。默认值现在取上表的上界。
pub(crate) const DEFAULT_TARGETS: &[(&str, u32)] = &[
    ("chrome", 111 << 16),
    ("safari", (16 << 16) | (4 << 8)),
    ("firefox", 113 << 16),
];

pub(crate) fn get_compiler_targets() -> Targets {
    static CACHE: std::sync::OnceLock<Targets> = std::sync::OnceLock::new();
    *CACHE.get_or_init(|| {
        let configured = crate::css::config::get_config()
            .map(|c| &c.css.targets)
            .filter(|t| !t.is_empty());

        let browsers = match configured {
            Some(t) => match parse_browsers(t) {
                Ok(b) => b,
                // 配置写错了不能静默退回默认值——那等于把用户写的基线当没看见
                Err(msg) => panic!("silex.toml `[css.targets]`：{msg}"),
            },
            None => {
                let mut b = lightningcss::targets::Browsers::default();
                for (name, version) in DEFAULT_TARGETS {
                    set_browser(&mut b, name, *version).expect("默认基线的浏览器名是合法的");
                }
                b
            }
        };

        Targets {
            browsers: Some(browsers),
            // 基线抬到 Chrome 111 / Safari 16.4 之后，媒体查询的区间语法
            // （`(width >= 768px)`）就在支持范围内了，lightningcss 会按那种
            // 形式打印。它不比 `(min-width: 768px)` 做得更多，却让产物与
            // Tailwind 的写法对不上——tw 的差分测试正是靠这个对齐的。
            // `include` = 无论目标是否支持都降级成 `min-`/`max-` 形式。
            include: lightningcss::targets::Features::MediaRangeSyntax
                | lightningcss::targets::Features::MediaIntervalSyntax,
            ..Targets::default()
        }
    })
}

/// 把 `[css.targets]` 解析成 lightningcss 的 `Browsers`。
pub(crate) fn parse_browsers(
    table: &std::collections::HashMap<String, String>,
) -> std::result::Result<lightningcss::targets::Browsers, String> {
    let mut browsers = lightningcss::targets::Browsers::default();
    let mut names: Vec<&String> = table.keys().collect();
    names.sort();
    for name in names {
        let raw = &table[name];
        let version = parse_version(raw).ok_or_else(|| {
            format!("`{name} = \"{raw}\"` 不是合法的版本号（形如 `16` 或 `16.4`）")
        })?;
        set_browser(&mut browsers, name, version)?;
    }
    Ok(browsers)
}

/// `"16.4"` → `16 << 16 | 4 << 8`，这是 lightningcss 的版本编码。
pub(crate) fn parse_version(raw: &str) -> Option<u32> {
    let mut parts = raw.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    let patch: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() || major > 0xffff || minor > 0xff || patch > 0xff {
        return None;
    }
    Some((major << 16) | (minor << 8) | patch)
}

pub(crate) fn set_browser(
    browsers: &mut lightningcss::targets::Browsers,
    name: &str,
    version: u32,
) -> std::result::Result<(), String> {
    let slot = match name {
        "android" => &mut browsers.android,
        "chrome" => &mut browsers.chrome,
        "edge" => &mut browsers.edge,
        "firefox" => &mut browsers.firefox,
        "ie" => &mut browsers.ie,
        "ios_saf" | "ios_safari" => &mut browsers.ios_saf,
        "opera" => &mut browsers.opera,
        "safari" => &mut browsers.safari,
        "samsung" => &mut browsers.samsung,
        other => {
            return Err(format!(
                "`{other}` 不是可识别的浏览器名（可用：android、chrome、edge、\
                 firefox、ie、ios_saf、opera、safari、samsung）"
            ));
        }
    };
    *slot = Some(version);
    Ok(())
}
