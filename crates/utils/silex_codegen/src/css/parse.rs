use super::syntax::{Kind, Resolver};
use super::types::{CssConfig, MdnCssProperty, MdnCssSyntax, ProcessedProp, ValueCap};
use heck::{AsPascalCase, AsSnakeCase};
use std::collections::HashMap;

pub fn parse_css(
    props_str: &str,
    syntaxes_str: &str,
) -> Result<CssConfig, Box<dyn std::error::Error>> {
    // MDN 把这些标成 `nonstandard`，但它们在真实页面里绕不开：没有它们，
    // `sty()` 连「关掉 select 的原生外观」「去掉移动端点按高亮」都写不出来。
    //
    // 注意这份白名单只能收录 **MDN 数据里存在** 的属性。`-webkit-font-smoothing`、
    // `-moz-osx-font-smoothing`、`-webkit-backdrop-filter` 根本不在 MDN 的
    // properties.json 里，加进来也不会生成任何东西——那一类只能走
    // `Style::raw(name, value)` 逃生舱。
    let whitelist = [
        "-webkit-line-clamp",
        "-webkit-text-fill-color",
        "-webkit-text-stroke",
        "-webkit-text-stroke-color",
        "-webkit-text-stroke-width",
        "-webkit-appearance",
        "-moz-appearance",
        "-webkit-user-select",
        "-webkit-user-modify",
        "-webkit-tap-highlight-color",
        "-webkit-touch-callout",
        "-webkit-overflow-scrolling",
        "-webkit-box-reflect",
        "-webkit-mask",
        "-webkit-mask-image",
        "-webkit-mask-position",
        "-webkit-mask-repeat",
        "-webkit-mask-size",
        "-webkit-mask-clip",
        "-webkit-mask-origin",
        "-webkit-mask-composite",
        "-moz-context-properties",
        "-moz-orient",
    ];

    let raw_props: HashMap<String, MdnCssProperty> = serde_json::from_str(props_str)?;
    let syntaxes: HashMap<String, MdnCssSyntax> = serde_json::from_str(syntaxes_str)?;
    let resolver = Resolver::new(&syntaxes, &raw_props);

    let mut properties = Vec::new();

    for (name, prop) in &raw_props {
        // Only standard properties, unless whitelisted
        if prop.status != "standard" && !whitelist.contains(&name.as_str()) {
            continue;
        }

        let method_name = AsSnakeCase(&name).to_string();
        let struct_name = AsPascalCase(&name).to_string();

        // Skip if empty or invalid identifiers (e.g. "--*")
        if method_name.is_empty() || struct_name.is_empty() || !is_valid_identifier(&method_name) {
            continue;
        }

        let (caps, keywords) = classify_property(prop, &resolver);

        properties.push(ProcessedProp {
            name: name.clone(),
            method_name,
            struct_name,
            caps,
            keywords,
        });
    }

    // Sort for deterministic output
    properties.sort_by(|a, b| a.name.cmp(&b.name));

    let color_keywords = collect_color_keywords(&resolver, &syntaxes);

    Ok(CssConfig {
        properties,
        syntaxes,
        color_keywords,
    })
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    for c in chars {
        if !c.is_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

/// 关键字必须能变成一个 Rust 变体名，且不能是语法记号的残渣。
fn is_usable_keyword(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() {
        return false;
    }
    s.chars().all(|c| c.is_alphanumeric() || c == '-')
}

/// 把一条属性语法翻译成「允许哪些 Rust 值类型」。
///
/// 旧实现只有一条判据：`syntax.contains(' ')` 就是 `Shorthand`（什么都收）。
/// MDN 的语法几乎都带空格，于是 78% 的属性退化成无类型约束。现在改成真正
/// 解析语法，问「哪些值可以单独构成完整取值」。
fn classify_property(prop: &MdnCssProperty, resolver: &Resolver) -> (Vec<ValueCap>, Vec<String>) {
    let analysis = resolver.analyze_syntax(&prop.syntax);

    let has = |k: Kind| analysis.singles.contains(&k);
    let mut caps: Vec<ValueCap> = Vec::new();

    if has(Kind::Length) {
        caps.push(ValueCap::Length);
    }
    if has(Kind::Percentage) {
        caps.push(ValueCap::Percent);
    }
    if has(Kind::Length) || has(Kind::Percentage) {
        caps.push(ValueCap::LenCalc);
    }
    if has(Kind::Number) {
        // `<number>` 也接受整数字面量，所以数值组直接覆盖整数组，
        // 两者同时出现会产生重复 impl
        caps.push(ValueCap::Num);
    } else if has(Kind::Integer) {
        caps.push(ValueCap::Int);
    }
    if has(Kind::Angle) {
        caps.push(ValueCap::Angle);
    }
    if has(Kind::Time) {
        caps.push(ValueCap::Time);
    }
    if has(Kind::Flex) {
        caps.push(ValueCap::Flex);
    }
    if has(Kind::Color) {
        caps.push(ValueCap::Color);
    }
    if has(Kind::Url) {
        caps.push(ValueCap::Url);
    }

    // 裸字符串是最后的兜底：只要这个属性的取值可能由多个分量拼成、或者含有
    // 我们没有对应 Rust 类型的东西（`<custom-ident>`、解析不出来的引用），
    // 就必须放行字符串，否则这些属性在 builder 里根本没法写。
    //
    // `<time>` 曾经也在这份兜底名单里——那是因为当时压根没有时间单位类型，
    // 于是 `transition-duration` 只能写 `"0.3s"`。现在有 `Sec` / `Ms` 了。
    let needs_string =
        analysis.multi || has(Kind::Textual) || has(Kind::Opaque) || prop.syntax.trim().is_empty();
    if needs_string {
        caps.push(ValueCap::Str);
    }

    let mut keywords: Vec<String> = analysis
        .keywords
        .iter()
        .filter(|k| is_usable_keyword(k))
        .cloned()
        .collect();
    keywords.sort();
    keywords.dedup();

    // 一个能力都没有、关键字也没有的属性在 builder 里将完全不可用，兜底放行字符串
    if caps.is_empty() && keywords.is_empty() {
        caps.push(ValueCap::Str);
    }

    caps.sort_by_key(|c| c.as_str());
    caps.dedup();

    (caps, keywords)
}

/// `ColorKeyword` 是全局共享的具名颜色表，需要穿透 `<color>` 收集。
///
/// 属性侧的分析把 `<color>` 当作终点（否则每个接受颜色的属性都会复制一份
/// 148 个具名颜色 + 31 个系统颜色的枚举——`keywords_gen.rs` 的 9 105 行有
/// 很大一部分正是这么来的），所以这份表在这里单独取一次。
fn collect_color_keywords(
    resolver: &Resolver,
    syntaxes: &HashMap<String, MdnCssSyntax>,
) -> Vec<String> {
    let color_syntax = syntaxes
        .get("color")
        .map(|s| s.syntax.clone())
        .unwrap_or_else(|| "<named-color> | currentcolor | transparent".to_string());
    let mut out: Vec<String> = resolver
        .harvest_keywords(&color_syntax)
        .into_iter()
        .filter(|k| is_usable_keyword(k))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver_data() -> (
        HashMap<String, MdnCssProperty>,
        HashMap<String, MdnCssSyntax>,
    ) {
        let props_str = include_str!("../../data/mdn_css_properties.json");
        let syntaxes_str = include_str!("../../data/mdn_css_syntaxes.json");
        (
            serde_json::from_str(props_str).unwrap(),
            serde_json::from_str(syntaxes_str).unwrap(),
        )
    }

    fn caps_of(name: &str) -> (Vec<ValueCap>, Vec<String>) {
        let (props, syntaxes) = resolver_data();
        let resolver = Resolver::new(&syntaxes, &props);
        let prop = props.get(name).unwrap_or_else(|| panic!("{name} 不存在"));
        classify_property(prop, &resolver)
    }

    /// 报告 P1-1 的三个招牌反例
    #[test]
    fn align_items_does_not_accept_colors() {
        let (caps, keywords) = caps_of("align-items");
        assert!(!caps.contains(&ValueCap::Color), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Length), "{caps:?}");
        assert!(keywords.contains(&"center".to_string()), "{keywords:?}");
    }

    #[test]
    fn animation_delay_does_not_accept_lengths() {
        let (caps, _) = caps_of("animation-delay");
        assert!(!caps.contains(&ValueCap::Length), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Color), "{caps:?}");
    }

    #[test]
    fn z_index_does_not_accept_strings_or_colors() {
        let (caps, _) = caps_of("z-index");
        assert!(caps.contains(&ValueCap::Int), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Str), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Color), "{caps:?}");
    }

    /// 最核心的尺寸属性必须有维度约束（旧实现里它们也是 `Shorthand`）
    #[test]
    fn width_is_a_dimension_not_a_free_for_all() {
        let (caps, keywords) = caps_of("width");
        assert!(caps.contains(&ValueCap::Length), "{caps:?}");
        assert!(caps.contains(&ValueCap::Percent), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Color), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Str), "{caps:?}");
        assert!(keywords.contains(&"auto".to_string()), "{keywords:?}");
    }

    #[test]
    fn color_accepts_colors_only() {
        let (caps, _) = caps_of("color");
        assert!(caps.contains(&ValueCap::Color), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Length), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Str), "{caps:?}");
    }

    /// 真正的复合属性仍然要能写裸字符串
    #[test]
    fn real_shorthands_still_accept_strings() {
        for name in ["margin", "border", "background", "transition", "font"] {
            let (caps, _) = caps_of(name);
            assert!(caps.contains(&ValueCap::Str), "{name}: {caps:?}");
        }
    }

    /// `margin: 4px` 这类单值写法不能因为收紧而丢掉
    #[test]
    fn margin_still_takes_a_single_length() {
        let (caps, _) = caps_of("margin");
        assert!(caps.contains(&ValueCap::Length), "{caps:?}");
        assert!(caps.contains(&ValueCap::Percent), "{caps:?}");
    }

    /// `background(AppTheme::SURFACE)`（`CssVar<Hex>`）必须还能用
    #[test]
    fn background_still_accepts_colors_and_images() {
        let (caps, _) = caps_of("background");
        assert!(caps.contains(&ValueCap::Color), "{caps:?}");
        assert!(caps.contains(&ValueCap::Url), "{caps:?}");
        assert!(caps.contains(&ValueCap::Str), "{caps:?}");
    }

    #[test]
    fn opacity_takes_numbers_and_percentages() {
        let (caps, _) = caps_of("opacity");
        assert!(caps.contains(&ValueCap::Num), "{caps:?}");
        assert!(caps.contains(&ValueCap::Percent), "{caps:?}");
        assert!(!caps.contains(&ValueCap::Color), "{caps:?}");
    }

    #[test]
    fn rotate_takes_angles() {
        let (caps, _) = caps_of("rotate");
        assert!(caps.contains(&ValueCap::Angle), "{caps:?}");
    }

    /// 数值组与整数组不能同时出现，否则会生成两份 `impl ValidFor<…> for i32`
    #[test]
    fn number_and_integer_caps_are_mutually_exclusive() {
        let (props, syntaxes) = resolver_data();
        let resolver = Resolver::new(&syntaxes, &props);
        for (name, prop) in &props {
            let (caps, _) = classify_property(prop, &resolver);
            assert!(
                !(caps.contains(&ValueCap::Num) && caps.contains(&ValueCap::Int)),
                "{name}: {caps:?}"
            );
        }
    }

    /// 具名颜色表必须来自 `<color>`，且不能泄漏进普通属性的关键字枚举
    #[test]
    fn color_keywords_are_collected_once_and_globally() {
        let (props, syntaxes) = resolver_data();
        let resolver = Resolver::new(&syntaxes, &props);
        let kws = collect_color_keywords(&resolver, &syntaxes);
        assert!(kws.contains(&"red".to_string()));
        assert!(kws.contains(&"transparent".to_string()));

        let (_, bg_keywords) = caps_of("background-color");
        assert!(
            !bg_keywords.contains(&"red".to_string()),
            "具名颜色不该复制进属性关键字枚举：{bg_keywords:?}"
        );
    }
}
