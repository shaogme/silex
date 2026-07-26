use crate::css::property_names::CSS_PROPERTY_NAMES;
use phf::phf_map;
use proc_macro2::Span;
use syn::Result;

/// 属性解析结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyResolveResult {
    /// 匹配到强类型的标准属性/短别名 (如 Padding, BorderRadius)
    Builtin(String),
    /// 注册表之外、无法定型的属性名：自定义变量 (`--*`) 与厂商前缀属性
    /// (`-webkit-*` 等)，统一映射为 `props::Any`
    Untyped,
}

/// 短别名映射表 (如 p => Padding, bg => BackgroundColor)
pub static SHORT_ALIAS_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "any" => "Any",
    "p" => "Padding",
    "px" => "PaddingInline",
    "py" => "PaddingBlock",
    "pt" => "PaddingTop",
    "pr" => "PaddingRight",
    "pb" => "PaddingBottom",
    "pl" => "PaddingLeft",
    "m" => "Margin",
    "mx" => "MarginInline",
    "my" => "MarginBlock",
    "mt" => "MarginTop",
    "mr" => "MarginRight",
    "mb" => "MarginBottom",
    "ml" => "MarginLeft",
    "w" => "Width",
    "h" => "Height",
    "bg" => "BackgroundColor",
    "text" => "Color",
    "border" => "BorderColor",
    "rounded" => "BorderRadius",
};

/// 解析 CSS 属性名，返回与之匹配的强类型 Struct 名称或错误。
///
/// 属性名不在注册表里就直接报错，并给出最接近的候选。此前这里对未知名称
/// 一律放行（kebab → Pascal 生成一个类型名），把问题推迟成宏展开产物里的
/// `E0412 cannot find type XxxYyy`；而**静态声明**连这一层都不走——
/// `colr: red` 会原样输出成 `colr:red`，编译通过、无警告、浏览器丢弃。
pub fn resolve_property_type(prop: &str, span: Span) -> Result<PropertyResolveResult> {
    // 1. 自定义 CSS 变量 (--*) 统一映射为 props::Any
    if prop.starts_with("--") {
        return Ok(PropertyResolveResult::Untyped);
    }

    // 2. 短别名优先 PHF 查表
    if let Some(&type_name) = SHORT_ALIAS_MAP.get(prop) {
        return Ok(PropertyResolveResult::Builtin(type_name.to_string()));
    }

    // 3. 注册表里有就用注册表里的强类型
    if CSS_PROPERTY_NAMES.binary_search(&prop).is_ok() {
        return Ok(PropertyResolveResult::Builtin(to_pascal_case(prop)));
    }

    // 4. 厂商前缀属性：MDN 数据里没有它们的语法，定不了型也拼不出建议，
    //    原样放行。这也是目前写 `-webkit-font-smoothing` 之类属性的唯一办法。
    if prop.starts_with('-') {
        return Ok(PropertyResolveResult::Untyped);
    }

    Err(syn::Error::new(span, unknown_property_message(prop)))
}

/// kebab-case → PascalCase（如 margin-top => MarginTop）
fn to_pascal_case(prop: &str) -> String {
    let mut pascal = String::with_capacity(prop.len());
    for part in prop.split('-') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            pascal.extend(first.to_uppercase());
            pascal.push_str(chars.as_str());
        }
    }
    pascal
}

fn unknown_property_message(prop: &str) -> String {
    let mut msg = format!("CSS 属性 `{prop}` 不存在");
    if let Some(candidate) = closest_property(prop) {
        msg.push_str(&format!("，是否想写 `{candidate}`？"));
    }
    msg.push_str("\n注：实验特性等注册表之外的声明请放进 `unsafe { … }` 块里原样透传。");
    msg
}

/// 找出编辑距离最近的候选属性名。
fn closest_property(prop: &str) -> Option<&'static str> {
    // 距离上限随名字长度放宽一点，但始终不超过 3——再远就不像「拼错了」
    let limit = (prop.len() / 4 + 1).min(3);
    let mut best: Option<(usize, &'static str)> = None;
    let candidates = CSS_PROPERTY_NAMES
        .iter()
        .copied()
        .chain(SHORT_ALIAS_MAP.keys().copied());
    for name in candidates {
        // 长度差已经超过上限就不用算了
        if name.len().abs_diff(prop.len()) > limit {
            continue;
        }
        let d = edit_distance(prop, name, limit);
        if d <= limit && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, name));
        }
    }
    best.map(|(_, name)| name)
}

/// Levenshtein 距离，超过 `limit` 就提前返回一个大于 `limit` 的值。
fn edit_distance(a: &str, b: &str, limit: usize) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(cur[j]);
        }
        if row_min > limit {
            return limit + 1;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_properties_resolve() {
        let span = Span::call_site();
        assert_eq!(
            resolve_property_type("margin-top", span).unwrap(),
            PropertyResolveResult::Builtin("MarginTop".into())
        );
        assert_eq!(
            resolve_property_type("p", span).unwrap(),
            PropertyResolveResult::Builtin("Padding".into())
        );
        assert_eq!(
            resolve_property_type("--my-var", span).unwrap(),
            PropertyResolveResult::Untyped
        );
    }

    /// 报告 P0-8 的招牌反例：`colr: red` 此前静默通过
    #[test]
    fn misspelled_properties_are_rejected_with_a_suggestion() {
        let err = resolve_property_type("colr", Span::call_site())
            .unwrap_err()
            .to_string();
        assert!(err.contains("`colr` 不存在"), "{err}");
        assert!(err.contains("`color`"), "{err}");
    }

    #[test]
    fn far_off_names_report_no_bogus_suggestion() {
        let err = resolve_property_type("totally-not-a-property", Span::call_site())
            .unwrap_err()
            .to_string();
        assert!(!err.contains("是否想写"), "{err}");
    }

    /// 厂商前缀属性 MDN 没有语法数据，定不了型也拼不出建议，原样放行
    #[test]
    fn vendor_prefixed_properties_pass_through_untyped() {
        assert_eq!(
            resolve_property_type("-webkit-backdrop-filter", Span::call_site()).unwrap(),
            PropertyResolveResult::Untyped
        );
        // 但注册表里有的厂商前缀属性照样强类型
        assert_eq!(
            resolve_property_type("-webkit-line-clamp", Span::call_site()).unwrap(),
            PropertyResolveResult::Builtin("WebkitLineClamp".into())
        );
    }

    /// 未知属性的报错要指出还有 `unsafe { … }` 这个出口
    #[test]
    fn unknown_property_errors_point_at_the_escape_hatch() {
        let err = resolve_property_type("totally-not-a-property", Span::call_site())
            .unwrap_err()
            .to_string();
        assert!(err.contains("unsafe { … }"), "{err}");
    }

    #[test]
    fn edit_distance_bails_out_early() {
        assert_eq!(edit_distance("color", "color", 3), 0);
        assert_eq!(edit_distance("colr", "color", 3), 1);
        assert!(edit_distance("abc", "zzzzzzzzzzzz", 3) > 3);
    }
}
